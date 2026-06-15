//! 组织架构用例 + DTO + 作用域权限解析。

use std::sync::Arc;

use dms_core::{CoreError, CoreResult, RequestContext, TenantId, UserId};
use dms_domain::orgs::{
    GrantRepository, GrantScope, MemberRole, NewOrganization, NewTeam, OrgRepository, Organization,
    OrganizationId, Principal, RoleGrantInput, Team, TeamId,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---- DTO ----

#[derive(Debug, Deserialize)]
pub struct CreateOrganizationRequest {
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct OrganizationResponse {
    pub id: OrganizationId,
    pub slug: String,
    pub name: String,
}

impl From<Organization> for OrganizationResponse {
    fn from(o: Organization) -> Self {
        Self {
            id: o.id,
            slug: o.slug,
            name: o.name,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateTeamRequest {
    pub organization_id: OrganizationId,
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct TeamResponse {
    pub id: TeamId,
    pub organization_id: OrganizationId,
    pub slug: String,
    pub name: String,
}

impl From<Team> for TeamResponse {
    fn from(t: Team) -> Self {
        Self {
            id: t.id,
            organization_id: t.organization_id,
            slug: t.slug,
            name: t.name,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    pub user_id: Uuid,
    #[serde(default)]
    pub role: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GrantRequest {
    pub principal_type: String,
    pub principal_id: Uuid,
    pub role_key: String,
    pub scope_type: String,
    #[serde(default)]
    pub scope_id: Option<Uuid>,
    #[serde(default)]
    pub resource_type: Option<String>,
}

fn parse_member_role(s: &Option<String>) -> MemberRole {
    match s.as_deref() {
        Some("admin") => MemberRole::Admin,
        _ => MemberRole::Member,
    }
}

fn parse_principal(kind: &str, id: Uuid) -> CoreResult<Principal> {
    match kind {
        "user" => Ok(Principal::User(UserId::from(id))),
        "team" => Ok(Principal::Team(TeamId::from(id))),
        _ => Err(CoreError::Validation(
            "principal_type must be user|team".into(),
        )),
    }
}

fn parse_scope(req: &GrantRequest) -> CoreResult<GrantScope> {
    match req.scope_type.as_str() {
        "tenant" => Ok(GrantScope::Tenant),
        "organization" => Ok(GrantScope::Organization(OrganizationId::from(
            req.scope_id
                .ok_or_else(|| CoreError::Validation("scope_id required".into()))?,
        ))),
        "team" => Ok(GrantScope::Team(TeamId::from(
            req.scope_id
                .ok_or_else(|| CoreError::Validation("scope_id required".into()))?,
        ))),
        "resource" => Ok(GrantScope::Resource {
            resource_type: req
                .resource_type
                .clone()
                .ok_or_else(|| CoreError::Validation("resource_type required".into()))?,
            resource_id: req
                .scope_id
                .ok_or_else(|| CoreError::Validation("scope_id (resource_id) required".into()))?,
        }),
        _ => Err(CoreError::Validation(
            "scope_type must be tenant|organization|team|resource".into(),
        )),
    }
}

/// 组织架构用例服务。
pub struct OrgService {
    orgs: Arc<dyn OrgRepository>,
    grants: Arc<dyn GrantRepository>,
}

impl OrgService {
    pub fn new(orgs: Arc<dyn OrgRepository>, grants: Arc<dyn GrantRepository>) -> Self {
        Self { orgs, grants }
    }

    pub async fn create_organization(
        &self,
        ctx: &RequestContext,
        req: CreateOrganizationRequest,
    ) -> CoreResult<Organization> {
        if req.slug.trim().is_empty() || req.name.trim().is_empty() {
            return Err(CoreError::Validation("slug/name must not be empty".into()));
        }
        self.orgs
            .create_organization(
                ctx,
                NewOrganization {
                    slug: req.slug,
                    name: req.name,
                },
            )
            .await
    }

    pub async fn list_organizations(&self, tenant: TenantId) -> CoreResult<Vec<Organization>> {
        self.orgs.list_organizations(tenant).await
    }

    pub async fn add_org_member(
        &self,
        ctx: &RequestContext,
        organization: OrganizationId,
        req: AddMemberRequest,
    ) -> CoreResult<()> {
        self.orgs
            .add_org_member(
                ctx,
                organization,
                UserId::from(req.user_id),
                parse_member_role(&req.role),
            )
            .await
    }

    pub async fn create_team(
        &self,
        ctx: &RequestContext,
        req: CreateTeamRequest,
    ) -> CoreResult<Team> {
        if req.slug.trim().is_empty() || req.name.trim().is_empty() {
            return Err(CoreError::Validation("slug/name must not be empty".into()));
        }
        self.orgs
            .create_team(
                ctx,
                NewTeam {
                    organization_id: req.organization_id,
                    slug: req.slug,
                    name: req.name,
                },
            )
            .await
    }

    pub async fn list_teams(
        &self,
        tenant: TenantId,
        organization: OrganizationId,
    ) -> CoreResult<Vec<Team>> {
        self.orgs.list_teams(tenant, organization).await
    }

    pub async fn add_team_member(
        &self,
        ctx: &RequestContext,
        team: TeamId,
        req: AddMemberRequest,
    ) -> CoreResult<()> {
        self.orgs
            .add_team_member(
                ctx,
                team,
                UserId::from(req.user_id),
                parse_member_role(&req.role),
            )
            .await
    }

    pub async fn grant_role(&self, ctx: &RequestContext, req: GrantRequest) -> CoreResult<()> {
        let input = RoleGrantInput {
            principal: parse_principal(&req.principal_type, req.principal_id)?,
            role_key: req.role_key.clone(),
            scope: parse_scope(&req)?,
        };
        self.grants.grant(ctx, input).await
    }

    pub async fn revoke_role(&self, ctx: &RequestContext, req: GrantRequest) -> CoreResult<()> {
        let input = RoleGrantInput {
            principal: parse_principal(&req.principal_type, req.principal_id)?,
            role_key: req.role_key.clone(),
            scope: parse_scope(&req)?,
        };
        self.grants.revoke(ctx, input).await
    }

    /// 解析用户在「租户 + 给定附加作用域」上的累积有效权限。
    pub async fn effective_permissions(
        &self,
        tenant: TenantId,
        user: UserId,
        scopes: &[GrantScope],
    ) -> CoreResult<Vec<String>> {
        self.grants
            .effective_permissions(tenant, user, scopes)
            .await
    }
}
