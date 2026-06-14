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

#[cfg(test)]
mod tests {
    //! 用内存 mock 仓储测试服务编排——无需数据库，证明端口/DI 设计的可测性。
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    use async_trait::async_trait;
    use dms_core::{TenantId, UserId};

    #[derive(Default)]
    struct MockProjectRepo {
        items: Mutex<HashMap<uuid::Uuid, Project>>,
    }

    #[async_trait]
    impl ProjectRepository for MockProjectRepo {
        async fn create(&self, ctx: &RequestContext, input: NewProject) -> CoreResult<Project> {
            let project = Project {
                id: ProjectId::new(),
                tenant_id: ctx.tenant_id,
                name: input.name,
                description: input.description,
                version: 1,
            };
            self.items
                .lock()
                .unwrap()
                .insert(project.id.as_uuid(), project.clone());
            Ok(project)
        }

        async fn get(&self, _tenant: TenantId, id: ProjectId) -> CoreResult<Option<Project>> {
            Ok(self.items.lock().unwrap().get(&id.as_uuid()).cloned())
        }

        async fn list(
            &self,
            _tenant: TenantId,
            page: PageRequest,
        ) -> CoreResult<Paginated<Project>> {
            let items: Vec<Project> = self.items.lock().unwrap().values().cloned().collect();
            let total = items.len() as i64;
            Ok(Paginated::new(items, total, page))
        }

        async fn update(
            &self,
            _ctx: &RequestContext,
            id: ProjectId,
            input: UpdateProject,
        ) -> CoreResult<Project> {
            let mut guard = self.items.lock().unwrap();
            let project = guard
                .get_mut(&id.as_uuid())
                .ok_or_else(|| CoreError::NotFound("not found".into()))?;
            if project.version != input.expected_version {
                return Err(CoreError::Conflict("version".into()));
            }
            if let Some(name) = input.name {
                project.name = name;
            }
            if let Some(description) = input.description {
                project.description = description;
            }
            project.version += 1;
            Ok(project.clone())
        }

        async fn delete(
            &self,
            _ctx: &RequestContext,
            id: ProjectId,
            expected_version: i32,
        ) -> CoreResult<()> {
            let mut guard = self.items.lock().unwrap();
            let project = guard
                .get(&id.as_uuid())
                .ok_or_else(|| CoreError::NotFound("not found".into()))?;
            if project.version != expected_version {
                return Err(CoreError::Conflict("version".into()));
            }
            guard.remove(&id.as_uuid());
            Ok(())
        }
    }

    fn ctx() -> RequestContext {
        RequestContext::for_user(uuid::Uuid::now_v7(), TenantId::new(), UserId::new())
    }

    fn service() -> ProjectService {
        ProjectService::new(Arc::new(MockProjectRepo::default()))
    }

    #[tokio::test]
    async fn create_rejects_empty_name() {
        let err = service()
            .create(
                &ctx(),
                CreateProjectRequest {
                    name: "   ".into(),
                    description: String::new(),
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(err, CoreError::Validation(_)));
    }

    #[tokio::test]
    async fn crud_and_optimistic_lock() {
        let svc = service();
        let c = ctx();

        let created = svc
            .create(
                &c,
                CreateProjectRequest {
                    name: "Alpha".into(),
                    description: "d".into(),
                },
            )
            .await
            .unwrap();
        assert_eq!(created.version, 1);

        let fetched = svc.get(c.tenant_id, created.id).await.unwrap();
        assert_eq!(fetched.name, "Alpha");

        let updated = svc
            .update(
                &c,
                created.id,
                UpdateProjectRequest {
                    name: Some("Beta".into()),
                    description: None,
                    version: 1,
                },
            )
            .await
            .unwrap();
        assert_eq!(updated.version, 2);
        assert_eq!(updated.name, "Beta");

        // 旧版本更新 → 冲突。
        let conflict = svc
            .update(
                &c,
                created.id,
                UpdateProjectRequest {
                    name: None,
                    description: None,
                    version: 1,
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(conflict, CoreError::Conflict(_)));

        // 旧版本删除 → 冲突；正确版本 → 成功；删后查不到。
        assert!(matches!(
            svc.delete(&c, created.id, 1).await.unwrap_err(),
            CoreError::Conflict(_)
        ));
        svc.delete(&c, created.id, 2).await.unwrap();
        assert!(matches!(
            svc.get(c.tenant_id, created.id).await.unwrap_err(),
            CoreError::NotFound(_)
        ));
    }
}
