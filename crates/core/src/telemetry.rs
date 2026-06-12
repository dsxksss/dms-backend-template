//! 可观测性初始化：结构化日志（tracing）与指标（Prometheus）。
//!
//! 在 `bin/server` 启动最早期调用，全进程仅初始化一次。

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use serde::{Deserialize, Serialize};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

/// 日志输出格式。云端/生产用 `Json`（便于日志系统采集），本地开发用 `Pretty`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// 结构化 JSON（默认）。
    #[default]
    Json,
    /// 人类可读彩色输出。
    Pretty,
}

/// 初始化全局 tracing 订阅器。
///
/// 过滤级别优先取环境变量 `RUST_LOG`，缺省时用 `default_directive`
/// （如 `"info,sqlx=warn"`）。
pub fn init_tracing(format: LogFormat, default_directive: &str) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_directive));

    let registry = tracing_subscriber::registry().with(filter);
    match format {
        LogFormat::Json => registry
            .with(
                fmt::layer()
                    .json()
                    .flatten_event(true)
                    .with_current_span(true),
            )
            .init(),
        LogFormat::Pretty => registry.with(fmt::layer().pretty()).init(),
    }
}

/// 安装全局 Prometheus 指标记录器，返回用于渲染 `/metrics` 文本的句柄。
///
/// 句柄可克隆并存入 API 状态。
pub fn init_metrics() -> PrometheusHandle {
    PrometheusBuilder::new()
        .install_recorder()
        .expect("failed to install Prometheus metrics recorder")
}
