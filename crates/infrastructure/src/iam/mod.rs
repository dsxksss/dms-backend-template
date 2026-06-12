//! 身份相关的 SQLx 仓储与初始管理员 bootstrap。

mod bootstrap;
mod repos;

pub use bootstrap::{BootstrapInput, bootstrap_tenant_admin};
pub use repos::{
    PgExternalIdentityRepository, PgRefreshTokenRepository, PgTenantRepository, PgUserRepository,
};
