-- Project 成员（project feature）：项目作为协作容器，成员 + 项目内结构角色。
-- 版本区间 0400-0499。依赖 projects、auth(users)、tenancy(tenants/app_current_tenant)。
-- 角色（自高到低）：owner 属主 / manager 管理 / contributor 贡献 / viewer 只读。
-- 这是「容器 + 成员」通用模型（对标 Benchling 的 Project Members），与 RBAC 的
-- role_grants（orgs 档）解耦：此处是结构化成员关系，role_grants 是可叠加的权限授予。

CREATE TABLE project_members (
    tenant_id  uuid        NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    project_id uuid        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_id    uuid        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role       text        NOT NULL DEFAULT 'viewer',
    added_at   timestamptz NOT NULL DEFAULT now(),
    added_by   uuid,
    PRIMARY KEY (project_id, user_id),
    CONSTRAINT project_members_role_chk
        CHECK (role IN ('owner', 'manager', 'contributor', 'viewer'))
);
ALTER TABLE project_members ENABLE ROW LEVEL SECURITY;
CREATE POLICY project_members_tenant_isolation ON project_members
    USING (tenant_id = app_current_tenant())
    WITH CHECK (tenant_id = app_current_tenant());
CREATE INDEX idx_project_members_user ON project_members (tenant_id, user_id);
