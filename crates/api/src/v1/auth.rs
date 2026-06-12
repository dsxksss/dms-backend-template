//! 认证端点：登录、令牌交换、刷新、登出，及受保护的 `/me` 演示。

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use dms_application::dto::{ExchangeRequest, LoginRequest, RefreshRequest, SessionTokens};
use serde_json::{Value, json};

use crate::error::ApiResult;
use crate::extract::AuthContext;
use crate::state::AppState;

/// 内置密码登录。
pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> ApiResult<Json<SessionTokens>> {
    Ok(Json(
        state
            .auth
            .login(&req.tenant, &req.email, &req.password)
            .await?,
    ))
}

/// 第三方平台令牌交换（免登嵌入 + JIT 账号映射）。
pub async fn exchange(
    State(state): State<AppState>,
    Json(req): Json<ExchangeRequest>,
) -> ApiResult<Json<SessionTokens>> {
    Ok(Json(
        state
            .auth
            .exchange(&req.tenant, &req.provider, &req.token)
            .await?,
    ))
}

/// 刷新会话（轮换 refresh）。
pub async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> ApiResult<Json<SessionTokens>> {
    Ok(Json(
        state.auth.refresh(&req.tenant, &req.refresh_token).await?,
    ))
}

/// 登出（吊销 refresh）。
pub async fn logout(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> ApiResult<StatusCode> {
    state.auth.logout(&req.tenant, &req.refresh_token).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// 受保护演示端点：回显当前身份与权限（验证鉴权中间件链路）。
pub async fn me(auth: AuthContext) -> ApiResult<Json<Value>> {
    Ok(Json(json!({
        "user_id": auth.ctx.actor.user_id().map(|u| u.to_string()),
        "tenant_id": auth.ctx.tenant_id.to_string(),
        "permissions": auth.perms,
    })))
}
