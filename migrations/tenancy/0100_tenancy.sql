-- 多租户档（multi-tenancy feature）：RLS 约定 + 租户注册表。
-- 版本区间 0100-0199 预留给多租户基建。

-- 多租户 RLS 约定：
-- 每个请求在事务内执行 `SELECT set_config('app.current_tenant', '<uuid>', true)`
-- 设置当前租户（事务局部）。RLS 策略通过下面的 helper 读取它；未设置时返回 NULL，
-- 策略据此拒绝（默认拒绝，安全）。
CREATE OR REPLACE FUNCTION app_current_tenant() RETURNS uuid
    LANGUAGE sql STABLE
    AS $$
        SELECT NULLIF(current_setting('app.current_tenant', true), '')::uuid
    $$;

-- 租户注册表（多租户根）。
--
-- 本表刻意【不启用 RLS】：登录/路由阶段需要在「尚未确定租户上下文」时按 slug 或
-- 域名解析出租户，因此运行时角色需能按 slug 查询本表。访问控制在应用层完成。
CREATE TABLE tenants (
    id          uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    slug        text        NOT NULL UNIQUE,
    name        text        NOT NULL,
    status      text        NOT NULL DEFAULT 'active',
    settings    jsonb       NOT NULL DEFAULT '{}'::jsonb,
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT tenants_status_chk CHECK (status IN ('active', 'suspended'))
);

CREATE TRIGGER trg_tenants_updated_at
    BEFORE UPDATE ON tenants
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE INDEX idx_tenants_status ON tenants (status);
