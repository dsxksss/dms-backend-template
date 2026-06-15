//! # dms-application — 应用层
//!
//! 用例编排、命令/查询、DTO、事务边界。依赖 `domain` 定义的端口（trait），
//! 不关心具体实现（SQLx/JWT 等由基础设施在组合根注入）。
//!
//! 内容按 Cargo feature 分档：
//! - 始终提供 [`port`]（通用能力端口，如 `HealthProbe`/`AlwaysReady`）。
//! - `auth`：[`auth::AuthService`] 身份联合与会话用例、[`dto`]、[`token`]。
//!
//! `Project` 用例随 M4 加入。

#[cfg(feature = "auth")]
pub mod auth;
#[cfg(feature = "auth")]
pub mod dto;
#[cfg(feature = "orgs")]
pub mod orgs;
pub mod port;
#[cfg(feature = "project")]
pub mod project;
#[cfg(feature = "auth")]
pub mod token;
