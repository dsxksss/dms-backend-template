//! 鉴权提取器。
//!
//! [`AuthContext`] 从 `Authorization: Bearer <token>` 解析并校验 access token，
//! 构造 [`RequestContext`]（租户 + 操作者）与权限列表，供下游 handler 使用与
//! RBAC 守卫。

use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use dms_core::{Actor, CoreError, RequestContext, TenantId, UserId};
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;

/// 已认证请求上下文 + 权限。
pub struct AuthContext {
    /// 租户 + 操作者 + request_id。
    pub ctx: RequestContext,
    /// 来自 access token 的有效权限键。
    pub perms: Vec<String>,
}

impl AuthContext {
    /// RBAC 守卫：要求具备某权限，否则返回 403。
    pub fn require(&self, permission: &str) -> Result<(), ApiError> {
        if self.perms.iter().any(|p| p == permission) {
            Ok(())
        } else {
            Err(ApiError(CoreError::Forbidden))
        }
    }
}

impl FromRequestParts<AppState> for AuthContext {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .ok_or(ApiError(CoreError::Unauthorized))?;

        let claims = state.auth.verify_access(token).map_err(ApiError)?;

        // 复用中间件注入的 x-request-id 以串联日志/审计。
        let request_id = parts
            .headers
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or_else(Uuid::now_v7);

        let ctx = RequestContext {
            request_id,
            tenant_id: TenantId::from(claims.tenant),
            actor: Actor::User {
                user_id: UserId::from(claims.sub),
            },
        };

        Ok(AuthContext {
            ctx,
            perms: claims.perms,
        })
    }
}
