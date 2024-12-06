use std::time::Duration;
use log::debug;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("auth error: {0}")]
    AuthError(#[from] google_cloud_auth::error::Error),

    #[error("auth token error: {0}")]
    AuthTokenError(#[from] Box<dyn std::error::Error + Send + Sync>),

    #[error("reqwest: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("code: {0}, body: {1}")] // FIXME: REMOVE THIS YOU DUMB DUMBS
    Deserialize(u16, String),
}

pub async fn token() -> Result<String, Error> {
    let workload_identity_pool = std::env::var("WORKLOAD_IDENTITY_POOL").ok();
    let github_id_token_url = std::env::var("ACTIONS_ID_TOKEN_REQUEST_URL").ok();
    let github_token = std::env::var("ACTIONS_ID_TOKEN_REQUEST_TOKEN").ok();

    match (workload_identity_pool, github_id_token_url, github_token) {
        (Some(workload_identity_pool), Some(github_id_token_url), Some(github_token)) => {
            let id_token = github_id_token(&github_id_token_url, &github_token, &workload_identity_pool).await?;
            exchange_federated_token(&workload_identity_pool, &id_token.value).await
                .map(|token| token.access_token)
        }
        (_, _, _) => get_gar_auth_token().await
    }
}

pub async fn get_gar_auth_token() -> Result<String, Error> {
    debug!("Exchanging Google credential file for an oauth2 token");

    use google_cloud_auth::{project::Config, token::DefaultTokenSourceProvider};
    use google_cloud_token::TokenSourceProvider as _;

    let audience = "https://oauth2.googleapis.com/token/";
    let scopes = [
        "https://www.googleapis.com/auth/cloud-platform",
    ];

    let config = Config::default()
        .with_audience(audience)
        .with_scopes(&scopes);
    let tsp = DefaultTokenSourceProvider::new(config).await.map_err(Error::AuthError)?;
    let ts = tsp.token_source();
    let token = ts.token().await.map_err(Error::AuthTokenError)?;
    Ok(token.strip_prefix("Bearer ").unwrap_or(&token).to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TokenExchangeRequest<'a> {
    grant_type: &'a str,
    audience: &'a str,
    scope: &'a str,
    requested_token_type: &'a str,
    subject_token: &'a str,
    subject_token_type: &'a str,
}

#[derive(Deserialize)]
pub struct TokenExchangeResponse {
    pub access_token: String,
}

#[derive(Deserialize)]
pub struct GitHubTokenResponse {
    pub value: String,
}

pub async fn exchange_federated_token(workload_identity_pool: &str, github_id_token: &str) -> Result<TokenExchangeResponse, Error> {
    debug!("Exchanging federated GitHub token for an oauth2 token");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()?;
    let request = TokenExchangeRequest {
        audience: &format!("//iam.googleapis.com/{workload_identity_pool}"),
        grant_type: "urn:ietf:params:oauth:grant-type:token-exchange",
        requested_token_type: "urn:ietf:params:oauth:token-type:access_token",
        scope: "https://www.googleapis.com/auth/cloud-platform",
        subject_token_type: "urn:ietf:params:oauth:token-type:jwt",
        subject_token: github_id_token,
    };

    let resp = client.post("https://sts.googleapis.com/v1/token")
        .json(&request)
        .send()
        .await?;

    let status = resp.status().as_u16();
    let bytes = resp.bytes().await?;

    match serde_json::from_slice(&bytes) {
        Ok(token) => Ok(token),
        Err(_) => {
            let body = String::from_utf8_lossy(&bytes);
            Err(Error::Deserialize(status, body.to_string()))
        }
    }
}

pub async fn github_id_token(url: &str, bearer_token: &str, workload_identity_pool: &str) -> Result<GitHubTokenResponse, Error> {
    debug!("Getting GitHub actions id_token");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()?;

    let resp = client.get(url)
        .bearer_auth(bearer_token)
        .query(&[("audience", format!("https://iam.googleapis.com/{workload_identity_pool}"))])
        .send()
        .await?;

    let status = resp.status().as_u16();
    let bytes = resp.bytes().await?;

    match serde_json::from_slice(&bytes) {
        Ok(token) => Ok(token),
        Err(_) => {
            let body = String::from_utf8_lossy(&bytes);
            Err(Error::Deserialize(status, body.to_string()))
        }
    }
}

#[cfg(test)]
#[test]
fn test_gar_service_account_id() {
    let slug = "crm-arbeidsforhold-admin";
    let id = gar_service_account_id(slug, "nais-management-233d");
    let expected = "projects/nais-management-233d/serviceAccounts/gar-crm-arbeidsforhold-ad-4789@nais-management-233d.iam.gserviceaccount.com";
    assert_eq!(id, expected);
}

/// Derive the Google service account name for GAR deployment, given a application slug and a Google cloud project ID.
///
/// Note: this is a re-implementation of `serviceAccountNameAndAccountID` from the api-reconcilers project.
fn gar_service_account_id(slug: &str, project_id: &str) -> String {
    const GAR_SERVICE_ACCOUNT_PREFIX: &'static str = "gar";
    const GOOGLE_SERVICE_ACCOUNT_MAX_LENGTH: usize = 30;

    let account_id = slug_hash_prefix_truncate(
        slug,
        GAR_SERVICE_ACCOUNT_PREFIX,
        GOOGLE_SERVICE_ACCOUNT_MAX_LENGTH
    ).unwrap(); // unwrap ok due to correct MAX_LENGTH, this will never fail[tm]
    let email_address = format!("{account_id}@{project_id}.iam.gserviceaccount.com");
    format!("projects/{project_id}/serviceAccounts/{email_address}")
}

/// Concatenate slug and prefix into `<PREFIX>-<TRUNCATED_SLUG>-<HASH>`, where:
/// * `PREFIX` is left as-is,
/// * `TRUNCATED_SLUG` is the part of the slug that still fits into the string after everything is assembled to the maximum length, and
/// * `HASH` is the first four characters of the hex-encoded SHA256 sum of the slug.
///
/// `max_length` must be at least `prefix_len` + 6, otherwise the length of the truncated slug
/// would end up below zero. In this case, `None` is returned.
///
/// Note: this is a re-implementation of `SlugHashPrefixTruncate` from the api-reconcilers project.
fn slug_hash_prefix_truncate(slug: &str, prefix: &str, max_length: usize) -> Option<String> {
    const HASH_LENGTH: usize = 4;
    let hashed_slug = sha256::digest(slug);
    let prefix_len = prefix.len() as isize;
    let slug_length = max_length as isize - prefix_len - HASH_LENGTH as isize - 2;
    if slug_length < 0 {
        return None;
    }
    let trimmed = truncate(slug, slug_length.max(0) as usize);
    let truncated = truncate(&hashed_slug, HASH_LENGTH.max(0));
    Some(vec![prefix, trimmed, truncated].join("-").to_string())
}

#[cfg(test)]
#[test]
fn test_slug_hash_prefix_truncate() {
    const MAX_LENGTH: usize = 30;
    let slug = "crm-arbeidsforhold-admin";
    let prefix="gar";
    let expected = "gar-crm-arbeidsforhold-ad-4789";
    let result = slug_hash_prefix_truncate(slug, prefix, MAX_LENGTH).unwrap();
    assert_eq!(result, expected);
    assert_eq!(result.len(), MAX_LENGTH);
}

#[cfg(test)]
#[test]
fn test_slug_hash_prefix_truncate_out_of_bounds() {
    const MAX_LENGTH: usize = 9;
    let slug = "very-long-slug-that-must-be-truncated";
    let prefix="four";
    let result = slug_hash_prefix_truncate(slug, prefix, MAX_LENGTH);
    assert_eq!(result, None);
}


/// Helper function for truncating a string without panicking.
fn truncate(s: &str, length: usize) -> &str {
    if s.len() < length {
        s
    } else {
        &s[0..length]
    }
}

