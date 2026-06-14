# 部署指南

同一份代码、同一镜像，通过 **配置 + Cargo features** 适配多环境。档位见 [tiers.md](tiers.md)。

## 构建镜像

```bash
# 完整档（企业）
docker build -f deploy/docker/Dockerfile --build-arg DMS_FEATURES=full -t dms-server:full .
# 精简档（默认，纯 HTTP）
docker build -f deploy/docker/Dockerfile -t dms-server:minimal .
```

- 多阶段 + cargo-chef 缓存依赖；运行时为非 root 的 debian-slim。
- 迁移 SQL 编译期内嵌进二进制，运行时无需 `migrations/`，仅需 `config/`。
- **无库构建**：采用运行时查询，`cargo build` / 镜像构建均不需要数据库连接，天然适配
  CI 与 air-gapped（无需 `.sqlx` 缓存）。内网无法访问 crates.io 时，见 `.cargo/config.toml`
  配置私有 registry 镜像。

## 本地开发

```bash
# 起 Postgres（5432 被占用：export DMS_DB_PORT=5433）
docker compose -f deploy/docker/docker-compose.yml up -d postgres
cp config/local.toml.example config/local.toml && export APP_ENV=local
# 初始化租户+管理员（顺带迁移）
BOOTSTRAP_EMAIL=admin@acme.com BOOTSTRAP_PASSWORD=change-me \
  cargo run -p dms-server --features full -- bootstrap
cargo run -p dms-server --features full
```

## 私有化（onprem）：自包含栈

```bash
DMS_JWT_SECRET=$(openssl rand -hex 32) \
  docker compose -f deploy/compose/onprem.yml up -d --build
# 初始管理员
BOOTSTRAP_EMAIL=admin@corp.local BOOTSTRAP_PASSWORD=*** \
  docker compose -f deploy/compose/onprem.yml run --rm app bootstrap
```

或 Kubernetes（Helm）：

```bash
kubectl create secret generic dms-backend-secrets \
  --from-literal=DMS__DATABASE__URL='postgres://dms_app:***@pg:5432/dms' \
  --from-literal=DMS__DATABASE__MIGRATION_URL='postgres://dms:***@pg:5432/dms' \
  --from-literal=DMS__AUTH__JWT_SECRET='***'
helm install dms deploy/helm/dms-backend -f deploy/helm/dms-backend/values-onprem.yaml
```

## 云端（cloud）：应用 + 外部托管 DB

```bash
# Compose
DMS_IMAGE=registry.example.com/dms-server:full \
DMS_DATABASE_URL='postgres://app:***@db.internal:5432/dms' \
DMS_JWT_SECRET='***' \
  docker compose -f deploy/compose/cloud.yml up -d

# 或 Helm（多副本 + HPA + Ingress）
helm install dms deploy/helm/dms-backend -f deploy/helm/dms-backend/values-cloud.yaml
```

云端迁移建议由**发布流程**单独执行（`run_migrations_on_start=false`），避免多副本并发迁移。

## 配置与密钥

- 配置层级：`config/default.toml` → `config/{APP_ENV}.toml` → 环境变量 `DMS__*`（双下划线分层）。
- 密钥（`DMS__DATABASE__URL`、`DMS__AUTH__JWT_SECRET`）经环境变量 / K8s Secret / 密钥管理注入，
  **不写入镜像或配置文件**。Helm 生产用 `secret.existingSecret` 引用外部 Secret。

## RLS 运行时角色（多租户）

多租户 RLS 要求运行时以**非超级用户、非 BYPASSRLS** 角色连接。开发用 `deploy/docker/initdb`
自动创建 `dms_app`；生产/私有化请由 IaC 在运行迁移前 provision 该角色（同等约束 + 默认权限），
迁移以拥有者角色执行。详见 [ADR-0003](adr/0003-multi-tenancy-rls.md)。

## 健康与可观测性

- `/healthz`（liveness）、`/readyz`（readiness，检 DB）、`/metrics`（Prometheus）。
- 结构化 JSON 日志（云端）。K8s 探针已在 Helm chart 配好。
