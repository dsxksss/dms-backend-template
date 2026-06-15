# 架构总览

## 分层（六边形 / Clean Architecture）

依赖只能指向内层。`domain` 不依赖任何 I/O；`infrastructure` 实现 `domain` 定义的
端口（trait）；`bin/server` 是唯一组合根，负责装配具体实现并注入。

```
        ┌─────────────────────────────────────────────┐
        │                  bin/server                  │  组合根：装配 + 启动
        └───────────────┬───────────────┬─────────────┘
                        │               │
                ┌───────▼──────┐   ┌────▼──────────┐
                │     api      │   │ infrastructure │  Axum 路由 / SQLx 仓储实现
                └───────┬──────┘   └────┬──────────┘
                        │               │ (实现端口)
                ┌───────▼───────────────▼──────┐
                │         application           │  用例服务 / DTO / 事务编排
                └───────────────┬──────────────┘
                        ┌───────▼──────┐
                        │    domain    │  实体 / 值对象 / 事件 / 端口(trait)
                        └───────┬──────┘
                        ┌───────▼──────┐
                        │     core     │  错误 / 强类型ID / 分页 / 上下文 / 观测
                        └──────────────┘
```

| crate | 职责 | 关键约束 |
|---|---|---|
| `dms-core` | 错误模型、`Id<T>`、分页、`RequestContext`、telemetry | 无 I/O，无业务 |
| `dms-domain` | 实体、值对象、领域事件、仓储端口 | 纯业务，不依赖 sqlx/axum |
| `dms-application` | 用例服务、命令/查询、DTO、事务边界 | 仅依赖 domain 端口 |
| `dms-infrastructure` | SQLx 仓储、认证 provider、审计、发件箱 | 实现 domain 端口 |
| `dms-api` | HTTP 路由、中间件、错误映射（RFC 7807） | 依赖 application，不依赖 infrastructure |
| `dms-config` | figment 分层配置 | 依赖 core |
| `dms-server` (bin) | 组合根：装配依赖、启动、优雅停机 | 唯一知道全部具体实现的地方 |

## 实际模块结构（与代码对齐）

```
crates/core/src/         error · id (Id<T>) · ids (TenantId/UserId) · pagination · context · telemetry
crates/config/src/       lib (AppConfig：figment 分层)
crates/domain/src/       iam · ports · project · orgs                # [auth]/[project]/[orgs] 档
crates/application/src/  port/{health,auth,storage} · auth(AuthService) · dto · token · project · orgs   # [auth]/[project]/[orgs]/[storage]
crates/infrastructure/src/  db/{mod,tenant} · health          # [database]/[multi-tenancy]
                            auth/{hasher,jwt,platform} · iam/{repos,bootstrap}   # [auth]
                            audit · outbox · project · orgs · storage   # [audit]/[project]/[orgs]/[storage]
crates/api/src/          error · health · state · extract(AuthContext) · v1/{auth,projects,orgs}
bin/server/src/          main (组合根 + bootstrap 子命令)
migrations/              core/ · tenancy/ · auth/ · audit/ · project/ · orgs/   # 按档分目录，预留版本区间
```

> 方括号标注该模块所属 Cargo feature 档；精简档（默认）只编译 core/config/application(port)/api/server。

## 依赖注入

`api::AppState` 以 `Arc<dyn Trait>` 持有 application 用例服务；`bin/server` 用
infrastructure 的具体实现（如 `PgProjectRepository`）构造 application 服务并注入。
这样替换实现（换 DB、加缓存、Mock 测试）不触及 api/domain。

> 各层模块按 Cargo feature 以 `#[cfg(feature)]` 门控，`dms-domain`/`dms-infrastructure`
> 是 `dms-server` 的可选依赖——精简档不编译它们。分档详见 [tiers.md](tiers.md) 与 ADR-0005。

## 如何新增一个功能（复制 Project 切片）

1. `domain`：定义实体 + 仓储端口 trait + 领域事件。
2. `application`：定义用例服务（依赖端口）+ DTO + 校验。
3. `infrastructure`：用 SQLx 实现端口，事务内写审计与 outbox。
4. `migrations`：在对应档目录（core/tenancy/auth/audit/project，见 tiers.md）新增表 + RLS
   策略 + history 触发器（审计/发件箱由仓储在事务内写）。
5. `api`：挂 `/v1/<feature>` 路由 + RBAC 守卫（OpenAPI 注解为计划项）。
6. `bin/server`：在组合根注入新服务，并归入合适的 Cargo feature 档。
7. 测试：领域单元 / 服务(mock 仓储) / 集成(真实 PG，默认 `#[ignore]`) / API e2e。

参考实现见 `Project`（M4 起）。

## 横切关注点

- **多租户**：行级 + Postgres RLS，每请求事务设 `app.current_tenant`（见 ADR-0003）。
- **可追溯**：审计日志 + `*_history` 表 + 事务性发件箱 + request_id 链路。
- **认证**：身份联合（可插拔 IdentityProvider + JIT 账号映射，见 ADR-0004）。
- **组织/权限**：`orgs` 档提供 租户→组织→团队 层级 + 带作用域角色授予（`role_grants`）+ 累积
  权限解析（对标 Benchling，见 ADR-0006）；资源级权限按请求解析、不入 JWT。
- **多环境**：figment 分层（`APP_ENV`：local/cloud/onprem）+ 同镜像配置驱动；交付（Docker/
  compose/Helm）见 [deployment.md](deployment.md)。
- **复杂度分档**：Cargo features（`minimal`→`full`）按需编译 database/multi-tenancy/auth/
  audit/project 子系统（见 [tiers.md](tiers.md)、ADR-0005）。
