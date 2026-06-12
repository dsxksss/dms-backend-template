//! argon2 密码哈希（实现 [`PasswordHasher`] 端口）。

use argon2::Argon2;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher as _, PasswordVerifier as _, SaltString};
use dms_application::port::PasswordHasher;
use dms_core::{CoreError, CoreResult};

/// 使用 argon2id 默认参数 + 随机盐的密码哈希器。
#[derive(Default)]
pub struct Argon2PasswordHasher;

impl PasswordHasher for Argon2PasswordHasher {
    fn hash(&self, password: &str) -> CoreResult<String> {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| CoreError::internal(format!("password hashing failed: {e}")))
    }

    fn verify(&self, password: &str, hash: &str) -> bool {
        match PasswordHash::new(hash) {
            Ok(parsed) => Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok(),
            Err(_) => false,
        }
    }
}
