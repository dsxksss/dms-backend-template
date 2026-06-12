//! 身份相关仓储的 SQLx 实现。
//!
//! 租户作用域操作一律经 `begin_tenant_tx`（设置 RLS 会话变量），由数据库强制
//! 行级隔离；查询本身无需手写 `WHERE tenant_id = ?`。

use async_trait::async_trait;
use dms_core::{CoreResult, TenantId, UserId};
use dms_domain::iam::{Tenant, User, UserStatus};
use dms_domain::ports::{
    ActiveRefreshToken, ExternalIdentityRepository, NewRefreshToken, ProvisionExternal,
    RefreshTokenRepository, TenantRepository, UserRepository,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::map_db_error;
use crate::db::tenant::begin_tenant_tx;

// ---- 行结构体（DB ↔ 领域 边界映射）----

#[derive(sqlx::FromRow)]
struct TenantRow {
    id: Uuid,
    slug: String,
    name: String,
    status: String,
}

impl From<TenantRow> for Tenant {
    fn from(r: TenantRow) -> Self {
        Tenant {
            id: r.id.into(),
            slug: r.slug,
            name: r.name,
            status: r.status,
        }
    }
}

#[derive(sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    tenant_id: Uuid,
    email: String,
    password_hash: Option<String>,
    display_name: String,
    status: String,
    version: i32,
}

impl From<UserRow> for User {
    fn from(r: UserRow) -> Self {
        User {
            id: r.id.into(),
            tenant_id: r.tenant_id.into(),
            email: r.email,
            password_hash: r.password_hash,
            display_name: r.display_name,
            status: UserStatus::from_db(&r.status),
            version: r.version,
        }
    }
}

const USER_COLUMNS: &str = "id, tenant_id, email, password_hash, display_name, status, version";

// ---- 租户仓储（无 RLS）----

pub struct PgTenantRepository {
    pool: PgPool,
}

impl PgTenantRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TenantRepository for PgTenantRepository {
    async fn find_by_slug(&self, slug: &str) -> CoreResult<Option<Tenant>> {
        let row = sqlx::query_as::<_, TenantRow>(
            "SELECT id, slug, name, status FROM tenants WHERE slug = $1",
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(row.map(Into::into))
    }
}

// ---- 用户仓储（RLS）----

pub struct PgUserRepository {
    pool: PgPool,
}

impl PgUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PgUserRepository {
    async fn find_by_email(&self, tenant: TenantId, email: &str) -> CoreResult<Option<User>> {
        let mut tx = begin_tenant_tx(&self.pool, tenant.into()).await?;
        let row = sqlx::query_as::<_, UserRow>(&format!(
            "SELECT {USER_COLUMNS} FROM users WHERE email = $1 AND deleted_at IS NULL"
        ))
        .bind(email)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(row.map(Into::into))
    }

    async fn find_by_id(&self, tenant: TenantId, id: UserId) -> CoreResult<Option<User>> {
        let mut tx = begin_tenant_tx(&self.pool, tenant.into()).await?;
        let row = sqlx::query_as::<_, UserRow>(&format!(
            "SELECT {USER_COLUMNS} FROM users WHERE id = $1 AND deleted_at IS NULL"
        ))
        .bind(Uuid::from(id))
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(row.map(Into::into))
    }

    async fn permissions_for(&self, tenant: TenantId, user: UserId) -> CoreResult<Vec<String>> {
        let mut tx = begin_tenant_tx(&self.pool, tenant.into()).await?;
        let perms = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT rp.permission_key
             FROM user_roles ur
             JOIN role_permissions rp ON rp.role_id = ur.role_id
             WHERE ur.user_id = $1",
        )
        .bind(Uuid::from(user))
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(perms)
    }
}

// ---- 外部身份仓储（RLS + JIT）----

pub struct PgExternalIdentityRepository {
    pool: PgPool,
}

