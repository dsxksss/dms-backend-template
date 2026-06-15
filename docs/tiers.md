# 复杂度分档（Cargo features）

模板按 Cargo feature 分档，**同一仓库**既能做精简项目，也能做企业项目。简单项目用
默认精简档零负担起步；需要时逐级开启重特性。

## 档位与内容

| 档位 (feature) | 新增能力 | 额外编译的 crate/依赖 |
|---|---|---|
| **minimal**（默认） | 纯 HTTP：`/healthz` `/readyz`(always ok) `/metrics`、分层配置、JSON 日志、RFC7807 错误 | core, config, application(port), api, server |
| **database** | 连接池、迁移、DB 就绪探针 | + infrastructure(db), sqlx |
| **multi-tenancy** | RLS 租户作用域（`begin_tenant_tx`）、`tenants` 迁移 | + uuid |
| **auth** | 身份联合（密码 + 平台 token 交换）、JIT 映射、会话、`/v1/auth/*`、RBAC、IAM 迁移、`bootstrap` 子命令 | + domain, argon2, jsonwebtoken |
| **audit** | 审计日志 + 事务性发件箱（含后台中继轮询投递） | + tokio |
| **project** | `Project` 参考切片：CRUD + RLS + 审计 + 发件箱 + 行级历史 + 乐观锁 + 软删；**容器 + 成员**（owner/manager/contributor/viewer，创建者自动属主） | + domain::project |
| **orgs** | 组织架构（对标 Benchling）：租户→组织→团队 + 资源协作者 + 带作用域角色授予 + 累积权限解析 | + domain::orgs |
| **storage** | 对象存储：内容寻址 blob（sha256 + 散列分片 `<sha[0:2]>/<sha[2:4]>/<sha>` + 去重）；文件系统后端，S3/MinIO 可换 | + sha2/hex/tokio（**不依赖 database**） |
| **field-perms** | 字段级权限**通用机制**：`FieldAccess`(Visible/Masked/Hidden) 响应裁剪 + `FieldPolicy` 端口（默认 `AllVisible`）；具体敏感字段规则属业务 | + serde_json（**纯 application 库能力**） |
| **full** | 以上全部 | 全部 |

依赖关系：`project → auth → multi-tenancy → database`；`audit → database`；`orgs → auth`；
`storage`、`field-perms` 均**独立**（不依赖 database）。开启上层会自动带上下层。

## 构建 / 运行

```bash
# 精简档（默认）—— 纯 HTTP，无需数据库
cargo run -p dms-server

# 启用数据库 / 多租户 / 完整
cargo run -p dms-server --features database
cargo run -p dms-server --features multi-tenancy
cargo run -p dms-server --features full

# 全量校验（CI 用，编译所有分档代码路径）
cargo clippy --workspace --all-features --all-targets -- -D warnings
```

> 注意：不同 feature 组合产出同名二进制 `target/debug/dms-server`，切换档位后需重新
> `cargo build`/`cargo run` 才会覆盖。

## 迁移分档

迁移按档分目录、预留版本区间，启用的档位对应目录按顺序执行（`infrastructure::db::run_migrations`，
各 migrator 开 `set_ignore_missing`）：

```
migrations/core/      0001-0099   database       扩展、set_updated_at()
migrations/tenancy/   0100-0199   multi-tenancy  app_current_tenant()、tenants、RLS 约定
migrations/auth/      0200-0299   auth           users / external_identities / refresh_tokens / RBAC
migrations/audit/     0300-0399   audit          audit_log / outbox
migrations/project/   0400-0499   project        projects / projects_history（触发器）/ project_members
migrations/orgs/      0500-0599   orgs           organizations / teams / 成员 / role_grants
```

新增业务迁移放对应档目录、沿用区间编号。

## 让简单项目永久精简（可选裁剪）

精简档本身已零负担（只编 4 个 crate、不连库），**通常保留 feature 关着即可**，不影响
精简档的构建与体积。若确实想把不需要的代码物理删除：

- 删 `crates/domain`、`crates/infrastructure`，并从 workspace `Cargo.toml` 的 `members` 移除；
- 删 `crates/api/src/extract.rs`、`crates/api/src/v1/`、`crates/application/src/{auth,dto,token}.rs`、
  `crates/application/src/port/auth.rs`；
- 删 `migrations/tenancy/`、`migrations/auth/`，只留 `migrations/core/`；
- 从各 `Cargo.toml` 移除对应 optional 依赖与 `[features]` 条目；
- `bin/server/src/main.rs` 删掉 `#[cfg(feature = ...)]` 的 DB/auth 装配段。

## 选档建议

- **内部小工具 / 无状态服务**：minimal。
- **单租户带库的简单后端**：database（或 multi-tenancy 固定一个租户）。
- **多租户 SaaS / 需要登录**：auth 起步，按需加 audit/project。
- **对标 Benchling 的完整 CMS**：full。
