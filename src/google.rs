use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("auth error: {0}")]
    AuthError(#[from] google_cloud_auth::error::Error),

    #[error("auth token error: {0}")]
    AuthTokenError(#[from] Box<dyn std::error::Error + Send + Sync>),
}

pub async fn get_gar_auth_token() -> Result<String, Error> {
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

