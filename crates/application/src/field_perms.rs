//! 字段级权限（field-perms feature）：**通用机制**，不含业务字段规则。
//!
//! 同一行可见、但部分字段需单独授权才显示——这是模板提供的「应用层裁剪」机制
//! （见 `docs/notes/field-permissions-and-custom-entities.md`，最省最通用的一档）：
//!
//! 1. [`FieldPolicy`] 端口按「主体 + 实体类型」解析出一张 [`FieldAccessMap`]
//!    （字段 → 访问级别）。模板只定义端口与默认 [`AllVisible`]；**哪些字段敏感、
//!    需要何种权限属于业务**，由基础设施侧实现（DB / 配置 / Schema 注册表驱动）。
//! 2. handler 把响应序列化为 [`serde_json::Value`] 后，用 [`apply_field_access`]
//!    （或列表用 [`apply_field_access_each`]）就地裁剪，再返回。
//!
//! 需要 DB 层兜底（脱敏视图 / 拆敏感列表 + RLS）时，复用多租户同款「请求事务内 set
//! 会话变量」机制——属业务落地，不在模板内置。

use std::collections::HashMap;

use async_trait::async_trait;
use dms_core::{CoreResult, RequestContext};
use serde_json::Value;

/// 单个字段的访问级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldAccess {
    /// 原样返回。
    Visible,
    /// 保留字段名但值替换为 [`REDACTED`]（前端知道字段存在、但无权查看）。
    Masked,
    /// 整个字段从响应中移除（前端无从得知其存在）。
    Hidden,
}

/// 字段访问决策：字段名 → 级别。**未列出的字段默认 [`FieldAccess::Visible`]**。
pub type FieldAccessMap = HashMap<String, FieldAccess>;

/// `Masked` 字段的占位值。
pub const REDACTED: &str = "***";

/// 按决策就地裁剪一个 JSON 对象（非对象则原样不动）。
pub fn apply_field_access(value: &mut Value, access: &FieldAccessMap) {
    let Value::Object(map) = value else { return };
    for (field, level) in access {
        match level {
            FieldAccess::Visible => {}
            FieldAccess::Masked => {
                if let Some(slot) = map.get_mut(field) {
                    *slot = Value::String(REDACTED.to_string());
                }
            }
            FieldAccess::Hidden => {
                map.remove(field);
            }
        }
    }
}

/// 对数组里的每个元素套用同一决策（列表响应）；非数组等价于 [`apply_field_access`]。
pub fn apply_field_access_each(value: &mut Value, access: &FieldAccessMap) {
    match value {
        Value::Array(items) => {
            for item in items {
                apply_field_access(item, access);
            }
        }
        other => apply_field_access(other, access),
    }
}

/// 字段级权限策略端口：按主体 + 实体类型解析字段访问决策。
///
/// 模板只定义端口与默认实现；具体规则属业务，由基础设施侧实现并在组合根注入。
#[async_trait]
pub trait FieldPolicy: Send + Sync {
    /// 解析 `entity_type`（如 `"project"`）对当前请求主体的字段访问决策。
    async fn resolve(&self, ctx: &RequestContext, entity_type: &str) -> CoreResult<FieldAccessMap>;
}

/// 默认策略：所有字段可见（不裁剪）。无字段级权限需求时使用。
pub struct AllVisible;

#[async_trait]
impl FieldPolicy for AllVisible {
    async fn resolve(
        &self,
        _ctx: &RequestContext,
        _entity_type: &str,
    ) -> CoreResult<FieldAccessMap> {
        Ok(FieldAccessMap::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn policy() -> FieldAccessMap {
        FieldAccessMap::from([
            ("smiles".to_string(), FieldAccess::Hidden),
            ("mw".to_string(), FieldAccess::Masked),
            // name 未列出 → 默认可见。
        ])
    }

    #[test]
    fn redacts_object() {
        let mut v = json!({ "name": "aspirin", "mw": 180.16, "smiles": "CC(=O)O" });
        apply_field_access(&mut v, &policy());
        assert_eq!(v, json!({ "name": "aspirin", "mw": "***" }));
    }

    #[test]
    fn redacts_each_in_array() {
        let mut v = json!([
            { "name": "a", "mw": 1, "smiles": "x" },
            { "name": "b", "mw": 2, "smiles": "y" },
        ]);
        apply_field_access_each(&mut v, &policy());
        assert_eq!(
            v,
            json!([{ "name": "a", "mw": "***" }, { "name": "b", "mw": "***" }])
        );
    }

    #[test]
    fn empty_policy_is_noop() {
        let original = json!({ "a": 1, "b": 2 });
        let mut v = original.clone();
        apply_field_access(&mut v, &FieldAccessMap::new());
        assert_eq!(v, original);
    }

    #[tokio::test]
    async fn all_visible_returns_empty_map() {
        let ctx = RequestContext::for_user(
            uuid::Uuid::now_v7(),
            dms_core::TenantId::new(),
            dms_core::UserId::new(),
        );
        let map = AllVisible.resolve(&ctx, "project").await.unwrap();
        assert!(map.is_empty());
    }
}
