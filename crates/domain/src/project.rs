//! Project 参考切片的领域模型 + 仓储端口。
//!
//! 这是「复制我」范本：演示标准实体（强类型 ID、乐观锁版本）、领域事件，以及
//! 携带 [`RequestContext`] 的写操作端口（供基础设施在同一事务内写审计与发件箱）。

use async_trait::async_trait;
use dms_core::{CoreResult, Id, PageRequest, Paginated, RequestContext, TenantId};
use serde::Serialize;

/// Project 的类型标记。
pub enum ProjectMarker {}
/// Project 强类型 ID。
pub type ProjectId = Id<ProjectMarker>;

/// 项目实体。
#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub id: ProjectId,
    pub tenant_id: TenantId,
    pub name: String,
    pub description: String,
    /// 乐观锁版本。
    pub version: i32,
}

/// 创建输入。
#[derive(Debug, Clone)]
pub struct NewProject {
    pub name: String,
    pub description: String,
}

/// 更新输入（部分字段 + 期望版本，用于乐观锁）。
#[derive(Debug, Clone)]
pub struct UpdateProject {
    pub name: Option<String>,
    pub description: Option<String>,
    pub expected_version: i32,
}

/// 领域事件（写入 outbox 的 payload）。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ProjectEvent {
    Created { id: ProjectId, name: String },
    Updated { id: ProjectId, version: i32 },
    Deleted { id: ProjectId },
}

impl ProjectEvent {
    /// 事件类型（写入 outbox.event_type）。
    pub fn event_type(&self) -> &'static str {
        match self {
            ProjectEvent::Created { .. } => "project.created",
            ProjectEvent::Updated { .. } => "project.updated",
            ProjectEvent::Deleted { .. } => "project.deleted",
        }
    }
}

/// 项目仓储端口。
///
/// 写操作接收 [`RequestContext`]（租户 + 操作者 + request_id），实现侧在**同一
/// 租户作用域事务**内完成状态变更 + 审计写入 + 发件箱入队，保证原子与可追溯。
#[async_trait]
pub trait ProjectRepository: Send + Sync {
    async fn create(&self, ctx: &RequestContext, input: NewProject) -> CoreResult<Project>;
    async fn get(&self, tenant: TenantId, id: ProjectId) -> CoreResult<Option<Project>>;
    async fn list(&self, tenant: TenantId, page: PageRequest) -> CoreResult<Paginated<Project>>;
    async fn update(
        &self,
        ctx: &RequestContext,
        id: ProjectId,
        input: UpdateProject,
    ) -> CoreResult<Project>;
    /// 软删除（带乐观锁版本校验）。
    async fn delete(
        &self,
        ctx: &RequestContext,
        id: ProjectId,
        expected_version: i32,
    ) -> CoreResult<()>;
}
