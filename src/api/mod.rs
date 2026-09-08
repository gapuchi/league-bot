mod football_data;

use std::fmt;

use reqwest::StatusCode;

pub use football_data::*;

const RATE_LIMIT_MESSAGE: &str =
    "The sports data API is rate-limited right now. Please try again later.";

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
