//! 运维端点：存活、就绪、指标。

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::json;

use crate::AppState;

/// Liveness：进程是否存活（不查依赖）。供 k8s livenessProbe。
pub async fn liveness() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

/// Readiness：是否可对外服务（检查数据库等关键依赖）。供 k8s readinessProbe。
pub async fn readiness(State(state): State<AppState>) -> impl IntoResponse {
    if state.health.ready().await {
        (StatusCode::OK, Json(json!({ "status": "ready" })))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "unavailable" })),
        )
    }
}

/// Prometheus 指标抓取端点。
pub async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    (StatusCode::OK, state.metrics.render())
}
