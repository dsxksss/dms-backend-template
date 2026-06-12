# ADR-0002: 后端技术栈 Rust + Axum + SQLx + PostgreSQL

- 状态: 已接受
- 日期: 2026-06-11

## 背景

需为对标 Benchling 的内容管理系统选定后端栈，要求高性能、强类型保障、
可追溯、易维护，并能同时交付云端 SaaS 与私有化。

## 决策

- **语言**：Rust 1.94（edition 2024）——内存安全 + 高性能 + 强类型，适合长期演进。
- **Web**：Axum 0.8——基于 Tokio/Tower，中间件生态成熟。
- **数据库访问**：SQLx 0.8——编译期校验原生 SQL，透明可控，便于做触发器/时态表/
  审计；`.sqlx` 离线缓存支持私有化/air-gapped 无库构建。
- **数据库**：PostgreSQL 16——成熟，原生支持 RLS（行级安全），是多租户隔离的基石。
- **配置**：figment 分层；**认证**：身份联合（可插拔 IdentityProvider）；
  **观测**：tracing + Prometheus。

## 备选方案

- ORM：SeaORM（更高抽象但多一层魔法）、Diesel（async 生态较弱）——均放弃。
- Web：Actix-web——性能相近，但 Axum 与 Tower 中间件模型更契合分层架构。
- API：GraphQL——灵活但鉴权/缓存更复杂；先用 REST + OpenAPI，GraphQL 留作后续补充。

## 影响

- 正面：编译期安全覆盖到 SQL 边界；单二进制交付，私有化友好。
- 成本：Rust 学习曲线与编译时长高于脚本语言；以 workspace 分 crate + cargo-chef
  缓存缓解编译时长。

详见 [ADR-0003（多租户 RLS）](0003-multi-tenancy-rls.md) 与
[ADR-0004（身份联合）](0004-identity-federation.md)（随对应里程碑补充）。
