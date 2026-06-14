## 变更说明

<!-- 这个 PR 做了什么、为什么。关联 issue。 -->

## 检查清单

- [ ] `cargo fmt --all -- --check` 通过
- [ ] `cargo clippy --workspace --all-features --all-targets -- -D warnings` 通过
- [ ] `cargo test --workspace --all-features` 通过
- [ ] 涉及数据库：新增迁移放对应档目录（core/tenancy/auth/audit/project）并沿用版本区间
- [ ] 涉及新功能/档位：更新了相关 feature 开关与 `docs/tiers.md`
- [ ] 重要架构取舍：新增/更新了 ADR（`docs/adr/`）
- [ ] 更新了 CHANGELOG
- [ ] 提交信息遵循 Conventional Commits
