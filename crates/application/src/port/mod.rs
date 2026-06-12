//! 应用层端口（trait）。
//!
//! 这些 trait 定义应用层对外部能力的需求；具体实现在 `infrastructure`，
//! 由 `bin/server` 组合根注入。API/应用层只依赖这些抽象，便于替换与测试。

#[cfg(feature = "auth")]
mod auth;
mod health;

#[cfg(feature = "auth")]
pub use auth::{AccessClaims, IdentityProvider, PasswordHasher, TokenIssuer, VerifiedIdentity};
pub use health::{AlwaysReady, HealthProbe};
