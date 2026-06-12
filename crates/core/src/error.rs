//! 统一错误模型。
//!
//! [`CoreError`] 是贯穿各层的规范业务错误分类：领域/应用层产生它，
//! 基础设施层把底层错误（如 `sqlx::Error`）映射成它，API 层再把它映射成
//! HTTP 响应（RFC 7807 Problem Details）。这样错误语义在全栈一致，
//! 新增端点无需重复设计错误处理。

/// 规范业务错误分类。
///
/// 刻意保持小而稳定——每个变体对应一类 HTTP 语义。需要携带更多结构化
/// 信息时，优先在 `detail` 字符串里表达，避免错误类型爆炸。
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// 资源不存在 → 404。
    #[error("not found: {0}")]
    NotFound(String),

    /// 唯一约束/状态冲突/乐观锁失败 → 409。
    #[error("conflict: {0}")]
    Conflict(String),

    /// 输入校验失败 → 422。
    #[error("validation failed: {0}")]
    Validation(String),

    /// 未认证（缺少/无效凭证）→ 401。
    #[error("unauthorized")]
    Unauthorized,

    /// 已认证但无权限 → 403。
    #[error("forbidden")]
    Forbidden,

    /// 未预期的内部错误 → 500。携带的信息只用于日志，不直接回传客户端。
    #[error("internal error: {0}")]
    Internal(String),
}

impl CoreError {
    /// 便捷构造内部错误。
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    /// 该错误是否属于客户端可见、可安全回显 detail 的类别。
    /// 内部错误不回显细节，避免泄露实现信息。
    pub fn is_client_safe(&self) -> bool {
        !matches!(self, CoreError::Internal(_))
    }
}

/// 全栈统一的 `Result` 别名。
pub type CoreResult<T> = Result<T, CoreError>;
