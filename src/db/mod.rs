mod draft;
mod epl;
mod guild_config;
mod league;
mod league_macros;
mod migrate;
mod nfl;
mod registration;
mod season;
mod team;
mod wc;

use rusqlite::Connection;

pub use draft::{
    DraftOrderKind, DraftParticipant, DraftSession, DraftSessionStatus,
};
pub use epl::{
    EplMatchResult, EplPlayerGoalTotal, EplProcessedMatch, EplTiebreakerPick,
};
pub use guild_config::GuildConfig;
pub use league::{competition_code as league_competition_code, exists as league_exists};
pub use migrate::SCHEMA_VERSION;
pub use nfl::{NflMatchResult, NflProcessedGame};
pub use registration::Registration;
pub use season::{RosterPhase, Season, SeasonDisplay, SeasonMeta};
pub use wc::{
    WcAnnouncedElimination, WcMatchResult, WcPlayerGoalTotal, WcProcessedMatch, WcTiebreakerPick,
};

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("PRAGMA foreign_keys = ON", [])?;
    migrate::run(conn)
}
