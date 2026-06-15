//! Project 参考切片的领域模型 + 仓储端口。
//!
//! 这是「复制我」范本：演示标准实体（强类型 ID、乐观锁版本）、领域事件，以及
//! 携带 [`RequestContext`] 的写操作端口（供基础设施在同一事务内写审计与发件箱）。

use async_trait::async_trait;
use dms_core::{CoreResult, Id, PageRequest, Paginated, RequestContext, TenantId, UserId};
use serde::{Deserialize, Serialize};

/// Project 的类型标记。
pub enum ProjectMarker {}
/// Project 强类型 ID。
pub type ProjectId = Id<ProjectMarker>;

/// 项目内成员角色（容器级结构角色，权限自 `Owner` 递减至 `Viewer`）。
///
/// 这是项目本地的协作角色，与 RBAC 的全局角色/`role_grants`（orgs 档）解耦。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectRole {
    Owner,
    Manager,
    Contributor,
    Viewer,
}

impl ProjectRole {
    /// 数据库存储用的字符串（与迁移的 CHECK 约束对齐）。
    pub fn as_str(self) -> &'static str {
        match self {
            ProjectRole::Owner => "owner",
            ProjectRole::Manager => "manager",
            ProjectRole::Contributor => "contributor",
            ProjectRole::Viewer => "viewer",
        }
    }

    /// 由数据库字符串解析（未知值返回 `None`）。
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "owner" => Some(ProjectRole::Owner),
            "manager" => Some(ProjectRole::Manager),
            "contributor" => Some(ProjectRole::Contributor),
            "viewer" => Some(ProjectRole::Viewer),
            _ => None,
        }
    }
}

/// 项目成员（用户 ↔ 项目，携带项目内角色）。
#[derive(Debug, Clone, Serialize)]
pub struct ProjectMember {
    pub project_id: ProjectId,
    pub user_id: UserId,
    pub role: ProjectRole,
}

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

    /// 列出项目成员（按加入时间）。
    async fn list_members(
        &self,
        tenant: TenantId,
        project: ProjectId,
    ) -> CoreResult<Vec<ProjectMember>>;

    /// 查询某用户在项目内的角色（无成员关系返回 `None`）。供授权判定使用。
    async fn member_role(
        &self,
        tenant: TenantId,
        project: ProjectId,
        user: UserId,
    ) -> CoreResult<Option<ProjectRole>>;

    /// 新增或更新成员角色（upsert）。
    async fn upsert_member(
        &self,
        ctx: &RequestContext,
        project: ProjectId,
        user: UserId,
        role: ProjectRole,
    ) -> CoreResult<ProjectMember>;

    /// 移除成员；不允许移除项目最后一名 `Owner`（返回冲突）。
    async fn remove_member(
        &self,
        ctx: &RequestContext,
        project: ProjectId,
        user: UserId,
    ) -> CoreResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_types() {
        let id = ProjectId::new();
        assert_eq!(
            ProjectEvent::Created {
                id,
                name: "x".into()
            }
            .event_type(),
            "project.created"
        );
        assert_eq!(
            ProjectEvent::Updated { id, version: 2 }.event_type(),
            "project.updated"
        );
        assert_eq!(ProjectEvent::Deleted { id }.event_type(), "project.deleted");
    }
}
