-- 组织档（orgs feature）：组织 → 团队层级 + 带作用域的角色授予（对标 Benchling）。
-- 版本区间 0500-0599。依赖 tenancy(tenants/app_current_tenant)、auth(users/roles)。
-- 所有表租户作用域 + RLS。

-- 组织（租户内，平级；≈ 业务单元/公司部门）。
CREATE TABLE organizations (
    id         uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id  uuid        NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    slug       text        NOT NULL,
    name       text        NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT organizations_uq UNIQUE (tenant_id, slug)
);
ALTER TABLE organizations ENABLE ROW LEVEL SECURITY;
CREATE POLICY organizations_tenant_isolation ON organizations
    USING (tenant_id = app_current_tenant())
    WITH CHECK (tenant_id = app_current_tenant());

-- 组织成员（用户 ↔ 组织，结构角色 admin/member）。
CREATE TABLE organization_members (
    tenant_id       uuid NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    organization_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id         uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role            text NOT NULL DEFAULT 'member',
    PRIMARY KEY (organization_id, user_id),
    CONSTRAINT organization_members_role_chk CHECK (role IN ('admin', 'member'))
);
ALTER TABLE organization_members ENABLE ROW LEVEL SECURITY;
CREATE POLICY organization_members_tenant_isolation ON organization_members
    USING (tenant_id = app_current_tenant())
    WITH CHECK (tenant_id = app_current_tenant());

-- 团队（隶属某组织）。
CREATE TABLE teams (
    id              uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       uuid        NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    organization_id uuid        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    slug            text        NOT NULL,
    name            text        NOT NULL,
    created_at      timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT teams_uq UNIQUE (organization_id, slug)
);
ALTER TABLE teams ENABLE ROW LEVEL SECURITY;
CREATE POLICY teams_tenant_isolation ON teams
    USING (tenant_id = app_current_tenant())
    WITH CHECK (tenant_id = app_current_tenant());

-- 团队成员（用户 ↔ 团队，结构角色 admin/member）。
CREATE TABLE team_members (
    tenant_id uuid NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    team_id   uuid NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    user_id   uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role      text NOT NULL DEFAULT 'member',
    PRIMARY KEY (team_id, user_id),
    CONSTRAINT team_members_role_chk CHECK (role IN ('admin', 'member'))
);
ALTER TABLE team_members ENABLE ROW LEVEL SECURITY;
CREATE POLICY team_members_tenant_isolation ON team_members
    USING (tenant_id = app_current_tenant())
    WITH CHECK (tenant_id = app_current_tenant());

-- 带作用域的角色授予（通用）：把某 role 授予 用户/团队，作用域 = 租户/组织/团队/资源。
-- 有效权限 = 适用授予的权限并集（累积、取最宽松）。
CREATE TABLE role_grants (
    id             uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id      uuid        NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    principal_type text        NOT NULL,                 -- user | team
    principal_id   uuid        NOT NULL,
    role_id        uuid        NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    scope_type     text        NOT NULL,                 -- tenant | organization | team | resource
    scope_id       uuid,                                 -- tenant 时为 NULL
    resource_type  text,                                 -- scope=resource 时标明资源类型（如 'project'）
    created_at     timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT role_grants_principal_chk CHECK (principal_type IN ('user', 'team')),
    CONSTRAINT role_grants_scope_chk CHECK (scope_type IN ('tenant', 'organization', 'team', 'resource')),
    CONSTRAINT role_grants_uq UNIQUE (principal_type, principal_id, role_id, scope_type, scope_id, resource_type)
);
ALTER TABLE role_grants ENABLE ROW LEVEL SECURITY;
CREATE POLICY role_grants_tenant_isolation ON role_grants
    USING (tenant_id = app_current_tenant())
    WITH CHECK (tenant_id = app_current_tenant());
CREATE INDEX idx_role_grants_principal ON role_grants (tenant_id, principal_type, principal_id);
CREATE INDEX idx_role_grants_scope ON role_grants (tenant_id, scope_type, scope_id);

-- 组织管理权限（bootstrap 的 admin 角色会自动获得全部权限）。
INSERT INTO permissions (key, description) VALUES
    ('org:read',  'Read organizations and teams'),
    ('org:write', 'Manage organizations, teams, members and role grants')
ON CONFLICT DO NOTHING;
