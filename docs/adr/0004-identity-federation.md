# ADR-0004: 认证采用身份联合 + JIT 账号映射

- 状态: 已接受
- 日期: 2026-06-11

## 背景

需求不止"内置登录"：要兼容第三方/既有平台，第三方账号也能登录，并在内部做账号
映射。同时模板要通用——后续可能接 OIDC、SAML、LDAP。

## 决策

- **规范账号解耦**：内部 `users` 为唯一主体（RBAC/租户/审计都挂其上）；
  `external_identities` 表记录 `(provider, external_subject) → user_id`，一人可关联
  多个外部身份。
- **可插拔 `IdentityProvider` 端口**：本期实现 `PlatformTokenProvider`（校验专有平台
  签发的 HS256 JWT，token 交换）；OIDC/SAML/LDAP 后续新增 provider 即可，应用层不变。
  内置密码登录走独立的 `PasswordHasher`（argon2），不经 `IdentityProvider`。
- **JIT 账号映射**：外部身份校验后 → 命中既有映射 → 否则按已验证 email 匹配既有用户
  → 再否则自动开通新用户并建立映射。
- **会话归我们签发**：统一签发自有 access(JWT, 含权限) + refresh(不透明、存摘要、可轮换/
  吊销)。支持密码登录（独立会话）与 token 交换（免登嵌入）两条入口。
- **RBAC**：权限内嵌 access token，避免每请求查库；access TTL 短以控制陈旧。

## 影响

- 正面：第三方账号经映射即可登录；新增外部源不改应用层；账号联动可追溯
  （external_identities + 审计）。
- 成本：access token 内权限有短暂陈旧窗口（由短 TTL 约束）；provider 配置当前在应用
  配置，后续可下沉到 per-tenant `identity_providers` 表。
- 未实现（留扩展点）：OIDC/OAuth2、SAML、LDAP provider；用户自助绑定多账号；对下游
  作 IdP（JWKS）。
