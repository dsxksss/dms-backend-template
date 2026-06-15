//! dms-server —— 组合根（composition root）。
//!
//! 唯一装配点。按 Cargo feature 分档装配（见 docs/tiers.md）：
//! - 精简档（默认）：纯 HTTP，仅 `/healthz`+`/readyz(always ok)`+`/metrics`。
//! - `database`+：连接池、迁移、DB 就绪探针。
//! - `auth`+：身份联合认证服务、`/v1/auth/*`、`bootstrap` 子命令。

use std::sync::Arc;

#[cfg(feature = "database")]
use anyhow::Context;
use dms_config::AppConfig;

#[cfg(not(feature = "database"))]
use dms_application::port::AlwaysReady;
use dms_application::port::HealthProbe;

#[cfg(feature = "auth")]
use dms_application::auth::AuthService;
#[cfg(feature = "auth")]
use dms_application::port::IdentityProvider;
#[cfg(feature = "auth")]
use dms_infrastructure::auth::{Argon2PasswordHasher, JwtTokenIssuer, PlatformTokenProvider};
#[cfg(feature = "auth")]
use dms_infrastructure::iam::{
    BootstrapInput, PgExternalIdentityRepository, PgRefreshTokenRepository, PgTenantRepository,
    PgUserRepository, bootstrap_tenant_admin,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = AppConfig::load()?;
    dms_core::telemetry::init_tracing(config.observability.format, &config.observability.level);

    handle_subcommands(&config).await?;

    let metrics = dms_core::telemetry::init_metrics();
    tracing::info!(env = %config.env, "starting dms-server");

    // 按档装配 AppState（结构体字面量，字段按 feature 门控，只有一个分支会被编译）。
    let state = {
        #[cfg(not(feature = "database"))]
        {
            let health: Arc<dyn HealthProbe> = Arc::new(AlwaysReady);
            dms_api::AppState {
                config: Arc::new(config.clone()),
                metrics,
                health,
                #[cfg(feature = "storage")]
                storage: build_blob_store(&config),
            }
        }
        #[cfg(feature = "database")]
        {
            if config.database.run_migrations_on_start {
                tracing::info!("running database migrations");
                dms_infrastructure::db::run_migrations(config.database.effective_migration_url())
                    .await
                    .context("run database migrations")?;
            }
            let pool = dms_infrastructure::db::create_pool(&config.database)
                .await
                .context("create database pool")?;
            tracing::info!("database pool ready");
            let health: Arc<dyn HealthProbe> =
                Arc::new(dms_infrastructure::health::DbHealthProbe::new(pool.clone()));

            #[cfg(feature = "audit")]
            spawn_outbox_relay(&config);

            dms_api::AppState {
                config: Arc::new(config.clone()),
                metrics,
                health,
                #[cfg(feature = "auth")]
                auth: build_auth_service(&config, &pool),
                #[cfg(feature = "project")]
                projects: build_project_service(&pool),
                #[cfg(feature = "orgs")]
                orgs: build_org_service(&pool),
                #[cfg(feature = "storage")]
                storage: build_blob_store(&config),
            }
        }
    };

    let app = dms_api::router(state);

    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| anyhow::anyhow!("failed to bind {addr}: {e}"))?;
    tracing::info!(%addr, "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| anyhow::anyhow!("server error: {e}"))?;

    tracing::info!("server stopped");
    Ok(())
}

/// 处理 CLI 子命令。`bootstrap`（创建初始租户+管理员）仅在 `auth` 档可用。
async fn handle_subcommands(_config: &AppConfig) -> anyhow::Result<()> {
    match std::env::args().nth(1).as_deref() {
        None => Ok(()),
        #[cfg(feature = "auth")]
        Some("bootstrap") => {
            run_bootstrap(_config).await?;
            std::process::exit(0);
        }
        Some(other) => anyhow::bail!("unknown subcommand: {other}"),
    }
}

