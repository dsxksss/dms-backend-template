//! 专有/既有平台令牌交换 provider（实现 [`IdentityProvider`] 端口）。
//!
//! 本期实现：校验外部平台签发的 HS256 JWT（共享密钥），提取 `sub`/`email`/`name`。
//! 对接其它协议（OIDC + JWKS、SAML、LDAP）时，新增对应 provider 即可，应用层不变。

use async_trait::async_trait;
use dms_application::port::{IdentityProvider, VerifiedIdentity};
use dms_core::{CoreError, CoreResult};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde_json::Value;

/// 平台令牌交换提供方。
pub struct PlatformTokenProvider {
    name: String,
    decoding: DecodingKey,
    issuer: Option<String>,
}

impl PlatformTokenProvider {
    pub fn new(name: String, secret: &str, issuer: Option<String>) -> Self {
        Self {
            name,
            decoding: DecodingKey::from_secret(secret.as_bytes()),
            issuer,
        }
    }
}

#[async_trait]
impl IdentityProvider for PlatformTokenProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn verify(&self, credential: &str) -> CoreResult<VerifiedIdentity> {
        let mut validation = Validation::new(Algorithm::HS256);
        if let Some(iss) = &self.issuer {
            validation.set_issuer(&[iss]);
        }

        let data = decode::<Value>(credential, &self.decoding, &validation)
            .map_err(|_| CoreError::Unauthorized)?;
        let claims = data.claims;

        let subject = claims
            .get("sub")
            .and_then(Value::as_str)
            .ok_or(CoreError::Unauthorized)?
            .to_string();
        let email = claims
            .get("email")
            .and_then(Value::as_str)
            .map(str::to_string);
        let display_name = claims
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string);

        Ok(VerifiedIdentity {
            provider: self.name.clone(),
            subject,
            email,
            display_name,
            raw_claims: claims,
        })
    }
}
