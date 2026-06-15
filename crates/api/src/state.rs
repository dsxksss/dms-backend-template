//! API 共享状态。
//!
//! 由组合根（`bin/server`）以结构体字面量构造并注入。字段按 feature 分档：
//! `config`/`metrics`/`health` 始终存在；`auth`/`projects`/`orgs` 各自 feature 下存在。

use std::sync::Arc;

#[cfg(feature = "auth")]
use dms_application::auth::AuthService;
#[cfg(feature = "orgs")]
use dms_application::orgs::OrgService;
use dms_application::port::HealthProbe;
#[cfg(feature = "project")]
use dms_application::project::ProjectService;
use dms_config::AppConfig;
use metrics_exporter_prometheus::PrometheusHandle;

/// 全局应用状态，必须可廉价克隆（Axum 每请求克隆一次）。
#[derive(Clone)]
pub struct AppState {
    /// 只读配置。
    pub config: Arc<AppConfig>,
    /// Prometheus 渲染句柄（供 `/metrics`）。
    pub metrics: PrometheusHandle,
    /// 就绪探针（供 `/readyz`）。精简档为 `AlwaysReady`，DB 档为 `DbHealthProbe`。
    pub health: Arc<dyn HealthProbe>,
    /// 认证服务（登录/交换/刷新/登出/校验）。
    #[cfg(feature = "auth")]
    pub auth: Arc<AuthService>,
    /// Project 用例服务。
    #[cfg(feature = "project")]
    pub projects: Arc<ProjectService>,
    /// 组织架构用例服务（组织/团队/成员/角色授予/权限解析）。
    #[cfg(feature = "orgs")]
    pub orgs: Arc<OrgService>,
}
