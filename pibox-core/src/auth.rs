//! JWT authentication for pibox
//!
//! Uses stateless JWT tokens:
//! - Access token: Short-lived (15 min), used for all API requests
//! - Refresh token: Long-lived (7 days), used to get new access tokens
//!
//! This design is ideal for embedded devices:
//! - No session storage needed on server
//! - Tokens are self-contained and verifiable
//! - Refresh flow allows long sessions without storing state

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// JWT claims embedded in tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (username)
    pub sub: String,
    /// Expiration time (Unix timestamp)
    pub exp: u64,
    /// Issued at (Unix timestamp)
    pub iat: u64,
    /// Token type (access or refresh)
    pub token_type: TokenType,
    /// Device ID (for multi-device tracking)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TokenType {
    Access,
    Refresh,
}

/// Pair of access and refresh tokens
#[derive(Debug, Clone)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

/// JWT authentication handler
pub struct JwtAuth {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    access_token_ttl: u64,  // seconds
    refresh_token_ttl: u64, // seconds
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Token encoding failed: {0}")]
    EncodingError(#[from] jsonwebtoken::errors::Error),

    #[error("Token has expired")]
    TokenExpired,

    #[error("Invalid token type: expected {expected:?}, got {got:?}")]
    InvalidTokenType { expected: TokenType, got: TokenType },

    #[error("Invalid credentials")]
    InvalidCredentials,
}

impl JwtAuth {
    /// Create new JWT auth handler
    ///
    /// # Arguments
    /// * `secret` - HMAC secret for signing tokens (should be >= 32 bytes)
    /// * `access_token_ttl` - Access token lifetime in seconds (default: 900 = 15 min)
    /// * `refresh_token_ttl` - Refresh token lifetime in seconds (default: 604800 = 7 days)
    pub fn new(secret: &[u8], access_token_ttl: Option<u64>, refresh_token_ttl: Option<u64>) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
            access_token_ttl: access_token_ttl.unwrap_or(900),
            refresh_token_ttl: refresh_token_ttl.unwrap_or(604800),
        }
    }

    /// Generate a new token pair for a user
    pub fn generate_tokens(&self, username: &str, device_id: Option<&str>) -> Result<TokenPair, AuthError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let access_claims = Claims {
            sub: username.to_string(),
            exp: now + self.access_token_ttl,
            iat: now,
            token_type: TokenType::Access,
            device_id: device_id.map(String::from),
        };

        let refresh_claims = Claims {
            sub: username.to_string(),
            exp: now + self.refresh_token_ttl,
            iat: now,
            token_type: TokenType::Refresh,
            device_id: device_id.map(String::from),
        };

        let access_token = encode(&Header::default(), &access_claims, &self.encoding_key)?;
        let refresh_token = encode(&Header::default(), &refresh_claims, &self.encoding_key)?;

        Ok(TokenPair {
            access_token,
            refresh_token,
            expires_in: self.access_token_ttl,
        })
    }

    /// Verify and decode an access token
    pub fn verify_access_token(&self, token: &str) -> Result<Claims, AuthError> {
        let claims = self.decode_token(token)?;

        if claims.token_type != TokenType::Access {
            return Err(AuthError::InvalidTokenType {
                expected: TokenType::Access,
                got: claims.token_type,
            });
        }

        Ok(claims)
    }

    /// Verify refresh token and generate new token pair
    pub fn refresh_tokens(&self, refresh_token: &str) -> Result<TokenPair, AuthError> {
        let claims = self.decode_token(refresh_token)?;

        if claims.token_type != TokenType::Refresh {
            return Err(AuthError::InvalidTokenType {
                expected: TokenType::Refresh,
                got: claims.token_type,
            });
        }

        self.generate_tokens(&claims.sub, claims.device_id.as_deref())
    }

    /// Decode and validate a token
    fn decode_token(&self, token: &str) -> Result<Claims, AuthError> {
        let validation = Validation::default();
        let token_data = decode::<Claims>(token, &self.decoding_key, &validation)?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if token_data.claims.exp < now {
            return Err(AuthError::TokenExpired);
        }

        Ok(token_data.claims)
    }

    /// Get the expected access token TTL
    pub fn access_token_ttl(&self) -> u64 {
        self.access_token_ttl
    }

    /// Get the expected refresh token TTL
    pub fn refresh_token_ttl(&self) -> u64 {
        self.refresh_token_ttl
    }
}

/// Generate a secure random secret for JWT signing
pub fn generate_secret() -> [u8; 32] {
    use rand::Rng;
    rand::thread_rng().r#gen()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_generation_and_verification() {
        let secret = generate_secret();
        let auth = JwtAuth::new(&secret, Some(60), Some(3600));

        let tokens = auth.generate_tokens("testuser", Some("device1")).unwrap();

        let claims = auth.verify_access_token(&tokens.access_token).unwrap();
        assert_eq!(claims.sub, "testuser");
        assert_eq!(claims.device_id, Some("device1".to_string()));
        assert_eq!(claims.token_type, TokenType::Access);
    }

    #[test]
    fn test_token_refresh() {
        let secret = generate_secret();
        let auth = JwtAuth::new(&secret, Some(60), Some(3600));

        let tokens = auth.generate_tokens("testuser", None).unwrap();

        // Refresh should succeed and produce valid tokens
        let new_tokens = auth.refresh_tokens(&tokens.refresh_token).unwrap();

        // Verify the new access token is valid
        let claims = auth.verify_access_token(&new_tokens.access_token).unwrap();
        assert_eq!(claims.sub, "testuser");
        assert_eq!(claims.token_type, TokenType::Access);

        // Using access token as refresh should fail
        let result = auth.refresh_tokens(&tokens.access_token);
        assert!(matches!(result, Err(AuthError::InvalidTokenType { .. })));
    }

    #[test]
    fn test_wrong_token_type() {
        let secret = generate_secret();
        let auth = JwtAuth::new(&secret, Some(60), Some(3600));

        let tokens = auth.generate_tokens("testuser", None).unwrap();

        // Try to use refresh token as access token
        let result = auth.verify_access_token(&tokens.refresh_token);
        assert!(matches!(result, Err(AuthError::InvalidTokenType { .. })));
    }
}
