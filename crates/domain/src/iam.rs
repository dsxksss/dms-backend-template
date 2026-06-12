//! 身份与访问管理领域模型。

use dms_core::{TenantId, UserId};
use serde::{Deserialize, Serialize};

/// 用户状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    Active,
    Suspended,
}

impl UserStatus {
    /// 数据库字符串表示。
    pub fn as_str(&self) -> &'static str {
        match self {
            UserStatus::Active => "active",
            UserStatus::Suspended => "suspended",
        }
    }

    /// 从数据库字符串解析（未知值兜底为 Suspended，倾向安全）。
    pub fn from_db(s: &str) -> Self {
        match s {
            "active" => UserStatus::Active,
            _ => UserStatus::Suspended,
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, UserStatus::Active)
    }
}

/// 租户（多租户根）。
#[derive(Debug, Clone)]
pub struct Tenant {
    pub id: TenantId,
    pub slug: String,
    pub name: String,
    pub status: String,
}

impl Tenant {
    pub fn is_active(&self) -> bool {
        self.status == "active"
    }
}

/// 内部规范用户账号——RBAC、租户、审计都挂在它上面。
#[derive(Debug, Clone)]
pub struct User {
    pub id: UserId,
    pub tenant_id: TenantId,
    pub email: String,
    /// 本地密码哈希（argon2）。外部身份用户为 `None`。
    pub password_hash: Option<String>,
    pub display_name: String,
    pub status: UserStatus,
    pub version: i32,
}
