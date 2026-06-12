//! RLS 租户作用域。
//!
//! 所有访问租户数据的操作都应在「已绑定租户的事务」内进行：事务开头设置
//! `app.current_tenant`（事务局部），之后所有查询自动受行级安全策略约束，
//! 只能看到/改动该租户的数据。这是多租户隔离的执行点。

use dms_core::CoreError;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::map_db_error;

/// 开启一个绑定到 `tenant_id` 的事务。
///
/// `set_config(.., true)` 等价于 `SET LOCAL`，仅在本事务有效，事务结束自动还原，
/// 因此连接归还连接池后不会泄露租户上下文。
pub async fn begin_tenant_tx(
    pool: &PgPool,
    tenant_id: Uuid,
) -> Result<Transaction<'_, Postgres>, CoreError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;

    sqlx::query("SELECT set_config('app.current_tenant', $1, true)")
        .bind(tenant_id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

    Ok(tx)
}
