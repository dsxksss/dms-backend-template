//! 强类型标识符 [`Id<T>`]。
//!
//! 在 `Uuid` 之上加一层零成本的类型标记，使 `Id<User>` 与 `Id<Tenant>` 在
//! 编译期不可互换——杜绝"把用户 ID 传给租户参数"这类 bug，是「易维护」的
//! 关键工具。运行时表示就是一个 `Uuid`（UUIDv7，时间有序，索引友好）。
//!
//! 领域层用纯 `Id<T>`（不耦合 sqlx）；基础设施层的 DB 行结构体用裸 `Uuid`，
//! 经 `.into()` 在边界互转。

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

/// 以类型 `T` 为标记的强类型 ID。`T` 通常是空枚举标记（见 [`crate::ids`]）。
///
/// `PhantomData<fn() -> T>` 让 `Id<T>` 无条件实现 `Send`/`Sync`/`Copy`，
/// 且对 `T` 协变，不要求 `T: Copy`。
pub struct Id<T> {
    value: Uuid,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    /// 生成新的时间有序 ID（UUIDv7）。
    pub fn new() -> Self {
        Self::from_uuid(Uuid::now_v7())
    }

    /// 由已有 `Uuid` 包装（DB/外部边界使用）。
    pub const fn from_uuid(value: Uuid) -> Self {
        Self {
            value,
            _marker: PhantomData,
        }
    }

    /// 取出底层 `Uuid`。
    pub const fn as_uuid(&self) -> Uuid {
        self.value
    }
}

impl<T> Default for Id<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T> Eq for Id<T> {}

impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.value, f)
    }
}

impl<T> fmt::Display for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.value, f)
    }
}

impl<T> From<Uuid> for Id<T> {
    fn from(value: Uuid) -> Self {
        Self::from_uuid(value)
    }
}

impl<T> From<Id<T>> for Uuid {
    fn from(id: Id<T>) -> Self {
        id.value
    }
}

impl<T> FromStr for Id<T> {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from_uuid(Uuid::parse_str(s)?))
    }
}

impl<T> Serialize for Id<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.value.serialize(serializer)
    }
}

impl<'de, T> Deserialize<'de> for Id<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self::from_uuid(Uuid::deserialize(deserializer)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    enum Foo {}
    enum Bar {}

    #[test]
    fn roundtrips_through_uuid() {
        let id: Id<Foo> = Id::new();
        let raw: Uuid = id.into();
        assert_eq!(id, Id::<Foo>::from_uuid(raw));
    }

    #[test]
    fn parses_and_displays() {
        let id: Id<Foo> = Id::new();
        let parsed: Id<Foo> = id.to_string().parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn distinct_markers_are_independent_types() {
        // 这段仅用于说明：下面这行若取消注释将无法编译，证明类型隔离生效。
        // let _: Id<Bar> = Id::<Foo>::new();
        let _foo: Id<Foo> = Id::new();
        let _bar: Id<Bar> = Id::new();
    }
}
