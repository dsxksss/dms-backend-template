# DMS Backend

可复用的 **Rust + PostgreSQL 企业级后端启动模板**。面向内容管理系统（对标 [Benchling](https://benchling.com/)），强调**可迭代、可追溯、易维护**，并支持**云端多租户 SaaS** 与**私有化单租户/离线**两类部署。

> 这是一个工程底座，不含具体业务功能。新功能通过复制 `Project` 参考切片快速展开（见 `docs/architecture.md`）。

## 技术栈

| 维度 | 选型 |
|---|---|
| 语言 | Rust 1.94 (edition 2024) |
| Web | Axum 0.8 + tower-http |
| 数据库 | PostgreSQL 16 + SQLx 0.8（默认运行时查询，构建无需活库；`query!` 编译期校验为可选） |
| 多租户 | 行级 + Postgres RLS |
| 认证 | 身份联合（内置密码 + 专有平台 token 交换，JIT 账号映射） |
| API 文档 | OpenAPI / `/docs`（计划中的可选 feature，当前未内置） |
| 配置 | figment 分层（`default → {env} → ENV`） |
| 观测 | tracing(JSON) + Prometheus 指标 |

## 架构分层

```
core ← config ← domain ← application ← {infrastructure, api} ← server(组合根)
```

依赖只能指向内层；`domain` 无 I/O，`infrastructure` 实现 `domain` 端口，`bin/server` 是唯一装配点。详见 [docs/architecture.md](docs/architecture.md) 与 [docs/adr/](docs/adr/)。

## 复杂度分档（精简 ↔ 企业）

同一仓库用 Cargo features 分档：简单项目零负担起步，企业项目按需开启。详见 [docs/tiers.md](docs/tiers.md)。

| 档位 | 内容 | 起步命令 |
|---|---|---|
| `minimal`（默认） | 纯 HTTP：`/healthz` `/readyz` `/metrics`，**无需数据库** | `cargo run -p dms-server` |
| `database` | + 连接池 / 迁移 / 就绪探针 | `... --features database` |
| `multi-tenancy` | + 行级 RLS 多租户 | `... --features multi-tenancy` |
| `auth` | + 身份联合 / 会话 / RBAC / `/v1/auth` | `... --features auth` |
| `full` | 全部企业能力 | `... --features full` |

依赖：`project → auth → multi-tenancy → database`，`audit → database`（上层自动带下层）。

## 快速开始

### 精简档（无需数据库）

```bash
cargo run -p dms-server
curl localhost:8080/healthz      # {"status":"ok"}
```

### 完整档（多租户 + 认证）

前置：Rust 1.94、Docker。

```bash
# 1. 起本地 Postgres（若本机 5432 被占用：export DMS_DB_PORT=5433 再起）
docker compose -f deploy/docker/docker-compose.yml up -d postgres

# 2. 用本地配置（含数据库连接串与启动自动迁移）
cp config/local.toml.example config/local.toml
export APP_ENV=local

# 3. 初始化首个租户 + 管理员（顺带执行迁移）
BOOTSTRAP_EMAIL=admin@acme.com BOOTSTRAP_PASSWORD=change-me \
  cargo run -p dms-server --features full -- bootstrap

# 4. 启动服务
cargo run -p dms-server --features full

# 5. 登录拿令牌
curl -s localhost:8080/v1/auth/login -H 'content-type: application/json' \
  -d '{"tenant":"acme","email":"admin@acme.com","password":"change-me"}'
```

## 常用命令

```bash
cargo build  --workspace                               # 构建（默认精简档；分档见 docs/tiers.md）
cargo test   --workspace --all-features                # 单元 + mock（集成测试默认 #[ignore]）
cargo fmt    --all -- --check                          # 格式
cargo clippy --workspace --all-features --all-targets -- -D warnings   # Lint
cargo deny check                                       # 供应链门禁
# 真实库集成测试：
# TEST_DATABASE_URL=postgres://dms:dms@127.0.0.1:5433/dms \
#   cargo test -p dms-infrastructure --features project -- --ignored
```

## 多环境

| 环境 | `APP_ENV` | 配置文件 | 说明 |
|---|---|---|---|
| 本地 | `local` | `config/local.toml` | 单租户，开发用 |
| 云端 | `cloud` | `config/cloud.toml` | 多租户 SaaS |
| 私有化 | `onprem` | `config/onprem.toml` | 单租户，可离线 |

同一镜像，**运行行为**由 `APP_ENV` + 配置决定；**编译进哪些能力**由 Cargo features（`minimal`…`full`，见 [docs/tiers.md](docs/tiers.md)）决定。`cloud`/`onprem` 是环境配置（`APP_ENV` + Helm values），不是 Cargo feature。
