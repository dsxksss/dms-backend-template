-- 审计档（audit feature）：审计日志 + 事务性发件箱。
-- 版本区间 0300-0399。依赖 tenancy(app_current_tenant)。
-- 这两张表是「可追溯」的通用基建，供任何业务实体复用（不限 Project）。

-- 审计日志：谁在何时对哪个实体做了什么（含前后变更）。
CREATE TABLE audit_log (
    id          uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   uuid        NOT NULL,
    actor_id    uuid,
    action      text        NOT NULL,
    entity_type text        NOT NULL,
    entity_id   uuid        NOT NULL,
    changes     jsonb       NOT NULL DEFAULT '{}'::jsonb,
    request_id  uuid,
    occurred_at timestamptz NOT NULL DEFAULT now()
);
ALTER TABLE audit_log ENABLE ROW LEVEL SECURITY;
CREATE POLICY audit_log_tenant_isolation ON audit_log
    USING (tenant_id = app_current_tenant())
    WITH CHECK (tenant_id = app_current_tenant());
CREATE INDEX idx_audit_log_entity ON audit_log (tenant_id, entity_type, entity_id);

-- 事务性发件箱：领域事件与状态变更在同一事务落库，后台中继可靠投递。
CREATE TABLE outbox (
    id             uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id      uuid        NOT NULL,
    aggregate_type text        NOT NULL,
    aggregate_id   uuid        NOT NULL,
    event_type     text        NOT NULL,
    payload        jsonb       NOT NULL DEFAULT '{}'::jsonb,
    occurred_at    timestamptz NOT NULL DEFAULT now(),
    processed_at   timestamptz
);
ALTER TABLE outbox ENABLE ROW LEVEL SECURITY;
CREATE POLICY outbox_tenant_isolation ON outbox
    USING (tenant_id = app_current_tenant())
    WITH CHECK (tenant_id = app_current_tenant());
-- 中继以拥有者连接（绕过 RLS）扫描未处理事件。
CREATE INDEX idx_outbox_unprocessed ON outbox (occurred_at) WHERE processed_at IS NULL;
