# 开发指南

## 起步

```bash
# 起本地 Postgres（5432 被占用：export DMS_DB_PORT=5433，并相应改连接串）
docker compose -f deploy/docker/docker-compose.yml up -d postgres
cp config/local.toml.example config/local.toml
export APP_ENV=local

# 精简档（无需数据库）
cargo run -p dms-server

# 完整档：先建初始管理员（顺带迁移），再启动
BOOTSTRAP_EMAIL=admin@acme.com BOOTSTRAP_PASSWORD=change-me \
  cargo run -p dms-server --features full -- bootstrap
cargo run -p dms-server --features full
```

## 常用命令

```bash
cargo build -p dms-server [--features <tier>]    # 按档构建
cargo test --workspace --all-features            # 单元 + mock（集成测试默认 #[ignore]）
TEST_DATABASE_URL=postgres://dms:dms@127.0.0.1:5433/dms \
  cargo test -p dms-infrastructure --features project -- --ignored   # 真实库集成测试
cargo fmt --all
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo deny check
```

## 自测接口（完整档）

```bash
# 登录
curl -s localhost:8080/v1/auth/login -H 'content-type: application/json' \
  -d '{"tenant":"acme","email":"admin@acme.com","password":"change-me"}'
# 用返回的 access_token：
TOKEN=...
curl -s localhost:8080/v1/me -H "authorization: Bearer $TOKEN"
curl -s -X POST localhost:8080/v1/projects -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"name":"Demo"}'
curl -s "localhost:8080/v1/projects?limit=10" -H "authorization: Bearer $TOKEN"
```

## 分层与新增功能

依赖只能指向内层：`core ← config ← domain ← application ← {infrastructure, api} ← server`。
新增功能复制 `Project` 切片，见 [architecture.md](architecture.md) 与 [tiers.md](tiers.md)。

## 迁移

按档分目录、预留版本区间（见 [tiers.md](tiers.md#迁移分档)）。新增迁移只追加、不修改已合并文件。
启用档对应目录由 `infrastructure::db::run_migrations` 按序执行。

## 排错

- Windows 下 `localhost` 优先解析 IPv6 导致 sqlx 连接慢 → 连接串用 `127.0.0.1`。
- 本机已有 Postgres 占用 5432 → `export DMS_DB_PORT=5433` 起容器，连接串同步改端口。
- 切换 feature 档后二进制同名，需重新 `cargo build`/`run` 覆盖。
