use std::fmt;
use std::sync::Arc;

use rusqlite::Connection;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct Data {
    pub db: Arc<Mutex<Connection>>,
    pub http: reqwest::Client,
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;

/// Error whose text is meant for the invoking Discord user. poise's default `on_error`
/// replies with the error's `Display`, so returning this from a use case is enough.
#[derive(Debug)]
pub struct UserError(pub String);

impl fmt::Display for UserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for UserError {}
