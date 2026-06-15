//! 组织架构领域模型 + 端口（对标 Benchling）。
//!
//! 层级：`租户 → 组织 → 团队`，外加资源级协作者。权限通过**带作用域的角色授予**
//! （[`RoleGrantInput`]：把角色授予 用户/团队，作用域 = 租户/组织/团队/资源）表达，
//! 有效权限取所有适用授予的**并集**（累积、最宽松）。

use async_trait::async_trait;
use dms_core::{CoreResult, Id, RequestContext, TenantId, UserId};
use serde::Serialize;
use uuid::Uuid;

/// 组织 ID 标记。
pub enum OrgMarker {}
pub type OrganizationId = Id<OrgMarker>;
/// 团队 ID 标记。
pub enum TeamMarker {}
pub type TeamId = Id<TeamMarker>;

/// 成员的结构角色（管理组织/团队本身的权限，与业务 RBAC 角色区分）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
    Admin,
    Member,
}

impl MemberRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemberRole::Admin => "admin",
            MemberRole::Member => "member",
        }
    }
    pub fn from_db(s: &str) -> Self {
        match s {
            "admin" => MemberRole::Admin,
            _ => MemberRole::Member,
        }
    }
}

/// 组织（租户内，平级）。
#[derive(Debug, Clone, Serialize)]
pub struct Organization {
    pub id: OrganizationId,
    pub tenant_id: TenantId,
    pub slug: String,
    pub name: String,
}

/// 团队（隶属某组织）。
#[derive(Debug, Clone, Serialize)]
pub struct Team {
    pub id: TeamId,
    pub tenant_id: TenantId,
    pub organization_id: OrganizationId,
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct NewOrganization {
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct NewTeam {
    pub organization_id: OrganizationId,
    pub slug: String,
    pub name: String,
}

/// 授予对象：用户或团队。
#[derive(Debug, Clone, Copy)]
pub enum Principal {
    User(UserId),
    Team(TeamId),
}

/// 授予作用域。
#[derive(Debug, Clone)]
pub enum GrantScope {
    Tenant,
    Organization(OrganizationId),
    Team(TeamId),
    Resource {
        resource_type: String,
        resource_id: Uuid,
    },
}

/// 角色授予输入：把 `role_key` 对应的角色授予 `principal`，作用域 `scope`。
#[derive(Debug, Clone)]
pub struct RoleGrantInput {
    pub principal: Principal,
    pub role_key: String,
    pub scope: GrantScope,
}

/// 组织/团队/成员仓储端口。
#[async_trait]
pub trait OrgRepository: Send + Sync {
    async fn create_organization(
        &self,
        ctx: &RequestContext,
        input: NewOrganization,
    ) -> CoreResult<Organization>;
    async fn list_organizations(&self, tenant: TenantId) -> CoreResult<Vec<Organization>>;
    async fn add_org_member(
        &self,
        ctx: &RequestContext,
        organization: OrganizationId,
        user: UserId,
        role: MemberRole,
    ) -> CoreResult<()>;
    async fn create_team(&self, ctx: &RequestContext, input: NewTeam) -> CoreResult<Team>;
    async fn list_teams(
        &self,
        tenant: TenantId,
        organization: OrganizationId,
    ) -> CoreResult<Vec<Team>>;
    async fn add_team_member(
        &self,
        ctx: &RequestContext,
        team: TeamId,
        user: UserId,
        role: MemberRole,
    ) -> CoreResult<()>;
}

/// 带作用域角色授予 + 累积权限解析端口。
#[async_trait]
pub trait GrantRepository: Send + Sync {
    async fn grant(&self, ctx: &RequestContext, input: RoleGrantInput) -> CoreResult<()>;
    async fn revoke(&self, ctx: &RequestContext, input: RoleGrantInput) -> CoreResult<()>;
    /// 用户（含其所属团队）在「租户级 + 给定附加作用域」上所有适用授予的权限**并集**。
    ///
    /// 始终包含租户级（`users`→`user_roles` 及 `role_grants` scope=tenant）；`scopes`
    /// 传入要额外计入的作用域（如某组织、某资源）。
    async fn effective_permissions(
        &self,
        tenant: TenantId,
        user: UserId,
        scopes: &[GrantScope],
    ) -> CoreResult<Vec<String>>;
}
