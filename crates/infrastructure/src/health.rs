//! 就绪探针实现（数据库连通性）。

use async_trait::async_trait;
use dms_application::port::HealthProbe;
use sqlx::PgPool;

use crate::db;

/// 基于数据库 `ping` 的就绪探针。实现 application 层定义的 [`HealthProbe`] 端口，
/// 由组合根注入 API。
pub struct DbHealthProbe {
    pool: PgPool,
}

impl DbHealthProbe {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl HealthProbe for DbHealthProbe {
    async fn ready(&self) -> bool {
        db::ping(&self.pool).await
    }
}
