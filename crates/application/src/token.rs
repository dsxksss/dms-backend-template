//! 刷新令牌生成与摘要。
//!
//! 刷新令牌是高熵不透明串，明文只返回客户端一次；服务端只存其 SHA-256 摘要，
//! 按摘要做等值查找（确定性哈希，便于索引；不可逆，泄库不暴露明文）。

use sha2::{Digest, Sha256};

/// 生成不透明刷新令牌（约 244 bits 熵）。
pub fn generate_refresh_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// 计算令牌摘要（十六进制 SHA-256）。
pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}
