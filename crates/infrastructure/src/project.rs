//! Project 仓储的 SQLx 实现（参考切片）。
//!
//! 写操作在**单个租户作用域事务**内完成：状态变更 + 审计日志 + 发件箱入队，
//! 三者原子提交。演示乐观锁（version）与软删（deleted_at）。

use async_trait::async_trait;
use dms_core::{CoreError, CoreResult, PageRequest, Paginated, RequestContext, TenantId, UserId};
use dms_domain::project::{
    NewProject, Project, ProjectEvent, ProjectId, ProjectMember, ProjectRepository, ProjectRole,
    UpdateProject,
};
use serde_json::json;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::audit;
use crate::db::map_db_error;
use crate::db::tenant::begin_tenant_tx;
use crate::outbox;

#[derive(sqlx::FromRow)]
struct ProjectRow {
    id: Uuid,
    tenant_id: Uuid,
    name: String,
    description: String,
    version: i32,
}

impl From<ProjectRow> for Project {
    fn from(r: ProjectRow) -> Self {
        Project {
            id: r.id.into(),
            tenant_id: r.tenant_id.into(),
            name: r.name,
            description: r.description,
            version: r.version,
        }
    }
}

const PROJECT_COLUMNS: &str = "id, tenant_id, name, description, version";

#[derive(sqlx::FromRow)]
struct MemberRow {
    project_id: Uuid,
    user_id: Uuid,
    role: String,
}

impl TryFrom<MemberRow> for ProjectMember {
    type Error = CoreError;
    fn try_from(r: MemberRow) -> Result<Self, Self::Error> {
        let role = ProjectRole::parse(&r.role)
            .ok_or_else(|| CoreError::internal(format!("invalid project role: {}", r.role)))?;
        Ok(ProjectMember {
            project_id: r.project_id.into(),
            user_id: r.user_id.into(),
            role,
        })
    }
}

pub struct PgProjectRepository {
    pool: PgPool,
}

