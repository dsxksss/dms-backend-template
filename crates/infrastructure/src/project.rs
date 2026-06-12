//! Project 仓储的 SQLx 实现（参考切片）。
//!
//! 写操作在**单个租户作用域事务**内完成：状态变更 + 审计日志 + 发件箱入队，
//! 三者原子提交。演示乐观锁（version）与软删（deleted_at）。

use async_trait::async_trait;
use dms_core::{CoreError, CoreResult, PageRequest, Paginated, RequestContext, TenantId};
use dms_domain::project::{
    NewProject, Project, ProjectEvent, ProjectId, ProjectRepository, UpdateProject,
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
}
