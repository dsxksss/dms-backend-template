-- Project 切片档（project feature）：参考业务实体。
-- 版本区间 0400-0499。依赖 tenancy(app_current_tenant, tenants) 与 core(set_updated_at)。
-- 演示标准审计列 + RLS + 乐观锁(version) + 软删(deleted_at) + 行级历史(触发器)。

CREATE TABLE projects (
    id          uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   uuid        NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name        text        NOT NULL,
    description text        NOT NULL DEFAULT '',
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now(),
    created_by  uuid,
    updated_by  uuid,
    version     integer     NOT NULL DEFAULT 1,
    deleted_at  timestamptz
);
CREATE TRIGGER trg_projects_updated_at
    BEFORE UPDATE ON projects FOR EACH ROW EXECUTE FUNCTION set_updated_at();
ALTER TABLE projects ENABLE ROW LEVEL SECURITY;
CREATE POLICY projects_tenant_isolation ON projects
    USING (tenant_id = app_current_tenant())
    WITH CHECK (tenant_id = app_current_tenant());
CREATE INDEX idx_projects_tenant ON projects (tenant_id) WHERE deleted_at IS NULL;

-- 行级历史：每次 INSERT/UPDATE/DELETE 由触发器写一条快照（可追溯）。
CREATE TABLE projects_history (
    history_id  bigserial   PRIMARY KEY,
    id          uuid        NOT NULL,
    tenant_id   uuid        NOT NULL,
    name        text,
    description text,
    version     integer,
    operation   text        NOT NULL,
    changed_at  timestamptz NOT NULL DEFAULT now(),
    changed_by  uuid
);
ALTER TABLE projects_history ENABLE ROW LEVEL SECURITY;
CREATE POLICY projects_history_tenant_isolation ON projects_history
    USING (tenant_id = app_current_tenant())
    WITH CHECK (tenant_id = app_current_tenant());

CREATE OR REPLACE FUNCTION projects_history_capture() RETURNS trigger
    LANGUAGE plpgsql AS $$
    BEGIN
        INSERT INTO projects_history
            (id, tenant_id, name, description, version, operation, changed_by)
        VALUES (
            COALESCE(NEW.id, OLD.id),
            COALESCE(NEW.tenant_id, OLD.tenant_id),
            COALESCE(NEW.name, OLD.name),
            COALESCE(NEW.description, OLD.description),
            COALESCE(NEW.version, OLD.version),
            TG_OP,
            COALESCE(NEW.updated_by, OLD.updated_by)
        );
        RETURN NULL;
    END
    $$;

CREATE TRIGGER trg_projects_history
    AFTER INSERT OR UPDATE OR DELETE ON projects
    FOR EACH ROW EXECUTE FUNCTION projects_history_capture();
