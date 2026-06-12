//! 身份联合与会话用例。
//!
//! 编排两条登录路径——内置密码 与 第三方令牌交换（JIT 映射）——统一签发我们
//! 自己的 access + refresh 会话，并支持刷新轮换与登出吊销。

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{Duration, Utc};
use dms_core::{CoreError, CoreResult, TenantId};
use dms_domain::iam::User;
use dms_domain::ports::{
    ExternalIdentityRepository, NewRefreshToken, ProvisionExternal, RefreshTokenRepository,
    TenantRepository, UserRepository,
};

use crate::dto::SessionTokens;
use crate::port::{AccessClaims, IdentityProvider, PasswordHasher, TokenIssuer};
use crate::token::{generate_refresh_token, hash_token};

/// 认证服务：组合各端口完成登录/交换/刷新/登出。
pub struct AuthService {
    tenants: Arc<dyn TenantRepository>,
    users: Arc<dyn UserRepository>,
    externals: Arc<dyn ExternalIdentityRepository>,
    refresh: Arc<dyn RefreshTokenRepository>,
    hasher: Arc<dyn PasswordHasher>,
    tokens: Arc<dyn TokenIssuer>,
    providers: HashMap<String, Arc<dyn IdentityProvider>>,
    refresh_ttl: Duration,
}

impl AuthService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tenants: Arc<dyn TenantRepository>,
        users: Arc<dyn UserRepository>,
        externals: Arc<dyn ExternalIdentityRepository>,
        refresh: Arc<dyn RefreshTokenRepository>,
        hasher: Arc<dyn PasswordHasher>,
        tokens: Arc<dyn TokenIssuer>,
        provider_list: Vec<Arc<dyn IdentityProvider>>,
        refresh_ttl_secs: i64,
    ) -> Self {
        let providers = provider_list
            .into_iter()
            .map(|p| (p.name().to_string(), p))
            .collect();
        Self {
            tenants,
            users,
            externals,
            refresh,
            hasher,
            tokens,
            providers,
            refresh_ttl: Duration::seconds(refresh_ttl_secs),
        }
    }

    /// 按 slug 解析活跃租户（鉴权失败一律 401，避免探测租户是否存在）。
    async fn resolve_tenant(&self, slug: &str) -> CoreResult<TenantId> {
        let tenant = self
            .tenants
            .find_by_slug(slug)
            .await?
            .ok_or(CoreError::Unauthorized)?;
        if !tenant.is_active() {
            return Err(CoreError::Forbidden);
        }
        Ok(tenant.id)
    }

    /// 为用户签发新会话（access + 新 refresh）。
    async fn issue_session(&self, tenant: TenantId, user: &User) -> CoreResult<SessionTokens> {
        let perms = self.users.permissions_for(tenant, user.id).await?;
        let (access_token, expires_in) = self.tokens.issue(user.id, tenant, &perms)?;

        let raw_refresh = generate_refresh_token();
        self.refresh
            .issue(
                tenant,
                NewRefreshToken {
                    user_id: user.id,
                    token_hash: hash_token(&raw_refresh),
                    expires_at: Utc::now() + self.refresh_ttl,
                },
            )
            .await?;

        Ok(SessionTokens {
            access_token,
            refresh_token: raw_refresh,
            token_type: "Bearer",
            expires_in,
        })
    }

    /// 内置密码登录。
    pub async fn login(
        &self,
        tenant_slug: &str,
        email: &str,
        password: &str,
    ) -> CoreResult<SessionTokens> {
        let tenant = self.resolve_tenant(tenant_slug).await?;
        let user = self
            .users
            .find_by_email(tenant, email)
            .await?
            .ok_or(CoreError::Unauthorized)?;

        let hash = user
            .password_hash
            .as_deref()
            .ok_or(CoreError::Unauthorized)?;
        if !self.hasher.verify(password, hash) {
            return Err(CoreError::Unauthorized);
        }
        if !user.status.is_active() {
            return Err(CoreError::Forbidden);
        }

        self.issue_session(tenant, &user).await
    }

    /// 第三方平台令牌交换（免登嵌入 + JIT 账号映射）。
    pub async fn exchange(
        &self,
        tenant_slug: &str,
        provider_name: &str,
        credential: &str,
    ) -> CoreResult<SessionTokens> {
        let tenant = self.resolve_tenant(tenant_slug).await?;
        let provider = self
            .providers
            .get(provider_name)
            .ok_or(CoreError::Unauthorized)?;

        let verified = provider.verify(credential).await?;
        let user = self
            .externals
            .resolve_or_provision(
                tenant,
                ProvisionExternal {
                    provider: verified.provider,
                    subject: verified.subject,
                    email: verified.email,
                    display_name: verified.display_name,
                    raw_claims: verified.raw_claims,
                },
            )
            .await?;

        if !user.status.is_active() {
            return Err(CoreError::Forbidden);
        }

        self.issue_session(tenant, &user).await
    }

    /// 刷新会话：校验旧 refresh → 吊销（轮换）→ 签发新会话。
    pub async fn refresh(
        &self,
        tenant_slug: &str,
        refresh_token: &str,
    ) -> CoreResult<SessionTokens> {
        let tenant = self.resolve_tenant(tenant_slug).await?;
        let hash = hash_token(refresh_token);

        let active = self
            .refresh
            .find_active(tenant, &hash)
            .await?
            .ok_or(CoreError::Unauthorized)?;

        // 轮换：先吊销旧令牌，再签发新会话。
        self.refresh.revoke(tenant, &hash).await?;

        let user = self
            .users
            .find_by_id(tenant, active.user_id)
            .await?
            .ok_or(CoreError::Unauthorized)?;

        self.issue_session(tenant, &user).await
    }

    /// 登出：吊销指定 refresh 令牌。
    pub async fn logout(&self, tenant_slug: &str, refresh_token: &str) -> CoreResult<()> {
        let tenant = self.resolve_tenant(tenant_slug).await?;
        self.refresh
            .revoke(tenant, &hash_token(refresh_token))
            .await
    }

    /// 校验 access token（供 API 鉴权中间件）。
    pub fn verify_access(&self, token: &str) -> CoreResult<AccessClaims> {
        self.tokens.verify(token)
    }
}
