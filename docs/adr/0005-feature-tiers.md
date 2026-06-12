# ADR-0005: 用 Cargo features 做复杂度分档

- 状态: 已接受
- 日期: 2026-06-11

## 背景

模板会被多个项目复用，但并非每个项目都需要多租户 / 认证 / 审计。完整企业栈（7 crate
六边形 + RLS + 身份联合）对简单项目偏重，违背"易上手"。

## 决策

**单仓库 + Cargo features 分档**，不维护多套代码、不重写已建实现：

- 档位：`minimal`(默认) → `database` → `multi-tenancy` → `auth` → `audit`/`project` → `full`，
  上层自动带下层（详见 [docs/tiers.md](../tiers.md)）。
- `dms-domain`、`dms-infrastructure` 设为 `dms-server` 的 **optional 依赖**，由 feature 经
  `dep:` 启用；各 crate 模块用 `#[cfg(feature)]` 门控；`AppState` 字段与 `main.rs` 装配段
  条件编译。
- 精简档注入 `AlwaysReady` 就绪探针，完全不连数据库。
- 迁移按档分目录（`core`/`tenancy`/`auth`）+ 预留版本区间，`run_migrations` 按启用档顺序
  执行并对各 migrator 开 `set_ignore_missing`（每个 migrator 只含本档迁移）。

## 备选方案

- cargo-generate 模板化（选档生成）：起步更干净，但需额外维护模板变量；留作后续。
- 现在就合并 crate 简化基线：要重构已验证的 M0–M3，收益不抵风险。

## 影响

- 正面：简单项目 `cargo run -p dms-server` 即起，仅编译 4 个 crate、无 DB 依赖；企业项目
  `--features full`。同一份代码服务两类需求。
- 成本：库 crate 的**完整类型检查需 `--all-features`**（CI 已采用）；新增功能要明确归档。
- 已验证：minimal / multi-tenancy / full 三档均构建通过、clippy 全量无警；精简档运行不连库、
  full 档迁移分档落库且认证流程正常。