/// 组装身份联合认证服务：仓储 + 密码哈希 + JWT 签发 + 已配置的外部身份 provider。
#[cfg(feature = "auth")]
fn build_auth_service(config: &AppConfig, pool: &sqlx::PgPool) -> Arc<AuthService> {
    let tenants = Arc::new(PgTenantRepository::new(pool.clone()));
    let users = Arc::new(PgUserRepository::new(pool.clone()));
    let externals = Arc::new(PgExternalIdentityRepository::new(pool.clone()));
    let refresh = Arc::new(PgRefreshTokenRepository::new(pool.clone()));
    let hasher = Arc::new(Argon2PasswordHasher);
    let tokens = Arc::new(JwtTokenIssuer::new(
        &config.auth.jwt_secret,
        config.auth.issuer.clone(),
        config.auth.access_ttl_secs,
    ));

    let mut providers: Vec<Arc<dyn IdentityProvider>> = Vec::new();
    if let Some(platform) = &config.auth.platform {
        providers.push(Arc::new(PlatformTokenProvider::new(
            platform.name.clone(),
            &platform.jwt_secret,
            platform.issuer.clone(),
        )));
        tracing::info!(provider = %platform.name, "registered platform identity provider");
    }

    Arc::new(AuthService::new(
        tenants,
        users,
        externals,
        refresh,
        hasher,
        tokens,
        providers,
        config.auth.refresh_ttl_secs,
    ))
}

/// 组装 Project 用例服务。
#[cfg(feature = "project")]
fn build_project_service(pool: &sqlx::PgPool) -> Arc<dms_application::project::ProjectService> {
    let repo = Arc::new(dms_infrastructure::project::PgProjectRepository::new(
        pool.clone(),
    ));
    Arc::new(dms_application::project::ProjectService::new(repo))
}

/// 组装组织架构用例服务。
#[cfg(feature = "orgs")]
fn build_org_service(pool: &sqlx::PgPool) -> Arc<dms_application::orgs::OrgService> {
    let orgs = Arc::new(dms_infrastructure::orgs::PgOrgRepository::new(pool.clone()));
    let grants = Arc::new(dms_infrastructure::orgs::PgGrantRepository::new(
        pool.clone(),
    ));
    Arc::new(dms_application::orgs::OrgService::new(orgs, grants))
}

/// 组装对象存储（当前文件系统后端；S3/MinIO 可替换 BlobStore 实现）。
#[cfg(feature = "storage")]
fn build_blob_store(config: &AppConfig) -> Arc<dyn dms_application::port::BlobStore> {
    Arc::new(dms_infrastructure::storage::FilesystemBlobStore::new(
        &config.storage.root,
    ))
}

/// 启动发件箱中继后台任务（拥有者连接，绕过 RLS 扫描全部租户）。
#[cfg(feature = "audit")]
fn spawn_outbox_relay(config: &AppConfig) {
    let url = config.database.effective_migration_url().to_string();
    tokio::spawn(async move {
        match dms_infrastructure::db::create_pool_from_url(&url, 2).await {
            Ok(pool) => {
                dms_infrastructure::outbox::run_relay(pool, std::time::Duration::from_secs(5)).await
            }
            Err(e) => tracing::error!(error = %e, "failed to start outbox relay"),
        }
    });
}

/// `bootstrap` 子命令：创建初始租户、seed 标准角色（owner/admin/member），
/// 并将首个用户授予 `owner`。
#[cfg(feature = "auth")]
async fn run_bootstrap(config: &AppConfig) -> anyhow::Result<()> {
    tracing::info!("running migrations before bootstrap");
    dms_infrastructure::db::run_migrations(config.database.effective_migration_url())
        .await
        .context("run database migrations")?;

    let pool = dms_infrastructure::db::create_pool(&config.database)
        .await
        .context("create database pool")?;
    let hasher = Argon2PasswordHasher;

    let slug = std::env::var("BOOTSTRAP_TENANT_SLUG").unwrap_or_else(|_| "acme".to_string());
    let name = std::env::var("BOOTSTRAP_TENANT_NAME").unwrap_or_else(|_| "Acme".to_string());
    let email = std::env::var("BOOTSTRAP_EMAIL").context("BOOTSTRAP_EMAIL is required")?;
    let password = std::env::var("BOOTSTRAP_PASSWORD").context("BOOTSTRAP_PASSWORD is required")?;

    bootstrap_tenant_admin(
        &pool,
        &hasher,
        BootstrapInput {
            tenant_slug: &slug,
            tenant_name: &name,
            email: &email,
            password: &password,
            role_key: "owner",
        },
    )
    .await
    .context("bootstrap tenant admin")?;

    tracing::info!(tenant = %slug, %email, "bootstrap complete");
    Ok(())
}

/// 等待 Ctrl-C 或 SIGTERM（容器优雅停机）。
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received, draining connections");
}
