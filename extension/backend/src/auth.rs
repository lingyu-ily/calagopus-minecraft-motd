use axum::http::{HeaderMap, StatusCode};
use sha2::{Digest, Sha256};
use shared::{State, response::ApiResponse};
use sqlx::Row;

pub fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

pub fn bearer(headers: &HeaderMap) -> Result<&str, ApiResponse> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ApiResponse::error("missing agent bearer token").with_status(StatusCode::UNAUTHORIZED)
        })
}

pub async fn authenticate_agent(
    state: &State,
    headers: &HeaderMap,
) -> Result<uuid::Uuid, ApiResponse> {
    let token = bearer(headers)?;
    let hash = token_hash(token);
    let row = sqlx::query(
        "SELECT node_uuid FROM ily_gfs_minecraftmotd_agents WHERE credential_hash = $1 AND revoked_at IS NULL",
    )
    .bind(hash)
    .fetch_optional(state.database.read())
    .await
    .map_err(|err| ApiResponse::error(err.to_string()))?;

    row.map(|row| row.get("node_uuid")).ok_or_else(|| {
        ApiResponse::error("invalid or revoked agent token").with_status(StatusCode::UNAUTHORIZED)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_hash_is_stable_and_does_not_store_plaintext() {
        let hash = token_hash("secret-agent-token");
        assert_eq!(hash, token_hash("secret-agent-token"));
        assert_ne!(hash, token_hash("different-agent-token"));
        assert_eq!(hash.len(), 32);
        assert_ne!(hash, b"secret-agent-token");
    }

    #[test]
    fn bearer_requires_the_expected_scheme_and_value() {
        let mut headers = HeaderMap::new();
        assert!(bearer(&headers).is_err());
        headers.insert(
            axum::http::header::AUTHORIZATION,
            "Basic abc".parse().unwrap(),
        );
        assert!(bearer(&headers).is_err());
        headers.insert(
            axum::http::header::AUTHORIZATION,
            "Bearer agent-token".parse().unwrap(),
        );
        assert_eq!(bearer(&headers).unwrap(), "agent-token");
    }
}
