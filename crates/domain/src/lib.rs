//! # dms-domain — 领域层
//!
//! 实体、值对象、领域事件、仓储端口（trait）。**纯业务，无 I/O**——不依赖
//! sqlx/axum 等基础设施，便于单元测试与长期演进。
//!
//! 内容按 Cargo feature 分档启用：
//! - `auth`：[`iam`] 身份模型 + [`ports`] 仓储端口。
//!
//! `Project` 参考切片随 M4（`project` feature）加入。

#[cfg(feature = "auth")]
pub mod iam;
#[cfg(feature = "orgs")]
pub mod orgs;
#[cfg(feature = "auth")]
pub mod ports;
#[cfg(feature = "project")]
pub mod project;
