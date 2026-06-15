# 文档索引

## 上手
- [/README.md](../README.md) — 项目简介、复杂度分档、快速开始
- [development.md](development.md) — 本地开发、常用命令、排错
- [deployment.md](deployment.md) — 多环境部署（Docker / compose / Helm）、密钥、RLS 角色

## 设计
- [architecture.md](architecture.md) — 六边形分层、依赖规则、「如何新增功能（复制 Project 切片）」
- [tiers.md](tiers.md) — 复杂度分档（Cargo features：minimal → full）、迁移分档、裁剪指南

## 架构决策记录（ADR）
- [0001](adr/0001-record-architecture-decisions.md) — 采用 ADR 记录决策
- [0002](adr/0002-tech-stack.md) — 技术栈（Rust + Axum + SQLx + PostgreSQL）
- [0003](adr/0003-multi-tenancy-rls.md) — 多租户：行级 + Postgres RLS
- [0004](adr/0004-identity-federation.md) — 认证：身份联合 + JIT 账号映射
- [0005](adr/0005-feature-tiers.md) — 复杂度分档（Cargo features）

## 评估备忘（调研结论，**未实现**，待业务需求明确）
- [notes/field-permissions-and-custom-entities.md](notes/field-permissions-and-custom-entities.md)
  — 字段级权限 + 用户自定义实体（含实测结论与成本分档建议）
- [notes/datasets-huggingface-reference.md](notes/datasets-huggingface-reference.md)
  — 数据集模块对标 HuggingFace（Gitea+LFS + Parquet + DuckDB 低成本方案，含 PoC 实测）
