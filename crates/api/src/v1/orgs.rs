//! `/v1` 组织架构端点：组织/团队/成员/角色授予 + 当前用户有效权限解析。

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use dms_application::orgs::{
    AddMemberRequest, CreateOrganizationRequest, CreateTeamRequest, GrantRequest,
    OrganizationResponse, TeamResponse,
};
use dms_core::CoreError;
use dms_domain::orgs::{GrantScope, OrganizationId, TeamId};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::extract::AuthContext;
use crate::state::AppState;

pub async fn create_org(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<CreateOrganizationRequest>,
) -> ApiResult<(StatusCode, Json<OrganizationResponse>)> {
    auth.require("org:write")?;
    let org = state.orgs.create_organization(&auth.ctx, req).await?;
    Ok((StatusCode::CREATED, Json(org.into())))
}

pub async fn list_orgs(
    State(state): State<AppState>,
    auth: AuthContext,
) -> ApiResult<Json<Vec<OrganizationResponse>>> {
    auth.require("org:read")?;
    let orgs = state.orgs.list_organizations(auth.ctx.tenant_id).await?;
    Ok(Json(orgs.into_iter().map(Into::into).collect()))
}

pub async fn add_org_member(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(org_id): Path<OrganizationId>,
    Json(req): Json<AddMemberRequest>,
) -> ApiResult<StatusCode> {
    auth.require("org:write")?;
    state.orgs.add_org_member(&auth.ctx, org_id, req).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn create_team(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<CreateTeamRequest>,
) -> ApiResult<(StatusCode, Json<TeamResponse>)> {
    auth.require("org:write")?;
    let team = state.orgs.create_team(&auth.ctx, req).await?;
    Ok((StatusCode::CREATED, Json(team.into())))
}

pub async fn list_teams(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(org_id): Path<OrganizationId>,
) -> ApiResult<Json<Vec<TeamResponse>>> {
    auth.require("org:read")?;
    let teams = state.orgs.list_teams(auth.ctx.tenant_id, org_id).await?;
    Ok(Json(teams.into_iter().map(Into::into).collect()))
}

pub async fn add_team_member(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(team_id): Path<TeamId>,
    Json(req): Json<AddMemberRequest>,
) -> ApiResult<StatusCode> {
    auth.require("org:write")?;
    state.orgs.add_team_member(&auth.ctx, team_id, req).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn grant_role(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<GrantRequest>,
) -> ApiResult<StatusCode> {
    auth.require("org:write")?;
    state.orgs.grant_role(&auth.ctx, req).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn revoke_role(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<GrantRequest>,
) -> ApiResult<StatusCode> {
    auth.require("org:write")?;
    state.orgs.revoke_role(&auth.ctx, req).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct PermQuery {
    #[serde(default)]
    pub organization: Option<Uuid>,
    #[serde(default)]
    pub resource_type: Option<String>,
    #[serde(default)]
    pub resource_id: Option<Uuid>,
}

/// 当前用户在「租户 + 可选附加作用域」上的累积有效权限（演示作用域解析）。
pub async fn my_permissions(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<PermQuery>,
) -> ApiResult<Json<Value>> {
    let user = auth
        .ctx
        .actor
        .user_id()
        .ok_or(ApiError(CoreError::Unauthorized))?;

    let mut scopes: Vec<GrantScope> = Vec::new();
    if let Some(org) = q.organization {
        scopes.push(GrantScope::Organization(OrganizationId::from(org)));
    }
    if let (Some(rt), Some(rid)) = (q.resource_type, q.resource_id) {
        scopes.push(GrantScope::Resource {
            resource_type: rt,
            resource_id: rid,
        });
    }

    let perms = state
        .orgs
        .effective_permissions(auth.ctx.tenant_id, user, &scopes)
        .await?;
    Ok(Json(json!({ "permissions": perms })))
}
