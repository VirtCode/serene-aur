pub mod requests;
pub mod data;

use reqwest::blocking::{Client, Response};
use serde::{Serialize};
use serde::de::DeserializeOwned;
use crate::config::Config;

type Result<T> = std::result::Result<T, Error>;

enum Error {
    Client {
        error: reqwest::Error,
    },
    Server {
        message: String,
    },
    Input {
        code: u16,
        message: String
    }
}

impl Error {
    pub fn print(&self) {
        match self {
            Error::Client { error } => {
                error!("failed to connect to server: {:#}", error);
            }
            Error::Server { message } => {
                error!("{}", message);
            }
            Error::Input { message, code} => {
                error!("({code}) {}", message);
            }
        }
    }
}

fn get_url(config: &Config, path: &str) -> String {
    format!("{}/{}", config.url, path)
}

fn process_errors(result: reqwest::Result<Response>) -> Result<Response> {
    let result = result.map_err(|e| Error::Client { error: e })?;

    if result.status().is_success() {
        Ok(result)
    } else if result.status().is_server_error() {
        Err(Error::Server {
            message: result.text().map_err(|e| Error::Client { error: e })?
        })
    } else {
        Err(Error::Input {
            code: result.status().as_u16(),
            message: result.text().map_err(|e| Error::Client { error: e })?
        })
    }
}

fn process_result<R: DeserializeOwned>(result: reqwest::Result<Response>) -> Result<R> {
    process_errors(result)?.json()
        .map_err(|e| Error::Client { error: e })
}

pub fn post<B: Serialize, R: DeserializeOwned>(config: &Config, path: &str, body: B) -> Result<R> {
    let result = Client::new().post(get_url(config, path))
        .header("Authorization", &config.secret)
        .json(&body)
        .send();

    process_result(result)
}

pub fn post_empty(config: &Config, path: &str) -> Result<()> {
    let result = Client::new().post(get_url(config, path))
        .header("Authorization", &config.secret)
        .send();

    process_errors(result)?;

    Ok(())
}

pub fn post_simple<B: Serialize>(config: &Config, path: &str, body: B) -> Result<()> {
    let result = Client::new().post(get_url(config, path))
        .header("Authorization", &config.secret)
        .json(&body)
        .send();

    process_errors(result)?;

    Ok(())
}

pub fn delete_empty(config: &Config, path: &str) -> Result<()> {
    let result = Client::new().delete(get_url(config, path))
        .header("Authorization", &config.secret)
        .send();

    process_errors(result)?;

    Ok(())
}

pub fn get<R: DeserializeOwned>(config: &Config, path: &str) -> Result<R> {
    let result = Client::new().get(get_url(config, path))
        .header("Authorization", &config.secret)
        .send();

    process_result(result)
}