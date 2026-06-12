-- 核心档（database feature）：基础扩展与通用触发器函数。
-- 与多租户/认证无关，任何用到数据库的档位都会执行。

-- gen_random_uuid()（兜底用；应用通常自带 UUIDv7 主键）。
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- 通用 updated_at 维护：在 BEFORE UPDATE 触发，自动刷新 updated_at。
CREATE OR REPLACE FUNCTION set_updated_at() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
    BEGIN
        NEW.updated_at := now();
        RETURN NEW;
    END
    $$;
