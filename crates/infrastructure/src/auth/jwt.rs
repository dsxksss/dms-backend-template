//! JWT access token 签发与校验（实现 [`TokenIssuer`] 端口）。

use chrono::Utc;
use dms_application::port::{AccessClaims, TokenIssuer};
use dms_core::{CoreError, CoreResult, TenantId, UserId};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};

/// 基于 HS256 对称密钥的 access token 签发器。
pub struct JwtTokenIssuer {
    encoding: EncodingKey,
    decoding: DecodingKey,
    issuer: String,
    access_ttl_secs: i64,
}

impl JwtTokenIssuer {
    pub fn new(secret: &str, issuer: String, access_ttl_secs: i64) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret.as_bytes()),
            decoding: DecodingKey::from_secret(secret.as_bytes()),
            issuer,
            access_ttl_secs,
        }
    }
}

impl TokenIssuer for JwtTokenIssuer {
    fn issue(&self, user: UserId, tenant: TenantId, perms: &[String]) -> CoreResult<(String, i64)> {
        let now = Utc::now().timestamp();
        let claims = AccessClaims {
            sub: user.into(),
            tenant: tenant.into(),
            iss: self.issuer.clone(),
            iat: now,
            exp: now + self.access_ttl_secs,
            perms: perms.to_vec(),
        };
        let token = encode(&Header::new(Algorithm::HS256), &claims, &self.encoding)
            .map_err(|e| CoreError::internal(format!("token signing failed: {e}")))?;
        Ok((token, self.access_ttl_secs))
    }

    fn verify(&self, token: &str) -> CoreResult<AccessClaims> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[&self.issuer]);
        decode::<AccessClaims>(token, &self.decoding, &validation)
            .map(|data| data.claims)
            .map_err(|_| CoreError::Unauthorized)
    }
}
