//! 事务性发件箱：入队 + 后台中继。
//!
//! 领域事件与状态变更在同一事务落 `outbox`，后台中继可靠投递（不丢事件）。

use std::time::Duration;

use dms_core::CoreResult;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::db::map_db_error;

/// 在事务内把一个领域事件写入发件箱。
pub async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    aggregate_type: &str,
    aggregate_id: Uuid,
    event_type: &str,
    payload: serde_json::Value,
) -> CoreResult<()> {
    sqlx::query(
        "INSERT INTO outbox
            (id, tenant_id, aggregate_type, aggregate_id, event_type, payload)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(Uuid::now_v7())
    .bind(tenant_id)
    .bind(aggregate_type)
    .bind(aggregate_id)
    .bind(event_type)
    .bind(sqlx::types::Json(payload))
    .execute(&mut **tx)
    .await
    .map_err(map_db_error)?;
    Ok(())
}

/// 发件箱中继循环：用拥有者连接（绕过 RLS）轮询未处理事件，投递后标记已处理。
///
/// 模板里"投递"仅记录日志；真实场景替换为消息队列 / Webhook / 集成回调。
pub async fn run_relay(owner_pool: PgPool, interval: Duration) {
    tracing::info!("outbox relay started");
    loop {
        match process_batch(&owner_pool).await {
            Ok(n) if n > 0 => tracing::info!(count = n, "outbox relay dispatched events"),
            Ok(_) => {}
            Err(e) => tracing::error!(error = %e, "outbox relay batch failed"),
        }
        tokio::time::sleep(interval).await;
    }
}

async fn process_batch(pool: &PgPool) -> CoreResult<u64> {
    let rows = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT id, event_type, aggregate_type FROM outbox
         WHERE processed_at IS NULL ORDER BY occurred_at LIMIT 100",
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    let mut dispatched = 0u64;
    for (id, event_type, aggregate_type) in rows {
        // TODO(集成): 投递到消息队列/Webhook。模板仅记录并标记已处理。
        tracing::info!(%id, %event_type, %aggregate_type, "dispatching domain event");
        sqlx::query("UPDATE outbox SET processed_at = now() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await
            .map_err(map_db_error)?;
        dispatched += 1;
    }
    Ok(dispatched)
}
