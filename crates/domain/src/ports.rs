//! 仓储端口（trait）。由 `infrastructure` 以 SQLx 实现，`bin/server` 注入。
//!
//! 端口方法只用领域类型与 [`dms_core`] 类型，不泄露 sqlx。租户作用域与 RLS
//! 由实现内部（经 `begin_tenant_tx`）处理；需要原子性的复合操作以单个粗粒度
//! 方法暴露（如 [`ExternalIdentityRepository::resolve_or_provision`]）。

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dms_core::{CoreResult, TenantId, UserId};

use crate::iam::{Tenant, User};

/// 租户注册表（无 RLS：登录/路由阶段需在未知租户上下文时按 slug 解析）。
#[async_trait]
pub trait TenantRepository: Send + Sync {
    async fn find_by_slug(&self, slug: &str) -> CoreResult<Option<Tenant>>;
}

/// 用户仓储 + 用户的有效权限查询。
#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_email(&self, tenant: TenantId, email: &str) -> CoreResult<Option<User>>;
    async fn find_by_id(&self, tenant: TenantId, id: UserId) -> CoreResult<Option<User>>;
    /// 经角色聚合出的用户有效权限键（如 `project:write`）。
    async fn permissions_for(&self, tenant: TenantId, user: UserId) -> CoreResult<Vec<String>>;
}

/// JIT 外部身份开通的输入。
#[derive(Debug, Clone)]
pub struct ProvisionExternal {
    pub provider: String,
    pub subject: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub raw_claims: serde_json::Value,
}

/// 外部身份映射仓储。
#[async_trait]
pub trait ExternalIdentityRepository: Send + Sync {
    /// JIT：把已校验的外部身份解析到内部用户。
    ///
    /// 顺序：命中 `(provider, subject)` 既有映射 → 否则按已验证 email 匹配既有用户
    /// → 再否则自动开通新用户；最后确保映射存在并刷新 `last_login_at`。整个过程
    /// 在单个租户作用域事务内完成，保证原子性。
    async fn resolve_or_provision(
        &self,
        tenant: TenantId,
        input: ProvisionExternal,
    ) -> CoreResult<User>;
}

/// 新刷新令牌（存摘要）。
#[derive(Debug, Clone)]
pub struct NewRefreshToken {
    pub user_id: UserId,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
}

/// 有效刷新令牌记录。
#[derive(Debug, Clone)]
pub struct ActiveRefreshToken {
    pub user_id: UserId,
}

/// 刷新令牌仓储（轮换/吊销）。
#[async_trait]
pub trait RefreshTokenRepository: Send + Sync {
    async fn issue(&self, tenant: TenantId, token: NewRefreshToken) -> CoreResult<()>;
    /// 按摘要查找未吊销、未过期的令牌。
    async fn find_active(
        &self,
        tenant: TenantId,
        token_hash: &str,
    ) -> CoreResult<Option<ActiveRefreshToken>>;
    async fn revoke(&self, tenant: TenantId, token_hash: &str) -> CoreResult<()>;
}
