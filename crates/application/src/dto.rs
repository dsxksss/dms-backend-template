//! 请求 / 响应数据结构（API 边界）。

use serde::{Deserialize, Serialize};

/// 密码登录请求。
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// 租户 slug。
    pub tenant: String,
    pub email: String,
    pub password: String,
}

/// 第三方平台令牌交换请求（免登嵌入）。
#[derive(Debug, Deserialize)]
pub struct ExchangeRequest {
    pub tenant: String,
    /// 外部身份提供方标识。
    pub provider: String,
    /// 外部平台签发的令牌。
    pub token: String,
}

/// 刷新 / 登出请求。
#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub tenant: String,
    pub refresh_token: String,
}

/// 会话令牌响应。
#[derive(Debug, Serialize)]
pub struct SessionTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    /// access token 有效期（秒）。
    pub expires_in: i64,
}
