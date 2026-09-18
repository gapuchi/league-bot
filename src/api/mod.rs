mod espn;
mod espn_nba;
mod football_data;

use std::fmt;

use reqwest::StatusCode;
use serde::{Deserialize, Deserializer};

pub use espn::*;
pub use espn_nba::*;
pub use football_data::*;

const RATE_LIMIT_MESSAGE: &str =
    "The sports data API is rate-limited right now. Please try again later.";

/// ESPN's edge returns 403 to unidentified clients; a crawler-style `name/version (+url)`
/// agent is accepted. Keep the repository URL — bare product tokens are still refused.
const ESPN_USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " (+",
    env!("CARGO_PKG_REPOSITORY"),
    ")"
);

/// Error shared by every upstream data client.
#[derive(Debug)]
pub enum ApiError {
    RateLimited,
    Request(reqwest::Error),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::RateLimited => write!(f, "{RATE_LIMIT_MESSAGE}"),
            ApiError::Request(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ApiError::RateLimited => None,
            ApiError::Request(error) => Some(error),
        }
    }
}

fn check_response(response: reqwest::Response) -> Result<reqwest::Response, ApiError> {
    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        return Err(ApiError::RateLimited);
    }
    response.error_for_status().map_err(ApiError::Request)
}

/// ESPN encodes ids as either JSON numbers or strings; both endpoints parse them to `i64`.
#[derive(Deserialize)]
#[serde(untagged)]
enum StringOrNumber {
    Number(i64),
    Text(String),
}

fn string_i64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    match StringOrNumber::deserialize(deserializer)? {
        StringOrNumber::Number(value) => Ok(value),
        StringOrNumber::Text(text) => text.trim().parse().map_err(serde::de::Error::custom),
    }
}

fn opt_string_i64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<i64>, D::Error> {
    match Option::<StringOrNumber>::deserialize(deserializer)? {
        None => Ok(None),
        Some(StringOrNumber::Number(value)) => Ok(Some(value)),
        Some(StringOrNumber::Text(text)) => {
            let text = text.trim();
            if text.is_empty() {
                Ok(None)
            } else {
                text.parse().map(Some).map_err(serde::de::Error::custom)
            }
        }
    }
}
