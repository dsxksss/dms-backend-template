//! 请求上下文：贯穿一次请求处理链路的身份与追踪信息。
//!
//! 由 API 中间件构造（解析 JWT → 租户 + 操作者，生成/透传 request_id），
//! 向下传递给应用/基础设施层：
//! - `tenant_id` → 设置 Postgres RLS 会话变量，实现行级隔离；
//! - `actor` → 写入审计日志（谁做的）；
//! - `request_id` → 串联日志/链路与审计记录。

use uuid::Uuid;

use crate::ids::{TenantId, UserId};

/// 操作主体。区分真实用户与系统内部动作（迁移、后台任务等）。
#[derive(Debug, Clone)]
pub enum Actor {
    /// 已认证用户。
    User { user_id: UserId },
    /// 系统/后台动作（无登录用户）。
    System,
}

impl Actor {
    /// 取操作用户 ID（系统动作返回 `None`）。
    pub fn user_id(&self) -> Option<UserId> {
        match self {
            Actor::User { user_id } => Some(*user_id),
            Actor::System => None,
        }
    }
}

/// 一次请求的上下文。克隆成本低（仅几个 Copy 字段）。
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// 请求关联 ID，用于日志/链路/审计串联。
    pub request_id: Uuid,
    /// 当前租户（RLS 隔离边界）。
    pub tenant_id: TenantId,
    /// 操作主体（审计归属）。
    pub actor: Actor,
}

impl RequestContext {
    /// 构造用户请求上下文。
    pub fn for_user(request_id: Uuid, tenant_id: TenantId, user_id: UserId) -> Self {
        Self {
            request_id,
            tenant_id,
            actor: Actor::User { user_id },
        }
    }

    /// 构造系统/后台上下文（如 outbox 中继、定时任务）。
    pub fn for_system(tenant_id: TenantId) -> Self {
        Self {
            request_id: Uuid::now_v7(),
            tenant_id,
            actor: Actor::System,
        }
    }
}
