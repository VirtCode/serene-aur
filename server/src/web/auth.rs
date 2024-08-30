use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use actix_web::{FromRequest, HttpRequest};
use actix_web::dev::Payload;
use actix_web::error::{ErrorBadRequest, ErrorForbidden, ErrorInternalServerError, ErrorServiceUnavailable, ErrorUnauthorized};
use actix_web::http::header::AUTHORIZATION;
use actix_web::web::Query;
use futures::FutureExt;
use serene_data::secret;
use crate::config::CONFIG;

const AUTHORIZED_PATH: &str = "authorized_secrets";

/// this extractor makes sure that users are authorized when making special requests
pub struct AuthWrite(String);
impl FromRequest for AuthWrite {
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;
    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {

        let secret = match req.headers().get(AUTHORIZATION) {
            Some(value) => Ok(value.to_str().unwrap_or("").to_string()),
            None => Err(ErrorUnauthorized("no secret provided"))
        };

        Box::pin(async move {
            let secret = secret?;
            if secret_authorized(&secret).await? { Ok(Self(secret)) }
            else { Err(ErrorForbidden("invalid secret")) }
        })
    }
}

impl AuthWrite {
    pub fn get_secret(&self) -> &String { &self.0 }
}

pub struct AuthRead(Option<String>);
impl FromRequest for AuthRead {
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        if CONFIG.allow_reads {
            // always allow
            Box::pin(async { Ok(Self(None)) })
        }
        else {
            let req = req.clone();

            Box::pin(async move {
                let mut payload = Payload::None;

                // delegate processing to write auth
                AuthWrite::from_request(&req.clone(), &mut payload).await.map(|a| Self(Some(a.0)))
            })
        }
    }
}

impl AuthRead {
    pub fn get_secret(&self) -> &Option<String> { &self.0 }
}

pub struct AuthWebhook(String);
impl FromRequest for AuthWebhook {
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let params = Query::<HashMap<String, String>>::from_query(req.query_string()).expect("Should accept any query params");
        let webhook_secret = params.into_inner().get("secret").ok_or(ErrorUnauthorized("no webhook secret provided")).cloned();
        let parameters: HashMap<String, String> = req.match_info().iter().map(|(k,v)| (k.to_string(), v.to_string())).collect();
        let name = parameters.get("name").ok_or(ErrorBadRequest("no package name parameter found")).cloned();

        Box::pin(async move {
            let webhook_secret = webhook_secret?;
            let secrets = get_secrets().await?;
            let name = name?;

            for authorized_secret in secrets.into_iter() {
                if create_webhook_secret(&name, &authorized_secret)?.eq(&webhook_secret) {
                    return Ok(Self(webhook_secret));
                }
            }

            return Err(ErrorForbidden("no signing secret found"))
        })
    }
}

impl AuthWebhook {
    pub fn get_secret(&self) -> &String { &self.0 }
}

/// get all authorized secrets
async fn get_secrets() -> actix_web::Result<Vec<String>> {
    let file = tokio::fs::read_to_string(AUTHORIZED_PATH).await
        .map_err(|_e| ErrorInternalServerError("failed to read authorized secrets"))?;

    let secrets = file.trim()
        .split('\n')
        .filter_map(|s| s.split_whitespace().next().map(|str| str.to_string()))
        .collect::<Vec<String>>();
    Ok(secrets)
}

/// checks whether a given secret is authorized
async fn secret_authorized(secret: &str) -> Result<bool, actix_web::Error> {
    let secrets = get_secrets().await?;
    Ok(secrets.contains(&secret::hash(secret)))
}

/// create a secret which can be used for webhooks for a given package
pub fn create_webhook_secret(package: &String, authorized_secret: &String) -> actix_web::Result<String> {
    let server_secret = CONFIG.webhook_secret.clone().ok_or(ErrorServiceUnavailable("webhooks aren't enabled on this server"))?;
    let secret_str = format!("{authorized_secret}-{package}-{server_secret}");
    Ok(secret::hash_url_safe(secret_str.as_str()))
}