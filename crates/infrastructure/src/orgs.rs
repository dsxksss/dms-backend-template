//! 组织架构仓储的 SQLx 实现：组织/团队/成员 + 带作用域角色授予 + 累积权限解析。

use std::collections::BTreeSet;

use async_trait::async_trait;
use dms_core::{CoreError, CoreResult, RequestContext, TenantId, UserId};
use dms_domain::orgs::{
    GrantRepository, GrantScope, MemberRole, NewOrganization, NewTeam, OrgRepository, Organization,
    OrganizationId, Principal, RoleGrantInput, Team, TeamId,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::map_db_error;
use crate::db::tenant::begin_tenant_tx;

#[derive(sqlx::FromRow)]
struct OrgRow {
    id: Uuid,
    tenant_id: Uuid,
    slug: String,
    name: String,
}

impl From<OrgRow> for Organization {
    fn from(r: OrgRow) -> Self {
        Organization {
            id: r.id.into(),
            tenant_id: r.tenant_id.into(),
            slug: r.slug,
            name: r.name,
        }
    }
}

#[derive(sqlx::FromRow)]
struct TeamRow {
    id: Uuid,
    tenant_id: Uuid,
    organization_id: Uuid,
    slug: String,
    name: String,
}

impl From<TeamRow> for Team {
    fn from(r: TeamRow) -> Self {
        Team {
            id: r.id.into(),
            tenant_id: r.tenant_id.into(),
            organization_id: r.organization_id.into(),
            slug: r.slug,
            name: r.name,
        }
    }
}

// ---- 组织/团队/成员仓储 ----

pub struct PgOrgRepository {
    pool: PgPool,
}

impl PgOrgRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl OrgRepository for PgOrgRepository {
    async fn create_organization(
        &self,
        ctx: &RequestContext,
        input: NewOrganization,
    ) -> CoreResult<Organization> {
        let tenant = Uuid::from(ctx.tenant_id);
        let mut tx = begin_tenant_tx(&self.pool, tenant).await?;
        let row = sqlx::query_as::<_, OrgRow>(
            "INSERT INTO organizations (id, tenant_id, slug, name) VALUES ($1, $2, $3, $4)
             RETURNING id, tenant_id, slug, name",
        )
        .bind(Uuid::now_v7())
        .bind(tenant)
        .bind(&input.slug)
        .bind(&input.name)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(row.into())
    }

    async fn list_organizations(&self, tenant: TenantId) -> CoreResult<Vec<Organization>> {
        let mut tx = begin_tenant_tx(&self.pool, tenant.into()).await?;
        let rows = sqlx::query_as::<_, OrgRow>(
            "SELECT id, tenant_id, slug, name FROM organizations ORDER BY name",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn add_org_member(
        &self,
        ctx: &RequestContext,
        organization: OrganizationId,
        user: UserId,
        role: MemberRole,
    ) -> CoreResult<()> {
        let tenant = Uuid::from(ctx.tenant_id);
        let mut tx = begin_tenant_tx(&self.pool, tenant).await?;
        sqlx::query(
            "INSERT INTO organization_members (tenant_id, organization_id, user_id, role)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (organization_id, user_id) DO UPDATE SET role = EXCLUDED.role",
        )
        .bind(tenant)
        .bind(Uuid::from(organization))
        .bind(Uuid::from(user))
        .bind(role.as_str())
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    async fn create_team(&self, ctx: &RequestContext, input: NewTeam) -> CoreResult<Team> {
        let tenant = Uuid::from(ctx.tenant_id);
        let mut tx = begin_tenant_tx(&self.pool, tenant).await?;
        let row = sqlx::query_as::<_, TeamRow>(
            "INSERT INTO teams (id, tenant_id, organization_id, slug, name)
             VALUES ($1, $2, $3, $4, $5)
             RETURNING id, tenant_id, organization_id, slug, name",
        )
        .bind(Uuid::now_v7())
        .bind(tenant)
        .bind(Uuid::from(input.organization_id))
        .bind(&input.slug)
        .bind(&input.name)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(row.into())
    }

    async fn list_teams(
        &self,
        tenant: TenantId,
        organization: OrganizationId,
    ) -> CoreResult<Vec<Team>> {
        let mut tx = begin_tenant_tx(&self.pool, tenant.into()).await?;
        let rows = sqlx::query_as::<_, TeamRow>(
            "SELECT id, tenant_id, organization_id, slug, name FROM teams
             WHERE organization_id = $1 ORDER BY name",
        )
        .bind(Uuid::from(organization))
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn add_team_member(
        &self,
        ctx: &RequestContext,
        team: TeamId,
        user: UserId,
        role: MemberRole,
    ) -> CoreResult<()> {
        let tenant = Uuid::from(ctx.tenant_id);
        let mut tx = begin_tenant_tx(&self.pool, tenant).await?;
        sqlx::query(
            "INSERT INTO team_members (tenant_id, team_id, user_id, role)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (team_id, user_id) DO UPDATE SET role = EXCLUDED.role",
        )
        .bind(tenant)
        .bind(Uuid::from(team))
        .bind(Uuid::from(user))
        .bind(role.as_str())
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }
}

// ---- 带作用域角色授予 + 累积权限解析 ----

pub struct PgGrantRepository {
    pool: PgPool,
}

impl PgGrantRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn principal_parts(p: &Principal) -> (&'static str, Uuid) {
    match p {
        Principal::User(id) => ("user", Uuid::from(*id)),
        Principal::Team(id) => ("team", Uuid::from(*id)),
    }
}

fn scope_parts(s: &GrantScope) -> (&'static str, Option<Uuid>, Option<String>) {
    match s {
        GrantScope::Tenant => ("tenant", None, None),
        GrantScope::Organization(id) => ("organization", Some(Uuid::from(*id)), None),
        GrantScope::Team(id) => ("team", Some(Uuid::from(*id)), None),
        GrantScope::Resource {
            resource_type,
            resource_id,
        } => ("resource", Some(*resource_id), Some(resource_type.clone())),
    }
}

/// 数据库中的一条授予作用域是否匹配目标作用域。
fn scope_matches(st: &str, sid: Option<Uuid>, rt: Option<&str>, target: &GrantScope) -> bool {
    match target {
        GrantScope::Tenant => st == "tenant",
        GrantScope::Organization(id) => st == "organization" && sid == Some(Uuid::from(*id)),
        GrantScope::Team(id) => st == "team" && sid == Some(Uuid::from(*id)),
        GrantScope::Resource {
            resource_type,
            resource_id,
        } => st == "resource" && sid == Some(*resource_id) && rt == Some(resource_type.as_str()),
    }
}

#[async_trait]
impl GrantRepository for PgGrantRepository {
    async fn grant(&self, ctx: &RequestContext, input: RoleGrantInput) -> CoreResult<()> {
        let tenant = Uuid::from(ctx.tenant_id);
        let mut tx = begin_tenant_tx(&self.pool, tenant).await?;

        let role_id: Uuid = sqlx::query_scalar("SELECT id FROM roles WHERE key = $1")
            .bind(&input.role_key)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_db_error)?
            .ok_or_else(|| CoreError::Validation(format!("unknown role: {}", input.role_key)))?;

        let (ptype, pid) = principal_parts(&input.principal);
        let (stype, sid, rtype) = scope_parts(&input.scope);

        sqlx::query(
            "INSERT INTO role_grants
                (id, tenant_id, principal_type, principal_id, role_id, scope_type, scope_id, resource_type)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT DO NOTHING",
        )
        .bind(Uuid::now_v7())
        .bind(tenant)
        .bind(ptype)
        .bind(pid)
        .bind(role_id)
        .bind(stype)
        .bind(sid)
        .bind(rtype)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    async fn revoke(&self, ctx: &RequestContext, input: RoleGrantInput) -> CoreResult<()> {
        let tenant = Uuid::from(ctx.tenant_id);
        let mut tx = begin_tenant_tx(&self.pool, tenant).await?;
        let (ptype, pid) = principal_parts(&input.principal);
        let (stype, sid, rtype) = scope_parts(&input.scope);
        sqlx::query(
            "DELETE FROM role_grants g
             USING roles r
             WHERE g.role_id = r.id AND r.key = $1
               AND g.principal_type = $2 AND g.principal_id = $3
               AND g.scope_type = $4
               AND g.scope_id IS NOT DISTINCT FROM $5
               AND g.resource_type IS NOT DISTINCT FROM $6",
        )
        .bind(&input.role_key)
        .bind(ptype)
        .bind(pid)
        .bind(stype)
        .bind(sid)
        .bind(rtype)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    async fn effective_permissions(
        &self,
        tenant: TenantId,
        user: UserId,
        scopes: &[GrantScope],
    ) -> CoreResult<Vec<String>> {
        let uid = Uuid::from(user);
        let mut tx = begin_tenant_tx(&self.pool, tenant.into()).await?;

        // 1) 租户级权限（user_roles —— 与登录 JWT 同源）。
        let mut perms: BTreeSet<String> = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT rp.permission_key FROM user_roles ur
             JOIN role_permissions rp ON rp.role_id = ur.role_id
             WHERE ur.user_id = $1",
        )
        .bind(uid)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db_error)?
        .into_iter()
        .collect();

        // 2) 作用域授予（role_grants：用户直授 + 用户所属团队的授予）。
        let rows = sqlx::query_as::<_, (String, String, Option<Uuid>, Option<String>)>(
            "SELECT rp.permission_key, g.scope_type, g.scope_id, g.resource_type
             FROM role_grants g
             JOIN role_permissions rp ON rp.role_id = g.role_id
             WHERE (g.principal_type = 'user' AND g.principal_id = $1)
                OR (g.principal_type = 'team' AND g.principal_id IN
                    (SELECT team_id FROM team_members WHERE user_id = $1))",
        )
        .bind(uid)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;

        for (perm, stype, sid, rtype) in rows {
            if stype == "tenant"
                || scopes
                    .iter()
                    .any(|s| scope_matches(&stype, sid, rtype.as_deref(), s))
            {
                perms.insert(perm);
            }
        }

        Ok(perms.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dms_domain::orgs::OrganizationId;

    #[test]
    fn scope_matching_rules() {
        let org = OrganizationId::new();
        let oid = Uuid::from(org);
        assert!(scope_matches(
            "organization",
            Some(oid),
            None,
            &GrantScope::Organization(org)
        ));
        assert!(!scope_matches(
            "organization",
            Some(Uuid::now_v7()),
            None,
            &GrantScope::Organization(org)
        ));
        assert!(scope_matches("tenant", None, None, &GrantScope::Tenant));

        let rid = Uuid::now_v7();
        let target = GrantScope::Resource {
            resource_type: "project".into(),
            resource_id: rid,
        };
        assert!(scope_matches(
            "resource",
            Some(rid),
            Some("project"),
            &target
        ));
        // 资源类型不同 → 不匹配
        assert!(!scope_matches(
            "resource",
            Some(rid),
            Some("dataset"),
            &target
        ));
        // 作用域类型不同 → 不匹配
        assert!(!scope_matches("tenant", None, None, &target));
    }
}
