//! 初始管理员 bootstrap。
//!
//! 用于私有化首次部署创建第一个租户 + 管理员（拥有全部权限），也方便本地用密码
//! 登录联调。幂等：可重复执行（更新名称/密码）。

use dms_application::port::PasswordHasher;
use dms_core::CoreResult;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::map_db_error;
use crate::db::tenant::begin_tenant_tx;

/// bootstrap 输入。
pub struct BootstrapInput<'a> {
    pub tenant_slug: &'a str,
    pub tenant_name: &'a str,
    pub email: &'a str,
    pub password: &'a str,
    /// 管理员角色 key（如 `admin`）。
    pub role_key: &'a str,
}

/// 创建/更新一个租户及其管理员用户，并授予该角色全部权限。
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

    // 2) upsert 管理员角色。
    let role_id: Uuid = sqlx::query_scalar(
        "INSERT INTO roles (id, tenant_id, key, name) VALUES ($1, $2, $3, 'Administrator')
         ON CONFLICT (tenant_id, key) DO UPDATE SET name = EXCLUDED.name
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(tenant_id)
    .bind(input.role_key)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_db_error)?;

    // 3) 角色授予全部权限。
    sqlx::query(
        "INSERT INTO role_permissions (tenant_id, role_id, permission_key)
         SELECT $1, $2, key FROM permissions
         ON CONFLICT DO NOTHING",
    )
    .bind(tenant_id)
    .bind(role_id)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

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
