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
}

pub async fn token() -> Result<String, Error> {
    let workload_identity_pool = std::env::var("WORKLOAD_IDENTITY_POOL").ok();
    let github_token = std::env::var("GITHUB_TOKEN").ok();

    match (github_token, workload_identity_pool) {
        (Some(github_jwt), Some(workload_identity_pool)) =>
            exchange_federated_token(&workload_identity_pool, &github_jwt).await
                .map(|token| token.access_token),
        (_, _) => get_gar_auth_token().await
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
    #[allow(dead_code)]
    pub issued_token_type: String,
    #[allow(dead_code)]
    pub token_type: String,
    #[allow(dead_code)]
    pub expires_in: usize,
}

pub async fn exchange_federated_token(workload_identity_pool: &str, github_jwt: &str) -> Result<TokenExchangeResponse, Error> {
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
        subject_token: github_jwt,
    };
    Ok(client.post("https://sts.googleapis.com/v1/token")
        .json(&request)
        .send()
        .await?
        .json()
        .await?
    )
}