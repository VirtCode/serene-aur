pub mod add;

use chrono::{DateTime, Utc};
use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize};
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
                error!("failed to make request");
                error!("{:#}", error);
            }
            Error::Server { message } => {
                error!("failed to process request");
                error!("{}", message);
            }
            Error::Input { message, code} => {
                error!("{}", message);
            }
        }
    }
}




fn get_url(config: &Config, path: &str) -> String {
    format!("{}/{}", config.url, path)
}

fn process_result<R: DeserializeOwned>(result: reqwest::Result<Response>) -> Result<R> {
    let result = result.map_err(|e| Error::Client { error: e })?;

    if result.status().is_success() {
        result.json().map_err(|e| Error::Client { error: e })
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

fn post<B: Serialize, R: DeserializeOwned>(config: &Config, path: &str, body: B) -> Result<R> {
    let result = Client::new().post(get_url(config, path))
        .header("Authorization", &config.secret)
        .json(&body)
        .send();

    process_result(result)
}

fn get<R: DeserializeOwned>(config: &Config, path: &str) -> Result<R> {
    let result = Client::new().get(get_url(config, path))
        .header("Authorization", &config.secret)
        .send();

    process_result(result)
}