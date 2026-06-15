//! 默认角色 seed 机制（auth 档）。
//!
//! 每个租户在开通时获得一套**标准 RBAC 角色 + 权限包**（owner/admin/member）。
//! 这是通用机制：角色与权限包以声明式 [`DEFAULT_ROLES`] 描述，[`seed_default_roles`]
//! 幂等落库。业务可在此基础上追加自有角色/权限包。
//!
//! 注意区分两类「角色」：
//! - **租户 RBAC 角色**（此处）：跨整个租户的权限集合，进 `roles`/`role_permissions`。
//! - **项目结构角色**（`project_members.role`：owner/manager/contributor/viewer）：
//!   项目本地的协作身份，不在此 seed，详见 project 档。

use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use dms_core::CoreResult;

use crate::db::map_db_error;

/// 角色的权限包。
#[derive(Debug, Clone, Copy)]
pub enum PermissionBundle {
    /// 目录中**全部**权限（含未来新增）——给最高权限角色。
    All,
    /// 指定权限 key；目录中不存在的 key 自动跳过（按已启用档的权限取交集）。
    Keys(&'static [&'static str]),
}

/// 一个默认角色的声明。
#[derive(Debug, Clone, Copy)]
pub struct RoleSeed {
    /// 角色 key（租户内唯一，如 `owner`）。
    pub key: &'static str,
    /// 展示名。
    pub name: &'static str,
    /// 该角色的权限包。
    pub permissions: PermissionBundle,
}

/// 标准角色目录（自高到低）。
///
/// - `owner`：全部权限（租户拥有者，新增权限自动纳入）。
/// - `admin`：显式列出的运营权限集（不会自动吞并未来的 owner 专属权限）。
/// - `member`：读基线 + 项目贡献。
///
/// 仅引用确定通用的权限 key；某档未启用时其 key 不在目录中、自动跳过。
pub const DEFAULT_ROLES: &[RoleSeed] = &[
    RoleSeed {
        key: "owner",
        name: "Owner",
        permissions: PermissionBundle::All,
    },
    RoleSeed {
        key: "admin",
        name: "Administrator",
        permissions: PermissionBundle::Keys(&[
            "project:read",
            "project:write",
            "user:read",
            "user:write",
            "org:read",
            "org:write",
        ]),
    },
    RoleSeed {
        key: "member",
        name: "Member",
        permissions: PermissionBundle::Keys(&[
            "project:read",
            "project:write",
            "user:read",
            "org:read",
        ]),
    },
];

/// 在给定租户事务内幂等落库 [`DEFAULT_ROLES`]：upsert 角色 + 绑定权限包。
///
/// 在租户作用域事务内调用（RLS 要求已 `SET app.current_tenant`）。重复执行安全：
/// 角色按 `(tenant_id, key)` upsert，权限绑定 `ON CONFLICT DO NOTHING`。
pub async fn seed_default_roles(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
) -> CoreResult<()> {
    for role in DEFAULT_ROLES {
        let role_id: Uuid = sqlx::query_scalar(
            "INSERT INTO roles (id, tenant_id, key, name) VALUES ($1, $2, $3, $4)
             ON CONFLICT (tenant_id, key) DO UPDATE SET name = EXCLUDED.name
             RETURNING id",
        )
        .bind(Uuid::now_v7())
        .bind(tenant_id)
        .bind(role.key)
        .bind(role.name)
        .fetch_one(&mut **tx)
        .await
        .map_err(map_db_error)?;

        match role.permissions {
            PermissionBundle::All => {
                sqlx::query(
                    "INSERT INTO role_permissions (tenant_id, role_id, permission_key)
                     SELECT $1, $2, key FROM permissions
                     ON CONFLICT DO NOTHING",
                )
                .bind(tenant_id)
                .bind(role_id)
                .execute(&mut **tx)
                .await
                .map_err(map_db_error)?;
            }
            PermissionBundle::Keys(keys) => {
                let keys: Vec<&str> = keys.to_vec();
                sqlx::query(
                    "INSERT INTO role_permissions (tenant_id, role_id, permission_key)
                     SELECT $1, $2, p.key FROM permissions p WHERE p.key = ANY($3)
                     ON CONFLICT DO NOTHING",
                )
                .bind(tenant_id)
                .bind(role_id)
                .bind(&keys)
                .execute(&mut **tx)
                .await
                .map_err(map_db_error)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_well_formed() {
        // 角色 key 唯一。
        let mut keys: Vec<&str> = DEFAULT_ROLES.iter().map(|r| r.key).collect();
        let n = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), n, "role keys must be unique");

        // owner 必须是全权限。
        assert!(matches!(
            DEFAULT_ROLES[0].permissions,
            PermissionBundle::All
        ));

        // member 的权限是 admin 的子集（层级单调）。
        let bundle_keys = |b: PermissionBundle| -> Vec<&str> {
            match b {
                PermissionBundle::Keys(k) => k.to_vec(),
                PermissionBundle::All => vec![],
            }
        };
        let admin = bundle_keys(DEFAULT_ROLES[1].permissions);
        let member = bundle_keys(DEFAULT_ROLES[2].permissions);
        assert!(
            member.iter().all(|m| admin.contains(m)),
            "member perms must be a subset of admin"
        );
    }
}
