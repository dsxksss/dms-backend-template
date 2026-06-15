//! 初始管理员 bootstrap。
//!
//! 用于私有化首次部署创建第一个租户 + 管理员（拥有全部权限），也方便本地用密码
//! 登录联调。幂等：可重复执行（更新名称/密码）。

use dms_application::port::PasswordHasher;
use dms_core::{CoreError, CoreResult};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::map_db_error;
use crate::db::tenant::begin_tenant_tx;
use crate::iam::seed::seed_default_roles;

/// bootstrap 输入。
pub struct BootstrapInput<'a> {
    pub tenant_slug: &'a str,
    pub tenant_name: &'a str,
    pub email: &'a str,
    pub password: &'a str,
    /// 授予 bootstrap 用户的角色 key（须为已 seed 的角色，如 `owner`）。
    pub role_key: &'a str,
}

/// 创建/更新一个租户：seed 标准角色目录（owner/admin/member）+ 创建管理员用户，
/// 并把 `role_key` 指定的角色授予该用户。幂等可重复执行。
pub async fn bootstrap_tenant_admin(
    pool: &PgPool,
    hasher: &dyn PasswordHasher,
    input: BootstrapInput<'_>,
) -> CoreResult<()> {
    // 1) upsert 租户（tenants 无 RLS）。
    let tenant_id: Uuid = sqlx::query_scalar(
        "INSERT INTO tenants (slug, name) VALUES ($1, $2)
         ON CONFLICT (slug) DO UPDATE SET name = EXCLUDED.name
         RETURNING id",
    )
    .bind(input.tenant_slug)
    .bind(input.tenant_name)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    let password_hash = hasher.hash(input.password)?;

    let mut tx = begin_tenant_tx(pool, tenant_id).await?;

    // 2) seed 标准角色目录（owner/admin/member + 权限包）。幂等。
    seed_default_roles(&mut tx, tenant_id).await?;

    // 3) 解析要授予该用户的角色（须为已 seed 的角色）。
    let role_id: Uuid =
        sqlx::query_scalar("SELECT id FROM roles WHERE tenant_id = $1 AND key = $2")
            .bind(tenant_id)
            .bind(input.role_key)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_db_error)?
            .ok_or_else(|| {
                CoreError::NotFound(format!("seed role not found: {}", input.role_key))
            })?;

    // 4) upsert 管理员用户（带密码）。
    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (id, tenant_id, email, password_hash, display_name, status)
         VALUES ($1, $2, $3, $4, $5, 'active')
         ON CONFLICT (tenant_id, email) DO UPDATE SET password_hash = EXCLUDED.password_hash
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(tenant_id)
    .bind(input.email)
    .bind(&password_hash)
    .bind(input.email)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_db_error)?;

    // 5) 分配角色。
    sqlx::query(
        "INSERT INTO user_roles (tenant_id, user_id, role_id) VALUES ($1, $2, $3)
         ON CONFLICT DO NOTHING",
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(role_id)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    tx.commit().await.map_err(map_db_error)?;
    Ok(())
}
