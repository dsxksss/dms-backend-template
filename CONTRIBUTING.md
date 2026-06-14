# 贡献指南

## 环境

- Rust 1.94（`rust-toolchain.toml` 已锁定）、Docker。
- 本地起库：`docker compose -f deploy/docker/docker-compose.yml up -d postgres`。

## 提交前自检

```bash
cargo fmt --all
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test --workspace --all-features      # 单元 + mock（集成测试默认 #[ignore]）
cargo deny check                           # 供应链门禁（需 cargo install cargo-deny）
```

## 约定

- **提交信息**：Conventional Commits（`feat:` / `fix:` / `docs:` / `refactor:` / `test:` / `chore:`）。
- **架构决策**：有长期影响的取舍写入 `docs/adr/`（编号递增、只追加）。
- **变更日志**：用户可见变更记入 `CHANGELOG.md`。
- **迁移**：只追加不修改；放对应档目录（core/tenancy/auth/audit/project）并沿用版本区间。

## 新增一个功能（复制 Project 切片）

参见 [docs/architecture.md](docs/architecture.md#如何新增一个功能复制-project-切片) 与
[docs/tiers.md](docs/tiers.md)：domain 实体+端口 → application 用例 → infrastructure 仓储
（事务内写审计/发件箱）→ api 路由 → 组合根注入 → 迁移 → 测试，并归入合适的 feature 档。
