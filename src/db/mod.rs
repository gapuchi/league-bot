mod draft;
mod guild_config;
mod league;
mod migrate;
mod registration;
mod season;
mod soccer_macros;
mod team;
mod epl;
mod wc;

use rusqlite::Connection;

pub use draft::{
    DraftOrderKind, DraftParticipant, DraftSession, DraftSessionStatus,
};
pub use guild_config::GuildConfig;
pub use league::{competition_code as league_competition_code, exists as league_exists};
pub use registration::Registration;
pub use season::{RosterPhase, Season, SeasonDisplay, SeasonMeta};
pub use migrate::SCHEMA_VERSION;
pub use epl::{
    EplMatchResult, EplPlayerGoalTotal, EplProcessedMatch, EplTiebreakerPick,
};
pub use wc::{
    WcAnnouncedElimination, WcMatchResult, WcPlayerGoalTotal, WcProcessedMatch, WcTiebreakerPick,
};

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("PRAGMA foreign_keys = ON", [])?;
    migrate::run(conn)
}
