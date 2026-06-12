# ADR-0003: 多租户隔离采用行级 + Postgres RLS

- 状态: 已接受
- 日期: 2026-06-11

## 背景

模板需用同一套代码同时支撑**云端多租户 SaaS**与**私有化单租户**。租户数据隔离是
安全红线，不能依赖应用层每个查询手写 `WHERE tenant_id = ?`（易遗漏、难审计）。

## 决策

- 每张业务表带 `tenant_id` 列，启用 **Postgres 行级安全（RLS）**，策略：
  `USING (tenant_id = app_current_tenant())`。
- 运行时以**非超级用户、非 BYPASSRLS** 的应用角色 `dms_app` 连接（超级用户会绕过
  RLS）；迁移以拥有者角色 `dms` 执行（两套连接串，见 `DatabaseConfig`）。
- 每个请求在事务开头执行 `SELECT set_config('app.current_tenant', <uuid>, true)`
  （事务局部，连接归还连接池后自动清除），由 `infrastructure::db::tenant::begin_tenant_tx`
  统一封装。`app_current_tenant()` 在未设置时返回 NULL → 策略默认拒绝（安全兜底）。
- **私有化即单租户**：固定一个 tenant，无需改代码。

## 备选方案

- Schema-per-tenant / database-per-tenant：隔离更强但迁移与连接管理更重，租户数大时
  不经济；留作后续可选。
- 纯应用层过滤：无数据库层兜底，遗漏即越权，放弃。

## 影响

- 正面：隔离下沉到数据库，应用层漏写 `WHERE` 也不会越权；审计/合规友好。
- 成本：运行时角色权限需正确配置（已由 `deploy/docker/initdb` 与默认权限处理）；
  开发者须始终经 `begin_tenant_tx` 访问租户数据——由仓储层统一约束。
- 租户注册表 `tenants` 本身不加 RLS（登录/路由阶段需在未知租户上下文时按 slug 解析）。
