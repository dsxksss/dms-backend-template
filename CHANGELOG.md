# Changelog

本项目遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/) 与
[语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased]

_模板已冻结于 0.1.0；后续条目为基于模板的业务功能。_

## [0.1.0] - 2026-06-14 — 模板基线（冻结）

### Added

- 工作区骨架与六边形分层（core/config/domain/application/infrastructure/api + server）。
- 分层配置（figment）、结构化日志（tracing）、Prometheus 指标、健康/就绪端点、
  RFC 7807 错误响应。
- PostgreSQL 接入：SQLx 连接池、分档迁移、行级安全（RLS）多租户、`dms_app` 受限角色、
  租户作用域事务。
- 身份联合与会话：可插拔 `IdentityProvider`（内置密码 + 专有平台 token 交换）、JIT 账号
  映射、access/refresh 会话（轮换/吊销）、RBAC、`bootstrap` 子命令。
- 复杂度分档（Cargo features：minimal/database/multi-tenancy/auth/audit/project/full），
  精简档零负担起步。
- `Project` 参考切片：CRUD + RLS + 乐观锁 + 软删 + 审计日志 + 事务性发件箱（含中继）+
  行级历史。
- 多环境打包：多阶段 Dockerfile（cargo-chef）、compose（cloud/onprem）、Helm chart。
- 测试体系：单元 + mock 服务测试 + 真实库集成测试范例。
- CI/CD：GitHub Actions（fmt/clippy/分档构建/测试/cargo-deny/集成）、release 镜像构建。
- 文档：架构、分档、部署、开发、ADR-0001..0005。
- 评估备忘：字段级权限与用户自定义实体的可行性验证与成本分档建议
  （`docs/notes/field-permissions-and-custom-entities.md`，仅评估、未实现）。
- 评估备忘：数据集模块对标 HuggingFace（Gitea+LFS 版本控制 + Parquet + DuckDB 查询的低成本
  方案），含 DuckDB/git-lfs PoC 实测（`docs/notes/datasets-huggingface-reference.md`，仅评估）。