impl PgExternalIdentityRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ExternalIdentityRepository for PgExternalIdentityRepository {
    async fn resolve_or_provision(
        &self,
        tenant: TenantId,
        input: ProvisionExternal,
    ) -> CoreResult<User> {
        let tenant_uuid = Uuid::from(tenant);
        let mut tx = begin_tenant_tx(&self.pool, tenant_uuid).await?;

        // 1) 命中既有外部身份映射。
        let existing = sqlx::query_as::<_, UserRow>(
            "SELECT u.id, u.tenant_id, u.email, u.password_hash, u.display_name, u.status, u.version
             FROM external_identities ei
             JOIN users u ON u.id = ei.user_id
             WHERE ei.provider = $1 AND ei.external_subject = $2 AND u.deleted_at IS NULL",
        )
        .bind(&input.provider)
        .bind(&input.subject)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?;

        let user_row = if let Some(u) = existing {
            sqlx::query(
                "UPDATE external_identities SET last_login_at = now()
                 WHERE provider = $1 AND external_subject = $2",
            )
            .bind(&input.provider)
            .bind(&input.subject)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;
            u
        } else {
            // 2) 按已验证 email 匹配既有用户。
            let matched = match input.email.as_deref() {
                Some(email) => sqlx::query_as::<_, UserRow>(&format!(
                    "SELECT {USER_COLUMNS} FROM users WHERE email = $1 AND deleted_at IS NULL"
                ))
                .bind(email)
                .fetch_optional(&mut *tx)
                .await
                .map_err(map_db_error)?,
                None => None,
            };

            // 3) 否则 JIT 自动开通用户。
            let user_row = match matched {
                Some(u) => u,
                None => {
                    let email = input.email.clone().unwrap_or_else(|| {
                        format!("{}+{}@external.local", input.subject, input.provider)
                    });
                    let display = input.display_name.clone().unwrap_or_default();
                    sqlx::query_as::<_, UserRow>(&format!(
                        "INSERT INTO users (id, tenant_id, email, display_name, status)
                         VALUES ($1, $2, $3, $4, 'active')
                         RETURNING {USER_COLUMNS}"
                    ))
                    .bind(Uuid::now_v7())
                    .bind(tenant_uuid)
                    .bind(&email)
                    .bind(&display)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(map_db_error)?
                }
            };

            // 建立外部身份映射。
            sqlx::query(
                "INSERT INTO external_identities
                    (id, tenant_id, user_id, provider, external_subject, external_email, raw_claims, last_login_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, now())",
            )
            .bind(Uuid::now_v7())
            .bind(tenant_uuid)
            .bind(user_row.id)
            .bind(&input.provider)
            .bind(&input.subject)
            .bind(input.email.as_deref())
            .bind(sqlx::types::Json(&input.raw_claims))
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;

            user_row
        };

        tx.commit().await.map_err(map_db_error)?;
        Ok(user_row.into())
    }
}

// ---- 刷新令牌仓储（RLS）----

pub struct PgRefreshTokenRepository {
    pool: PgPool,
}

impl PgRefreshTokenRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RefreshTokenRepository for PgRefreshTokenRepository {
    async fn issue(&self, tenant: TenantId, token: NewRefreshToken) -> CoreResult<()> {
        let tenant_uuid = Uuid::from(tenant);
        let mut tx = begin_tenant_tx(&self.pool, tenant_uuid).await?;
        sqlx::query(
            "INSERT INTO refresh_tokens (id, tenant_id, user_id, token_hash, expires_at)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(Uuid::now_v7())
        .bind(tenant_uuid)
        .bind(Uuid::from(token.user_id))
        .bind(&token.token_hash)
        .bind(token.expires_at)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    async fn find_active(
        &self,
        tenant: TenantId,
        token_hash: &str,
    ) -> CoreResult<Option<ActiveRefreshToken>> {
        let mut tx = begin_tenant_tx(&self.pool, tenant.into()).await?;
        let row: Option<(Uuid,)> = sqlx::query_as(
            "SELECT user_id FROM refresh_tokens
             WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > now()",
        )
        .bind(token_hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(row.map(|(user_id,)| ActiveRefreshToken {
            user_id: user_id.into(),
        }))
    }

    async fn revoke(&self, tenant: TenantId, token_hash: &str) -> CoreResult<()> {
        let mut tx = begin_tenant_tx(&self.pool, tenant.into()).await?;
        sqlx::query(
            "UPDATE refresh_tokens SET revoked_at = now()
             WHERE token_hash = $1 AND revoked_at IS NULL",
        )
        .bind(token_hash)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }
}
