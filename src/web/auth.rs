use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use actix_web::{FromRequest, HttpRequest};
use actix_web::dev::Payload;
use actix_web::error::{ErrorForbidden, ErrorInternalServerError, ErrorUnauthorized};
use actix_web::http::header::AUTHORIZATION;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use sha2::{Sha256, Digest};

const AUTHORIZED_PATH: &str = "authorized_secrets";

/// this extractor makes sure that users are authorized when making special requests
pub struct Auth;
impl FromRequest for Auth {
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;
    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let secret = match req.headers().get(AUTHORIZATION) {
            Some(value) => Ok(value.to_str().unwrap_or("").to_string()),
            None => Err(ErrorUnauthorized("no secret provided"))
        };

        Box::pin(async move {
            let secret = secret?;
            if secret_authorized(&secret).await? { Ok(Auth) }
            else { Err(ErrorForbidden("invalid secret")) }
        })
    }
}

/// checks whether a given secret is authorized
async fn secret_authorized(secret: &str) -> Result<bool, actix_web::Error> {
    let mut hasher = Sha256::new();
    hasher.update(secret);
    let hashed = BASE64_STANDARD.encode(hasher.finalize());

    let file = tokio::fs::read_to_string(AUTHORIZED_PATH).await
        .map_err(|_e| ErrorInternalServerError("failed to read authorized secrets"))?;

    let secrets = file.trim()
        .split('\n')
        .filter_map(|s| s.split_whitespace().next())
        .collect::<Vec<&str>>();

    Ok(secrets.contains(&hashed.as_str()))
}
