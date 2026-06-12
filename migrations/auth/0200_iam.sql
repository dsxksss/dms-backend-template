-- 认证档（auth feature）：用户、外部身份映射、刷新令牌、RBAC。
-- 版本区间 0200-0299 预留给身份与访问管理。依赖 core(set_updated_at) 与
-- tenancy(app_current_tenant, tenants)。所有租户作用域表启用 RLS。

-- 用户（租户作用域）。password_hash 可空：纯外部身份用户无本地密码。
CREATE TABLE users (
    id            uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     uuid        NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    email         text        NOT NULL,
    password_hash text,
    display_name  text        NOT NULL DEFAULT '',
    status        text        NOT NULL DEFAULT 'active',
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL DEFAULT now(),
    created_by    uuid,
    updated_by    uuid,
    version       integer     NOT NULL DEFAULT 1,
    deleted_at    timestamptz,
    CONSTRAINT users_status_chk CHECK (status IN ('active', 'suspended')),
    CONSTRAINT users_email_uq   UNIQUE (tenant_id, email)
);
CREATE TRIGGER trg_users_updated_at
    BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION set_updated_at();
ALTER TABLE users ENABLE ROW LEVEL SECURITY;
CREATE POLICY users_tenant_isolation ON users
    USING (tenant_id = app_current_tenant())
    WITH CHECK (tenant_id = app_current_tenant());
CREATE INDEX idx_users_tenant ON users (tenant_id);

-- 外部身份映射（账号联动核心）。一个内部用户可关联多个外部身份。
CREATE TABLE external_identities (
    id               uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id        uuid        NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id          uuid        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider         text        NOT NULL,
    external_subject text        NOT NULL,
    external_email   text,
    raw_claims       jsonb       NOT NULL DEFAULT '{}'::jsonb,
    linked_at        timestamptz NOT NULL DEFAULT now(),
    last_login_at    timestamptz,
    CONSTRAINT external_identities_uq UNIQUE (tenant_id, provider, external_subject)
);
ALTER TABLE external_identities ENABLE ROW LEVEL SECURITY;
CREATE POLICY external_identities_tenant_isolation ON external_identities
    USING (tenant_id = app_current_tenant())
    WITH CHECK (tenant_id = app_current_tenant());
CREATE INDEX idx_external_identities_user ON external_identities (tenant_id, user_id);

-- 刷新令牌（存摘要，支持轮换与吊销）。
CREATE TABLE refresh_tokens (
    id          uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   uuid        NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id     uuid        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  text        NOT NULL UNIQUE,
    issued_at   timestamptz NOT NULL DEFAULT now(),
    expires_at  timestamptz NOT NULL,
    revoked_at  timestamptz
);
ALTER TABLE refresh_tokens ENABLE ROW LEVEL SECURITY;
CREATE POLICY refresh_tokens_tenant_isolation ON refresh_tokens
    USING (tenant_id = app_current_tenant())
    WITH CHECK (tenant_id = app_current_tenant());
CREATE INDEX idx_refresh_tokens_user ON refresh_tokens (tenant_id, user_id);

-- RBAC：权限为全局目录；角色与分配按租户。
CREATE TABLE permissions (
    key         text PRIMARY KEY,
    description text NOT NULL DEFAULT ''
);

CREATE TABLE roles (
    id         uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id  uuid        NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    key        text        NOT NULL,
    name       text        NOT NULL DEFAULT '',
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT roles_uq UNIQUE (tenant_id, key)
);
ALTER TABLE roles ENABLE ROW LEVEL SECURITY;
CREATE POLICY roles_tenant_isolation ON roles
    USING (tenant_id = app_current_tenant())
    WITH CHECK (tenant_id = app_current_tenant());

CREATE TABLE role_permissions (
    tenant_id      uuid NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    role_id        uuid NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission_key text NOT NULL REFERENCES permissions(key) ON DELETE CASCADE,
    PRIMARY KEY (role_id, permission_key)
);
ALTER TABLE role_permissions ENABLE ROW LEVEL SECURITY;
CREATE POLICY role_permissions_tenant_isolation ON role_permissions
    USING (tenant_id = app_current_tenant())
    WITH CHECK (tenant_id = app_current_tenant());

CREATE TABLE user_roles (
    tenant_id uuid NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id   uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id   uuid NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, role_id)
);
ALTER TABLE user_roles ENABLE ROW LEVEL SECURITY;
CREATE POLICY user_roles_tenant_isolation ON user_roles
    USING (tenant_id = app_current_tenant())
    WITH CHECK (tenant_id = app_current_tenant());

-- 种子权限（全局目录）。Project 切片（M4）会用到 project:*。
INSERT INTO permissions (key, description) VALUES
    ('project:read',  'Read projects'),
    ('project:write', 'Create, update and delete projects'),
    ('user:read',     'Read users'),
    ('user:write',    'Manage users')
ON CONFLICT DO NOTHING;
