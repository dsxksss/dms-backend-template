//! Project 用例服务 + API DTO。
//!
//! 编排校验与仓储调用；审计/发件箱写入由仓储在事务内完成（见
//! `dms_domain::project::ProjectRepository` 的契约）。

use std::sync::Arc;

use dms_core::{CoreError, CoreResult, PageRequest, Paginated, RequestContext, TenantId};
use dms_domain::project::{NewProject, Project, ProjectId, ProjectRepository, UpdateProject};
use serde::{Deserialize, Serialize};

/// 创建项目请求。
#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

/// 更新项目请求（部分字段 + 乐观锁版本）。
#[derive(Debug, Deserialize)]
pub struct UpdateProjectRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// 期望的当前版本（乐观锁）。
    pub version: i32,
}

/// 项目响应（不暴露 tenant_id）。
#[derive(Debug, Serialize)]
pub struct ProjectResponse {
    pub id: ProjectId,
    pub name: String,
    pub description: String,
    pub version: i32,
}

impl From<Project> for ProjectResponse {
    fn from(p: Project) -> Self {
        Self {
            id: p.id,
            name: p.name,
            description: p.description,
            version: p.version,
        }
    }
}

/// 项目用例服务。
pub struct ProjectService {
    repo: Arc<dyn ProjectRepository>,
}

impl ProjectService {
    pub fn new(repo: Arc<dyn ProjectRepository>) -> Self {
        Self { repo }
    }

    fn validate_name(name: &str) -> CoreResult<()> {
        let len = name.trim().chars().count();
        if len == 0 {
            return Err(CoreError::Validation("name must not be empty".into()));
        }
        if len > 200 {
            return Err(CoreError::Validation("name too long (max 200)".into()));
        }
        Ok(())
    }

    pub async fn create(
        &self,
        ctx: &RequestContext,
        req: CreateProjectRequest,
    ) -> CoreResult<Project> {
        Self::validate_name(&req.name)?;
        self.repo
            .create(
                ctx,
                NewProject {
                    name: req.name,
                    description: req.description,
                },
            )
            .await
    }

    pub async fn get(&self, tenant: TenantId, id: ProjectId) -> CoreResult<Project> {
        self.repo
            .get(tenant, id)
            .await?
            .ok_or_else(|| CoreError::NotFound("project not found".into()))
    }

    pub async fn list(
        &self,
        tenant: TenantId,
        page: PageRequest,
    ) -> CoreResult<Paginated<Project>> {
        self.repo.list(tenant, page).await
    }

    pub async fn update(
        &self,
        ctx: &RequestContext,
        id: ProjectId,
        req: UpdateProjectRequest,
    ) -> CoreResult<Project> {
        if let Some(name) = &req.name {
            Self::validate_name(name)?;
        }
        self.repo
            .update(
                ctx,
                id,
                UpdateProject {
                    name: req.name,
                    description: req.description,
                    expected_version: req.version,
                },
            )
            .await
    }

    pub async fn delete(
        &self,
        ctx: &RequestContext,
        id: ProjectId,
        expected_version: i32,
    ) -> CoreResult<()> {
        self.repo.delete(ctx, id, expected_version).await
    }
}
