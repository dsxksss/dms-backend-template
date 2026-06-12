use async_trait::async_trait;

/// 就绪探针端口：判断关键依赖（数据库等）是否就绪。
///
/// 由基础设施实现（如检查 DB 连通性），供 `/readyz` 使用。
#[async_trait]
pub trait HealthProbe: Send + Sync {
    /// 返回是否就绪。
    async fn ready(&self) -> bool;
}

/// 永远就绪的探针。精简档（无数据库）用它，使 `/readyz` 始终返回就绪。
pub struct AlwaysReady;

#[async_trait]
impl HealthProbe for AlwaysReady {
    async fn ready(&self) -> bool {
        true
    }
}
