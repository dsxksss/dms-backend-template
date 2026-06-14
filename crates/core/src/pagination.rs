//! 分页原语。
//!
//! 模板默认用 limit/offset 分页（简单、够用）。如需大数据量游标分页，
//! 可在此基础上扩展 `CursorRequest`，API 形态保持兼容。

use serde::{Deserialize, Serialize};

/// 单页最大条数上限，防止恶意/误用拉取过多数据。
pub const MAX_LIMIT: i64 = 100;
/// 默认每页条数。
pub const DEFAULT_LIMIT: i64 = 20;

/// 分页请求参数。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PageRequest {
    /// 每页条数。
    #[serde(default = "default_limit")]
    pub limit: i64,
    /// 偏移量。
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    DEFAULT_LIMIT
}

impl Default for PageRequest {
    fn default() -> Self {
        Self {
            limit: DEFAULT_LIMIT,
            offset: 0,
        }
    }
}

impl PageRequest {
    /// 将参数夹到安全范围：`limit ∈ [1, MAX_LIMIT]`，`offset ≥ 0`。
    /// 仓储层应始终使用夹取后的值。
    pub fn clamped(self) -> Self {
        Self {
            limit: self.limit.clamp(1, MAX_LIMIT),
            offset: self.offset.max(0),
        }
    }
}

/// 分页结果包装。
#[derive(Debug, Clone, Serialize)]
pub struct Paginated<T> {
    /// 当前页数据。
    pub items: Vec<T>,
    /// 满足条件的总条数（用于前端计算总页数）。
    pub total: i64,
    /// 回显本次的 limit。
    pub limit: i64,
    /// 回显本次的 offset。
    pub offset: i64,
}

impl<T> Paginated<T> {
    /// 由结果集、总数与分页请求组装。
    pub fn new(items: Vec<T>, total: i64, page: PageRequest) -> Self {
        let page = page.clamped();
        Self {
            items,
            total,
            limit: page.limit,
            offset: page.offset,
        }
    }

    /// 在保持分页元信息不变的前提下，对每个元素做类型映射
    /// （如 domain 实体 → API DTO）。
    pub fn map<U, F: FnMut(T) -> U>(self, f: F) -> Paginated<U> {
        Paginated {
            items: self.items.into_iter().map(f).collect(),
            total: self.total,
            limit: self.limit,
            offset: self.offset,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_limit_and_offset() {
        assert_eq!(
            PageRequest {
                limit: 1000,
                offset: 0
            }
            .clamped()
            .limit,
            MAX_LIMIT
        );
        assert_eq!(
            PageRequest {
                limit: 0,
                offset: 0
            }
            .clamped()
            .limit,
            1
        );
        assert_eq!(
            PageRequest {
                limit: 10,
                offset: -5
            }
            .clamped()
            .offset,
            0
        );
    }

    #[test]
    fn paginated_new_uses_clamped_page() {
        let p = Paginated::new(
            vec![1, 2, 3],
            3,
            PageRequest {
                limit: 9999,
                offset: -1,
            },
        );
        assert_eq!(p.limit, MAX_LIMIT);
        assert_eq!(p.offset, 0);
        assert_eq!(p.total, 3);
    }
}
