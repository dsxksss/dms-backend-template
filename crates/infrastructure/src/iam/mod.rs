//! 身份相关的 SQLx 仓储与初始管理员 bootstrap。

mod bootstrap;
mod repos;
mod seed;

pub use bootstrap::{BootstrapInput, bootstrap_tenant_admin};
pub use repos::{
    PgExternalIdentityRepository, PgRefreshTokenRepository, PgTenantRepository, PgUserRepository,
};
pub use seed::{DEFAULT_ROLES, PermissionBundle, RoleSeed, seed_default_roles};
