//! # dms-core — 共享内核
//!
//! 跨层复用的基础设施：统一错误模型、强类型 ID、分页、请求/租户上下文、
//! 可观测性初始化。**不依赖任何 I/O**，被所有上层 crate 复用。
//!
//! 分层约束见 `docs/architecture.md`：依赖只能指向内层，core 是最内层。

pub mod context;
pub mod error;
pub mod id;
pub mod ids;
pub mod pagination;
pub mod telemetry;

pub use context::{Actor, RequestContext};
pub use error::{CoreError, CoreResult};
pub use id::Id;
pub use ids::{TenantId, UserId};
pub use pagination::{PageRequest, Paginated};
