//! 真实数据库集成测试示例（四层测试中的「集成」层）。
//!
//! 默认 `#[ignore]`，不依赖 Docker、不阻塞无库 CI。需要时提供连接串运行：
//!
//! ```bash
//! TEST_DATABASE_URL=postgres://dms:dms@127.0.0.1:5433/dms \
//!   cargo test -p dms-infrastructure --features project -- --ignored
//! ```
//!
//! 用拥有者角色连接串（迁移需要 DDL 权限）。这是编写 testcontainers / `sqlx::test`
//! 真实集成测试的起点范本。

#![cfg(feature = "project")]

use dms_infrastructure::db;

fn test_db_url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()
}

#[tokio::test]
#[ignore = "需要 PostgreSQL；设 TEST_DATABASE_URL 后用 --ignored 运行"]
async fn migrations_apply_and_db_reachable() {
    let url = test_db_url().expect("set TEST_DATABASE_URL (owner role) to run this test");

    // 按启用档顺序执行全部迁移（core→tenancy→auth→audit→project）。
    db::run_migrations(&url)
        .await
        .expect("migrations should apply");

    // 连接并 ping。
    let pool = db::create_pool_from_url(&url, 1)
        .await
        .expect("pool should connect");
    assert!(db::ping(&pool).await, "ping should succeed");
}
