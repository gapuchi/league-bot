mod api;
mod tiebreaker;

pub(crate) mod poll;
pub(crate) mod standings;
pub(crate) mod teams;
pub mod remaining;

pub(crate) use api::football_data;
pub use tiebreaker::pick_tiebreaker_player;
pub use remaining::FetchOutcome;
