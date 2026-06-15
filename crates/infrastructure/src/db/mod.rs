//! 数据库接入：连接池、迁移、错误映射。

#[cfg(feature = "multi-tenancy")]
pub mod tenant;

use std::time::Duration;

use dms_config::DatabaseConfig;
use dms_core::CoreError;
use sqlx::PgPool;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;

// 迁移按档分目录、预留版本区间（core 1-99 / tenancy 100-199 / auth 200-299），
// 按启用档顺序执行（core → tenancy → auth）。各 migrator 只含本档迁移，运行时开
// ignore_missing，使「其它档的已应用记录」不被当作缺失而报错。

/// 将 SQLx 错误映射为统一的 [`CoreError`]。
///
/// 供租户作用域事务与仓储使用（`multi-tenancy`+ 档），故在该档下编译。
#[cfg(feature = "multi-tenancy")]
pub fn map_db_error(err: sqlx::Error) -> CoreError {
    use sqlx::error::ErrorKind;

    match err {
        sqlx::Error::RowNotFound => CoreError::NotFound("resource not found".into()),
        sqlx::Error::Database(ref db) => match db.kind() {
            ErrorKind::UniqueViolation => CoreError::Conflict("unique constraint violated".into()),
            ErrorKind::ForeignKeyViolation => {
                CoreError::Conflict("referenced resource does not exist".into())
            }
            ErrorKind::CheckViolation => CoreError::Validation("check constraint violated".into()),
            ErrorKind::NotNullViolation => CoreError::Validation("required field missing".into()),
            _ => {
                tracing::error!(error = %err, "database error");
                CoreError::internal("database error")
            }
        },
        _ => {
            tracing::error!(error = %err, "database error");
            CoreError::internal("database error")
        }
    }
}

/// 创建运行时连接池。
///
/// 多租户档下应以受 RLS 约束的应用角色（如 `dms_app`）连接。
pub async fn create_pool(cfg: &DatabaseConfig) -> Result<PgPool, CoreError> {
    PgPoolOptions::new()
        .max_connections(cfg.max_connections)
        .acquire_timeout(Duration::from_secs(cfg.acquire_timeout_secs))
        .connect(&cfg.url)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to connect to database");
            CoreError::internal("database connection failed")
        })
}

/// 以指定 URL 建池。发件箱中继用拥有者连接（绕过 RLS）扫描全部租户的未处理事件。
#[cfg(feature = "audit")]
pub async fn create_pool_from_url(url: &str, max_connections: u32) -> Result<PgPool, CoreError> {
    PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(url)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to connect relay owner pool");
            CoreError::internal("relay database connection failed")
        })
}

/// 以拥有者角色执行迁移（需要 DDL 权限）。按启用档顺序执行各迁移目录。
pub async fn run_migrations(url: &str) -> Result<(), CoreError> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(url)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to connect for migrations");
            CoreError::internal("migration connection failed")
        })?;

    run_one(&pool, sqlx::migrate!("../../migrations/core")).await?;
    #[cfg(feature = "multi-tenancy")]
    run_one(&pool, sqlx::migrate!("../../migrations/tenancy")).await?;
    #[cfg(feature = "auth")]
    run_one(&pool, sqlx::migrate!("../../migrations/auth")).await?;
    #[cfg(feature = "audit")]
    run_one(&pool, sqlx::migrate!("../../migrations/audit")).await?;
    #[cfg(feature = "project")]
    run_one(&pool, sqlx::migrate!("../../migrations/project")).await?;
    #[cfg(feature = "orgs")]
    run_one(&pool, sqlx::migrate!("../../migrations/orgs")).await?;

    pool.close().await;
    Ok(())
}

async fn run_one(pool: &PgPool, mut migrator: Migrator) -> Result<(), CoreError> {
    // 每个分档 migrator 只含本档迁移；忽略「其它档已应用」记录的缺失校验。
    migrator.set_ignore_missing(true);
    migrator.run(pool).await.map_err(|e| {
        tracing::error!(error = %e, "migration failed");
        CoreError::internal("migration failed")
    })
}

/// 就绪探测：能否成功执行一次最简查询。
pub async fn ping(pool: &PgPool) -> bool {
    sqlx::query("SELECT 1").execute(pool).await.is_ok()
}
