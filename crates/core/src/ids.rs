//! 跨层身份 ID 的类型标记与别名。
//!
//! 租户与用户是贯穿全栈（RLS、审计、上下文）的身份概念，其标记放在 core；
//! 具体业务实体（如 `Project`）的 ID 标记放在各自的 domain 模块。

use crate::id::Id;

/// 空枚举标记，仅用于参数化 [`Id`]，无运行时表示。
pub mod markers {
    /// 租户标记。
    pub enum Tenant {}
    /// 用户标记。
    pub enum User {}
}

/// 租户 ID。
pub type TenantId = Id<markers::Tenant>;

/// 用户 ID。
pub type UserId = Id<markers::User>;
