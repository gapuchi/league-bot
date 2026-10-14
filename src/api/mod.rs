mod espn;
mod football_data;

use std::fmt;

use reqwest::StatusCode;
use serde::de::DeserializeOwned;

pub use espn::*;
pub use football_data::*;

const RATE_LIMIT_MESSAGE: &str =
    "The sports data API is rate-limited right now. Please try again later.";
const FULL_BODY_LOG_LIMIT: usize = 64 * 1024;
const ERROR_WINDOW: usize = 4 * 1024;

/// Error shared by every upstream data client.
#[derive(Debug)]
pub enum ApiError {
    RateLimited,
    MissingToken(&'static str),
    Request(reqwest::Error),
    Decode(serde_json::Error),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::RateLimited => write!(f, "{RATE_LIMIT_MESSAGE}"),
            ApiError::MissingToken(var) => {
                write!(f, "{var} is not set; this action needs that API token")
            }
            ApiError::Request(error) => write!(f, "{error}"),
            ApiError::Decode(error) => write!(f, "error decoding response body: {error}"),
        }
    }
}

impl std::error::Error for ApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ApiError::RateLimited | ApiError::MissingToken(_) => None,
            ApiError::Request(error) => Some(error),
            ApiError::Decode(error) => Some(error),
        }
    }
}

async fn decode_json<T: DeserializeOwned>(response: reqwest::Response) -> Result<T, ApiError> {
    let url = response.url().to_string();
    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let bytes = response.bytes().await.map_err(ApiError::Request)?;
    match serde_json::from_slice(&bytes) {
        Ok(value) => Ok(value),
        Err(error) => {
            let body = String::from_utf8_lossy(&bytes);
            let type_label = if content_type.is_empty() {
                status.to_string()
            } else {
                format!("{status} {content_type}")
            };
            eprintln!(
                "error decoding response body from {url} ({type_label}): {error}\n{}",
                decode_error_body(&body, &error)
            );
            Err(ApiError::Decode(error))
        }
    }
}

fn decode_error_body(body: &str, error: &serde_json::Error) -> String {
    if body.len() <= FULL_BODY_LOG_LIMIT {
        return body.to_string();
    }
    let at = error_byte_index(body, error.line(), error.column());
    let start = floor_char_boundary(body, at.saturating_sub(ERROR_WINDOW / 2));
    let end = ceil_char_boundary(body, (at + ERROR_WINDOW / 2).min(body.len()));
    format!(
        "response is {} bytes; showing {start}..{end} around line {} column {}\n{}",
        body.len(),
        error.line(),
        error.column(),
        &body[start..end]
    )
}

fn error_byte_index(body: &str, line: usize, column: usize) -> usize {
    if line == 0 {
        return body.len();
    }
    let mut current_line = 1usize;
    let mut current_column = 1usize;
    for (index, ch) in body.char_indices() {
        if current_line == line && current_column == column {
            return index;
        }
        if ch == '\n' {
            current_line += 1;
            current_column = 1;
        } else {
            current_column += 1;
        }
    }
    body.len()
}

fn floor_char_boundary(body: &str, mut index: usize) -> usize {
    if index > body.len() {
        return body.len();
    }
    while index > 0 && !body.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn ceil_char_boundary(body: &str, mut index: usize) -> usize {
    if index > body.len() {
        return body.len();
    }
    while index < body.len() && !body.is_char_boundary(index) {
        index += 1;
    }
    index
}

fn check_response(response: reqwest::Response) -> Result<reqwest::Response, ApiError> {
    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        return Err(ApiError::RateLimited);
    }
    response.error_for_status().map_err(ApiError::Request)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_decode_error_logs_the_whole_body() {
        let body = "{\"message\":\"not a scoreboard\"}";
        let error = serde_json::from_str::<Vec<i64>>(body).unwrap_err();
        assert_eq!(decode_error_body(body, &error), body);
    }

    #[test]
    fn large_decode_error_logs_the_failing_region() {
        let body = format!("[1,{} \"bad\"]", "2,".repeat(FULL_BODY_LOG_LIMIT));
        let error = serde_json::from_str::<Vec<i64>>(&body).unwrap_err();
        let logged = decode_error_body(&body, &error);
        assert!(logged.contains("response is"));
        assert!(logged.contains("\"bad\""));
        assert!(!logged.contains("[1,"));
    }
}
