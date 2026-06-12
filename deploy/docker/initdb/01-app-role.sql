-- 仅开发用：在容器首次初始化时创建受 RLS 约束的运行时应用角色 dms_app。
--
-- 生产/私有化请通过 IaC/密钥管理 provision 该角色（同样的 NOSUPERUSER NOBYPASSRLS
-- 约束），并在运行迁移前完成 —— 不要把登录角色与密码写进迁移文件。
--
-- 本脚本以超级用户 dms 身份运行；其中的 ALTER DEFAULT PRIVILEGES 使「之后由 dms
-- 创建的表/序列」自动授予 dms_app，因此后续迁移无需逐表 GRANT。

DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'dms_app') THEN
        CREATE ROLE dms_app LOGIN PASSWORD 'dms_app'
            NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE;
    END IF;
END
$$;

GRANT USAGE ON SCHEMA public TO dms_app;

ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO dms_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT USAGE, SELECT ON SEQUENCES TO dms_app;
