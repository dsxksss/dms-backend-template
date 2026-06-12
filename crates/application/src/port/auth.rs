//! 认证相关端口。
//!
//! 把「如何校验外部令牌」「如何哈希密码」「如何签发/校验会话令牌」抽象成端口，
//! 实现可插拔：内置密码 + 专有平台 token 交换为本期实现，OIDC/SAML/LDAP 后续
//! 追加新的 [`IdentityProvider`] 即可，无需改动应用层。

use async_trait::async_trait;
use dms_core::{CoreResult, TenantId, UserId};
use serde::{Deserialize, Serialize};

/// 外部身份提供方校验后的标准结果。
#[derive(Debug, Clone)]
pub struct VerifiedIdentity {
    pub provider: String,
    pub subject: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub raw_claims: serde_json::Value,
}

/// 外部身份提供方端口（联合认证）。
///
/// 每个外部源（专有平台、OIDC、SAML…）实现一个，按 [`name`](Self::name) 注册。
#[async_trait]
pub trait IdentityProvider: Send + Sync {
    /// 提供方标识（与登录请求中的 `provider` 对应）。
    fn name(&self) -> &str;
    /// 校验外部凭证（如平台签发的 JWT），返回标准化的外部身份。
    async fn verify(&self, credential: &str) -> CoreResult<VerifiedIdentity>;
}

/// 密码哈希端口（内置密码登录用）。
pub trait PasswordHasher: Send + Sync {
    fn hash(&self, password: &str) -> CoreResult<String>;
    fn verify(&self, password: &str, hash: &str) -> bool;
}

/// access token 的声明。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessClaims {
    /// 用户 ID。
    pub sub: uuid::Uuid,
    /// 租户 ID。
    pub tenant: uuid::Uuid,
    /// 签发者。
    pub iss: String,
    /// 签发时间（Unix 秒）。
    pub iat: i64,
    /// 过期时间（Unix 秒）。
    pub exp: i64,
    /// 用户有效权限键（用于 RBAC，免去每请求查库）。
    pub perms: Vec<String>,
}

/// access token 签发/校验端口。
pub trait TokenIssuer: Send + Sync {
    /// 签发 access token，返回 `(token, expires_in_secs)`。
    fn issue(&self, user: UserId, tenant: TenantId, perms: &[String]) -> CoreResult<(String, i64)>;
    /// 校验并解析 access token。
    fn verify(&self, token: &str) -> CoreResult<AccessClaims>;
}
