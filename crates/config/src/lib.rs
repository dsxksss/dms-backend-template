//! # dms-config — 分层配置
//!
//! 加载顺序（后者覆盖前者）：
//! 1. 代码内置默认值（[`AppConfig::default`]）
//! 2. `config/default.toml`（所有环境共享基线）
//! 3. `config/{APP_ENV}.toml`（按环境覆盖：local / cloud / onprem）
//! 4. 环境变量 `DMS__*`（双下划线表层级，如 `DMS__SERVER__PORT`）
//! 5. `DATABASE_URL`（与 sqlx-cli 对齐，单独覆盖数据库连接串）
//!
//! 密钥（jwt_secret、database.url）不写入配置文件，仅由环境变量/密钥管理注入。

use dms_core::telemetry::LogFormat;
use figment::Figment;
use figment::providers::{Env, Format, Serialized, Toml};
use serde::{Deserialize, Serialize};

/// 顶层应用配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// 运行环境名：local | cloud | onprem。
    pub env: String,
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub auth: AuthConfig,
    pub observability: ObservabilityConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            env: "local".to_string(),
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            auth: AuthConfig::default(),
            observability: ObservabilityConfig::default(),
        }
    }
}

/// HTTP 服务配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    /// 单请求超时（秒）。
    pub request_timeout_secs: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            request_timeout_secs: 30,
        }
    }
}

/// 数据库 / 连接池配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DatabaseConfig {
    /// 运行时连接串：以受 RLS 约束的应用角色（如 `dms_app`）连接。
    /// 生产环境经 `DATABASE_URL` 或密钥管理注入。
    pub url: String,
    /// 迁移连接串：以拥有者角色（具备 DDL 权限）连接。为空则回退用 `url`。
    /// 拆成两个角色是 RLS 生效的前提——运行时角色不能是超级用户/BYPASSRLS。
    pub migration_url: Option<String>,
    pub max_connections: u32,
    pub acquire_timeout_secs: u64,
    /// 启动时是否自动执行迁移（dev/onprem 便利；云端通常由发布流程单独执行）。
    pub run_migrations_on_start: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgres://dms_app:dms_app@127.0.0.1:5432/dms".to_string(),
            migration_url: None,
            max_connections: 10,
            acquire_timeout_secs: 5,
            run_migrations_on_start: false,
        }
    }
}

impl DatabaseConfig {
    /// 执行迁移使用的连接串：优先 `migration_url`，否则回退 `url`。
    pub fn effective_migration_url(&self) -> &str {
        self.migration_url.as_deref().unwrap_or(&self.url)
    }
}

/// 认证 / 会话配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthConfig {
    /// 签发 access token 的对称密钥（生产请用强随机值）。
    pub jwt_secret: String,
    /// JWT issuer 声明。
    pub issuer: String,
    /// access token 有效期（秒）。
    pub access_ttl_secs: i64,
    /// refresh token 有效期（秒）。
    pub refresh_ttl_secs: i64,
    /// 可选的专有平台令牌交换 provider（账号联动）。配置后即注册该外部身份源。
    pub platform: Option<PlatformProviderConfig>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: "dev-only-change-me-please-change-in-prod".to_string(),
            issuer: "dms".to_string(),
            access_ttl_secs: 900,
            refresh_ttl_secs: 1_209_600,
            platform: None,
        }
    }
}

/// 专有/既有平台令牌交换 provider 配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformProviderConfig {
    /// provider 标识（登录请求里的 `provider` 字段与之对应）。
    pub name: String,
    /// 校验平台令牌（HS256 JWT）的共享密钥。
    pub jwt_secret: String,
    /// 期望的令牌 issuer（可选，配置后会校验）。
    #[serde(default)]
    pub issuer: Option<String>,
}

/// 可观测性配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ObservabilityConfig {
    /// 日志格式。
    pub format: LogFormat,
    /// 默认日志过滤指令（`RUST_LOG` 存在时以其为准）。
    pub level: String,
    /// 是否启用 Prometheus 指标。
    pub metrics_enabled: bool,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            format: LogFormat::default(),
            level: "info,sqlx=warn,tower_http=info".to_string(),
            metrics_enabled: true,
        }
    }
}

/// 配置加载错误。
///
/// `figment::Error` 体积较大，装箱以保持 `Result` 紧凑（满足 clippy
/// `result_large_err`）。
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("configuration error: {0}")]
    Figment(Box<figment::Error>),
}

impl From<figment::Error> for ConfigError {
    fn from(err: figment::Error) -> Self {
        Self::Figment(Box::new(err))
    }
}

impl AppConfig {
    /// 按分层顺序加载配置。
    pub fn load() -> Result<Self, ConfigError> {
        let env = std::env::var("APP_ENV").unwrap_or_else(|_| "local".to_string());

        let mut cfg: AppConfig = Figment::from(Serialized::defaults(AppConfig::default()))
            .merge(Toml::file("config/default.toml"))
            .merge(Toml::file(format!("config/{env}.toml")))
            .merge(Env::prefixed("DMS__").split("__"))
            .extract()?;

        cfg.env = env;

        // 与 sqlx-cli 对齐：DATABASE_URL 单独覆盖。
        if let Ok(url) = std::env::var("DATABASE_URL") {
            cfg.database.url = url;
        }

        Ok(cfg)
    }
}
