//! # dms-api — HTTP 层
//!
//! Axum 路由、通用中间件栈、错误映射、健康/指标端点。依赖 `application`
//! 暴露的用例，不直接依赖 `infrastructure`——具体实现由 `bin/server` 组合根
//! 装配后注入 [`AppState`]。
//!
//! 按 feature 分档：始终提供 `/healthz`/`/readyz`/`/metrics`；`auth` 档额外挂载
//! `/v1/auth/*` 与鉴权提取器。

pub mod error;
#[cfg(feature = "auth")]
pub mod extract;
pub mod health;
pub mod state;
#[cfg(feature = "auth")]
pub mod v1;

use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use axum::routing::get;
use tower::ServiceBuilder;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

pub use error::{ApiError, ApiResult, ProblemDetails};
#[cfg(feature = "auth")]
pub use extract::AuthContext;
pub use state::AppState;

/// 组装根路由 + 通用中间件栈。`auth` 档下额外挂载 `/v1`。
pub fn router(state: AppState) -> Router {
    let timeout = Duration::from_secs(state.config.server.request_timeout_secs);

    let middleware = ServiceBuilder::new()
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(CatchPanicLayer::new())
        // 模板默认放开 CORS；生产应按域名收紧（见 docs/deployment.md）。
        .layer(CorsLayer::permissive())
        .layer(CompressionLayer::new())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            timeout,
        ));

    let app = Router::new()
        .route("/healthz", get(health::liveness))
        .route("/readyz", get(health::readiness))
        .route("/metrics", get(health::metrics));

    #[cfg(feature = "auth")]
    let app = app.nest("/v1", v1::router());

    app.layer(middleware).with_state(state)
}
