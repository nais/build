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
    Deserialize(u16, String)
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
        audience: workload_identity_pool,
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
