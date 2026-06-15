//! # dms-infrastructure — 基础设施层
//!
//! 实现 `domain`/`application` 定义的端口：SQLx 连接池与迁移、RLS 租户作用域、
//! 认证 provider、仓储等。是唯一接触外部系统的层。
//!
//! 内容按 Cargo feature 分档：
//! - `database`：[`db`]（池/迁移/错误映射）、[`health`]（DB 就绪探针）。
//! - `multi-tenancy`：[`db::tenant`]（租户作用域事务）。
//! - `auth`：[`auth`]（密码/JWT/平台 provider）、[`iam`]（IAM 仓储 + bootstrap）。

#[cfg(feature = "database")]
pub mod db;
#[cfg(feature = "database")]
pub mod health;

#[cfg(feature = "auth")]
pub mod auth;
#[cfg(feature = "auth")]
pub mod iam;

#[cfg(feature = "audit")]
pub mod audit;
#[cfg(feature = "orgs")]
pub mod orgs;
#[cfg(feature = "audit")]
pub mod outbox;
#[cfg(feature = "project")]
pub mod project;