impl PgProjectRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// 行未命中时区分"版本冲突"与"不存在"（在同一事务内再查一次）。
async fn conflict_or_not_found(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> CoreError {
    match sqlx::query_scalar::<_, i32>(
        "SELECT version FROM projects WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    {
        Ok(Some(_)) => CoreError::Conflict("version conflict (optimistic lock)".into()),
        Ok(None) => CoreError::NotFound("project not found".into()),
        Err(e) => map_db_error(e),
    }
}

/// 写一条事件到发件箱（payload 用领域事件 [`ProjectEvent`]）。
async fn enqueue_event(
    tx: &mut Transaction<'_, Postgres>,
    tenant: Uuid,
    event: &ProjectEvent,
    aggregate_id: Uuid,
) -> CoreResult<()> {
    let payload = serde_json::to_value(event)
        .map_err(|e| CoreError::internal(format!("serialize event: {e}")))?;
    outbox::enqueue(
        tx,
        tenant,
        "project",
        aggregate_id,
        event.event_type(),
        payload,
    )
    .await
}

#[async_trait]
impl ProjectRepository for PgProjectRepository {
    async fn create(&self, ctx: &RequestContext, input: NewProject) -> CoreResult<Project> {
        let tenant = Uuid::from(ctx.tenant_id);
        let actor = ctx.actor.user_id().map(Uuid::from);
        let mut tx = begin_tenant_tx(&self.pool, tenant).await?;

        let row = sqlx::query_as::<_, ProjectRow>(
            "INSERT INTO projects (id, tenant_id, name, description, created_by, updated_by)
             VALUES ($1, $2, $3, $4, $5, $5)
             RETURNING id, tenant_id, name, description, version",
        )
        .bind(Uuid::now_v7())
        .bind(tenant)
        .bind(&input.name)
        .bind(&input.description)
        .bind(actor)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?;

        // 创建者自动成为项目 owner（同事务，保证容器与属主原子建立）。
        if let Some(actor_id) = actor {
            sqlx::query(
                "INSERT INTO project_members (tenant_id, project_id, user_id, role, added_by)
                 VALUES ($1, $2, $3, 'owner', $3)",
            )
            .bind(tenant)
            .bind(row.id)
            .bind(actor_id)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;
        }

        audit::record(
            &mut tx,
            ctx,
            "created",
            "project",
            row.id,
            json!({ "name": &row.name, "description": &row.description }),
        )
        .await?;
        let event = ProjectEvent::Created {
            id: row.id.into(),
            name: row.name.clone(),
        };
        enqueue_event(&mut tx, tenant, &event, row.id).await?;

        tx.commit().await.map_err(map_db_error)?;
        Ok(row.into())
    }

    async fn get(&self, tenant: TenantId, id: ProjectId) -> CoreResult<Option<Project>> {
        let mut tx = begin_tenant_tx(&self.pool, Uuid::from(tenant)).await?;
        let row = sqlx::query_as::<_, ProjectRow>(&format!(
            "SELECT {PROJECT_COLUMNS} FROM projects WHERE id = $1 AND deleted_at IS NULL"
        ))
        .bind(Uuid::from(id))
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(row.map(Into::into))
    }

    async fn list(&self, tenant: TenantId, page: PageRequest) -> CoreResult<Paginated<Project>> {
        let page = page.clamped();
        let mut tx = begin_tenant_tx(&self.pool, Uuid::from(tenant)).await?;
        let rows = sqlx::query_as::<_, ProjectRow>(&format!(
            "SELECT {PROJECT_COLUMNS} FROM projects WHERE deleted_at IS NULL
             ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        ))
        .bind(page.limit)
        .bind(page.offset)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db_error)?;
        let total: i64 =
            sqlx::query_scalar("SELECT count(*) FROM projects WHERE deleted_at IS NULL")
                .fetch_one(&mut *tx)
                .await
                .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(Paginated::new(
            rows.into_iter().map(Into::into).collect(),
            total,
            page,
        ))
    }

    async fn update(
        &self,
        ctx: &RequestContext,
        id: ProjectId,
        input: UpdateProject,
    ) -> CoreResult<Project> {
        let tenant = Uuid::from(ctx.tenant_id);
        let actor = ctx.actor.user_id().map(Uuid::from);
        let mut tx = begin_tenant_tx(&self.pool, tenant).await?;

        let updated = sqlx::query_as::<_, ProjectRow>(
            "UPDATE projects
             SET name = COALESCE($2, name), description = COALESCE($3, description),
                 version = version + 1, updated_by = $4
             WHERE id = $1 AND version = $5 AND deleted_at IS NULL
             RETURNING id, tenant_id, name, description, version",
        )
        .bind(Uuid::from(id))
        .bind(input.name)
        .bind(input.description)
        .bind(actor)
        .bind(input.expected_version)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?;

        let row = match updated {
            Some(r) => r,
            None => return Err(conflict_or_not_found(&mut tx, Uuid::from(id)).await),
        };

        audit::record(
            &mut tx,
            ctx,
            "updated",
            "project",
            row.id,
            json!({ "name": &row.name, "description": &row.description, "version": row.version }),
        )
        .await?;
        let event = ProjectEvent::Updated {
            id: row.id.into(),
            version: row.version,
        };
        enqueue_event(&mut tx, tenant, &event, row.id).await?;

        tx.commit().await.map_err(map_db_error)?;
        Ok(row.into())
    }

    async fn delete(
        &self,
        ctx: &RequestContext,
        id: ProjectId,
        expected_version: i32,
    ) -> CoreResult<()> {
        let tenant = Uuid::from(ctx.tenant_id);
        let actor = ctx.actor.user_id().map(Uuid::from);
        let pid = Uuid::from(id);
        let mut tx = begin_tenant_tx(&self.pool, tenant).await?;

        let deleted = sqlx::query_scalar::<_, Uuid>(
            "UPDATE projects SET deleted_at = now(), version = version + 1, updated_by = $3
             WHERE id = $1 AND version = $2 AND deleted_at IS NULL
             RETURNING id",
        )
        .bind(pid)
        .bind(expected_version)
        .bind(actor)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?;

        if deleted.is_none() {
            return Err(conflict_or_not_found(&mut tx, pid).await);
        }

        audit::record(&mut tx, ctx, "deleted", "project", pid, json!({})).await?;
        let event = ProjectEvent::Deleted { id };
        enqueue_event(&mut tx, tenant, &event, pid).await?;

        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    async fn list_members(
        &self,
        tenant: TenantId,
        project: ProjectId,
    ) -> CoreResult<Vec<ProjectMember>> {
        let mut tx = begin_tenant_tx(&self.pool, Uuid::from(tenant)).await?;
        let rows = sqlx::query_as::<_, MemberRow>(
            "SELECT project_id, user_id, role FROM project_members
             WHERE project_id = $1 ORDER BY added_at",
        )
        .bind(Uuid::from(project))
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        rows.into_iter().map(ProjectMember::try_from).collect()
    }

    async fn member_role(
        &self,
        tenant: TenantId,
        project: ProjectId,
        user: UserId,
    ) -> CoreResult<Option<ProjectRole>> {
        let mut tx = begin_tenant_tx(&self.pool, Uuid::from(tenant)).await?;
        let role: Option<String> = sqlx::query_scalar(
            "SELECT role FROM project_members WHERE project_id = $1 AND user_id = $2",
        )
        .bind(Uuid::from(project))
        .bind(Uuid::from(user))
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        role.map(|r| {
            ProjectRole::parse(&r)
                .ok_or_else(|| CoreError::internal(format!("invalid project role: {r}")))
        })
        .transpose()
    }

    async fn upsert_member(
        &self,
        ctx: &RequestContext,
        project: ProjectId,
        user: UserId,
        role: ProjectRole,
    ) -> CoreResult<ProjectMember> {
        let tenant = Uuid::from(ctx.tenant_id);
        let actor = ctx.actor.user_id().map(Uuid::from);
        let pid = Uuid::from(project);
        let mut tx = begin_tenant_tx(&self.pool, tenant).await?;

        // 项目须存在且未软删，否则 NotFound（FK 不约束软删状态）。
        let exists = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM projects WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(pid)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?;
        if exists.is_none() {
            return Err(CoreError::NotFound("project not found".into()));
        }

        sqlx::query(
            "INSERT INTO project_members (tenant_id, project_id, user_id, role, added_by)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (project_id, user_id) DO UPDATE SET role = EXCLUDED.role",
        )
        .bind(tenant)
        .bind(pid)
        .bind(Uuid::from(user))
        .bind(role.as_str())
        .bind(actor)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

        audit::record(
            &mut tx,
            ctx,
            "member_set",
            "project",
            pid,
            json!({ "user_id": Uuid::from(user), "role": role.as_str() }),
        )
        .await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(ProjectMember {
            project_id: project,
            user_id: user,
            role,
        })
    }

    async fn remove_member(
        &self,
        ctx: &RequestContext,
        project: ProjectId,
        user: UserId,
    ) -> CoreResult<()> {
        let tenant = Uuid::from(ctx.tenant_id);
        let pid = Uuid::from(project);
        let uid = Uuid::from(user);
        let mut tx = begin_tenant_tx(&self.pool, tenant).await?;

        let role: Option<String> = sqlx::query_scalar(
            "SELECT role FROM project_members WHERE project_id = $1 AND user_id = $2",
        )
        .bind(pid)
        .bind(uid)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?;
        let Some(role) = role else {
            return Err(CoreError::NotFound("member not found".into()));
        };

        // 不允许移除最后一名 owner（避免项目失去属主）。
        if role == "owner" {
            let owners: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM project_members WHERE project_id = $1 AND role = 'owner'",
            )
            .bind(pid)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_db_error)?;
            if owners <= 1 {
                return Err(CoreError::Conflict("cannot remove the last owner".into()));
            }
        }

        sqlx::query("DELETE FROM project_members WHERE project_id = $1 AND user_id = $2")
            .bind(pid)
            .bind(uid)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;

        audit::record(
            &mut tx,
            ctx,
            "member_removed",
            "project",
            pid,
            json!({ "user_id": uid }),
        )
        .await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }
}
