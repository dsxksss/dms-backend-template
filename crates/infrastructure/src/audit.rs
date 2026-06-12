//! 审计日志写入。
//!
//! 在业务写操作的**同一事务**内调用，保证状态变更与审计记录原子落库。

use dms_core::{CoreResult, RequestContext};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::db::map_db_error;

/// 在事务内记录一条审计日志。
pub async fn record(
    tx: &mut Transaction<'_, Postgres>,
    ctx: &RequestContext,
    action: &str,
    entity_type: &str,
    entity_id: Uuid,
    changes: serde_json::Value,
) -> CoreResult<()> {
    sqlx::query(
        "INSERT INTO audit_log
            (id, tenant_id, actor_id, action, entity_type, entity_id, changes, request_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(Uuid::now_v7())
    .bind(Uuid::from(ctx.tenant_id))
    .bind(ctx.actor.user_id().map(Uuid::from))
    .bind(action)
    .bind(entity_type)
    .bind(entity_id)
    .bind(sqlx::types::Json(changes))
    .bind(ctx.request_id)
    .execute(&mut **tx)
    .await
    .map_err(map_db_error)?;
    Ok(())
}
