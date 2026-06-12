//! API 错误映射：[`dms_core::CoreError`] → RFC 7807 Problem Details JSON。
//!
//! Handler 统一返回 [`ApiResult`]，`?` 传播的 [`CoreError`] 自动转成恰当的
//! HTTP 状态码与结构化错误体。内部错误只记日志、不向客户端泄露细节。

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use dms_core::CoreError;
use serde::Serialize;

/// RFC 7807 风格错误响应体。
#[derive(Debug, Serialize)]
pub struct ProblemDetails {
    /// 问题类型 URI（模板默认 `about:blank`，可按需指向文档）。
    #[serde(rename = "type")]
    pub type_uri: String,
    /// 简短标题（HTTP 状态短语）。
    pub title: String,
    /// HTTP 状态码。
    pub status: u16,
    /// 可选的人类可读细节（内部错误时省略）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// 包装 [`CoreError`]，实现 `IntoResponse`。
#[derive(Debug)]
pub struct ApiError(pub CoreError);

impl From<CoreError> for ApiError {
    fn from(err: CoreError) -> Self {
        Self(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            CoreError::NotFound(_) => StatusCode::NOT_FOUND,
            CoreError::Conflict(_) => StatusCode::CONFLICT,
            CoreError::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            CoreError::Unauthorized => StatusCode::UNAUTHORIZED,
            CoreError::Forbidden => StatusCode::FORBIDDEN,
            CoreError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        if status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!(error = %self.0, "internal error while handling request");
        }

        let detail = if self.0.is_client_safe() {
            Some(self.0.to_string())
        } else {
            None
        };

        let body = ProblemDetails {
            type_uri: "about:blank".to_string(),
            title: status.canonical_reason().unwrap_or("Error").to_string(),
            status: status.as_u16(),
            detail,
        };

        (status, Json(body)).into_response()
    }
}

/// Handler 统一返回类型。
pub type ApiResult<T> = Result<T, ApiError>;
