//! `/v1/projects` CRUD —— Project 参考切片的 HTTP 入口。
//!
//! 每个 handler：RBAC 守卫（`auth.require`）→ 调用用例服务（传 `RequestContext`）。

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use dms_application::project::{
    AddMemberRequest, CreateProjectRequest, MemberResponse, ProjectResponse, UpdateProjectRequest,
};
use dms_core::{PageRequest, Paginated, UserId};
use dms_domain::project::ProjectId;
use serde::Deserialize;

use crate::error::ApiResult;
use crate::extract::AuthContext;
use crate::state::AppState;

/// 软删除需带期望版本（乐观锁）：`DELETE /v1/projects/{id}?version=N`。
#[derive(Debug, Deserialize)]
pub struct DeleteQuery {
    pub version: i32,
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<CreateProjectRequest>,
) -> ApiResult<(StatusCode, Json<ProjectResponse>)> {
    auth.require("project:write")?;
    let project = state.projects.create(&auth.ctx, req).await?;
    Ok((StatusCode::CREATED, Json(project.into())))
}

pub async fn get(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<ProjectId>,
) -> ApiResult<Json<ProjectResponse>> {
    auth.require("project:read")?;
    let project = state.projects.get(auth.ctx.tenant_id, id).await?;
    Ok(Json(project.into()))
}

pub async fn list(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(page): Query<PageRequest>,
) -> ApiResult<Json<Paginated<ProjectResponse>>> {
    auth.require("project:read")?;
    let projects = state.projects.list(auth.ctx.tenant_id, page).await?;
    Ok(Json(projects.map(ProjectResponse::from)))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<ProjectId>,
    Json(req): Json<UpdateProjectRequest>,
) -> ApiResult<Json<ProjectResponse>> {
    auth.require("project:write")?;
    let project = state.projects.update(&auth.ctx, id, req).await?;
    Ok(Json(project.into()))
}

pub async fn delete(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<ProjectId>,
    Query(q): Query<DeleteQuery>,
) -> ApiResult<StatusCode> {
    auth.require("project:write")?;
    state.projects.delete(&auth.ctx, id, q.version).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_members(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<ProjectId>,
) -> ApiResult<Json<Vec<MemberResponse>>> {
    auth.require("project:read")?;
    let members = state.projects.list_members(auth.ctx.tenant_id, id).await?;
    Ok(Json(
        members.into_iter().map(MemberResponse::from).collect(),
    ))
}

pub async fn add_member(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<ProjectId>,
    Json(req): Json<AddMemberRequest>,
) -> ApiResult<(StatusCode, Json<MemberResponse>)> {
    auth.require("project:write")?;
    let member = state.projects.add_member(&auth.ctx, id, req).await?;
    Ok((StatusCode::CREATED, Json(member.into())))
}

pub async fn remove_member(
    State(state): State<AppState>,
    auth: AuthContext,
    Path((id, user_id)): Path<(ProjectId, UserId)>,
) -> ApiResult<StatusCode> {
    auth.require("project:write")?;
    state.projects.remove_member(&auth.ctx, id, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
