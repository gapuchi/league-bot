use rusqlite::{Connection, OptionalExtension, params};

use super::{guild_config::GuildConfig, league};

/// Registration / draft lifecycle for a season.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RosterPhase {
    Open,
    Drafting,
    Frozen,
}

impl RosterPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Drafting => "drafting",
            Self::Frozen => "frozen",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "open" => Some(Self::Open),
            "drafting" => Some(Self::Drafting),
            "frozen" => Some(Self::Frozen),
            _ => None,
        }
    }
}

pub struct Season {
    pub id: i64,
    pub guild_id: u64,
    pub league_id: i64,
    pub slug: String,
    pub name: String,
    pub announce_channel_id: Option<u64>,
    /// When true, the background poller includes this season. Independent of
    /// guild command focus (`GuildConfig.default_season_id`).
    pub polling_enabled: bool,
    pub roster_phase: RosterPhase,
}

pub struct SeasonMeta {
    pub season: Season,
    pub league_slug: String,
    pub league_name: String,
}

pub struct SeasonLeague {
    pub season: Season,
    pub league_slug: String,
    pub league_name: String,
}

pub struct SeasonDisplay {
    pub league_name: String,
    pub name: String,
    pub slug: String,
}

impl Season {
    pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<Self>> {
        conn.query_row(
            "
            SELECT id, guild_id, league_id, slug, name, announce_channel_id, polling_enabled,
                   roster_phase
            FROM seasons
            WHERE id = ?1
            ",
            params![id],
            row_from,
        )
        .optional()
    }

    pub fn default_for_guild(conn: &Connection, guild_id: u64) -> rusqlite::Result<Self> {
        let config =
            GuildConfig::get(conn, guild_id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        let season = Self::get(conn, config.default_season_id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        if season.guild_id != guild_id {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        Ok(season)
    }

    pub fn list_all_with_meta(conn: &Connection) -> rusqlite::Result<Vec<SeasonMeta>> {
        let mut stmt = conn.prepare(
            "
            SELECT
                s.id,
                s.guild_id,
                s.league_id,
                s.slug,
                s.name,
                s.announce_channel_id,
                s.polling_enabled,
                s.roster_phase,
                l.slug,
                l.name
            FROM seasons s
            JOIN leagues l ON l.id = s.league_id
            ORDER BY s.id
            ",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(SeasonMeta {
                season: season_from_row(row)?,
                league_slug: row.get(8)?,
                league_name: row.get(9)?,
            })
        })?;
        rows.collect()
    }

    /// Seasons the background poller should process (independent of command focus).
    pub fn list_live_with_meta(conn: &Connection) -> rusqlite::Result<Vec<SeasonMeta>> {
        let mut stmt = conn.prepare(
            "
            SELECT
                s.id,
                s.guild_id,
                s.league_id,
                s.slug,
                s.name,
                s.announce_channel_id,
                s.polling_enabled,
                s.roster_phase,
                l.slug,
                l.name
            FROM seasons s
            JOIN leagues l ON l.id = s.league_id
            WHERE s.polling_enabled = 1
            ORDER BY s.id
            ",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(SeasonMeta {
                season: season_from_row(row)?,
                league_slug: row.get(8)?,
                league_name: row.get(9)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_for_guild_league(
        conn: &Connection,
        guild_id: u64,
        league_slug: &str,
    ) -> rusqlite::Result<Option<Self>> {
        conn.query_row(
            "
            SELECT s.id, s.guild_id, s.league_id, s.slug, s.name, s.announce_channel_id,
                   s.polling_enabled, s.roster_phase
            FROM seasons s
            JOIN leagues l ON l.id = s.league_id
            WHERE s.guild_id = ?1 AND l.slug = ?2
            ORDER BY s.id DESC
            LIMIT 1
            ",
            params![guild_id as i64, league_slug],
            row_from,
        )
        .optional()
    }

    pub fn get_or_create(
        conn: &Connection,
        guild_id: u64,
        league_slug: &str,
        slug: &str,
        name: &str,
    ) -> rusqlite::Result<Self> {
        if let Some(season) = Self::get_by_guild_league_slug(conn, guild_id, league_slug, slug)? {
            return Ok(season);
        }

        let league_id = league::id_for_slug(conn, league_slug)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;

        conn.execute(
            "
            INSERT INTO seasons (guild_id, league_id, slug, name)
            VALUES (?1, ?2, ?3, ?4)
            ",
            params![guild_id as i64, league_id, slug, name],
        )?;
        Self::get(conn, conn.last_insert_rowid())?.ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn set_announce_channel(
        conn: &Connection,
        id: i64,
        channel_id: u64,
    ) -> rusqlite::Result<()> {
        conn.execute(
            "UPDATE seasons SET announce_channel_id = ?1 WHERE id = ?2",
            params![channel_id as i64, id],
        )?;
        Ok(())
    }

    pub fn set_polling_enabled(
        conn: &Connection,
        id: i64,
        enabled: bool,
    ) -> rusqlite::Result<()> {
        conn.execute(
            "UPDATE seasons SET polling_enabled = ?1 WHERE id = ?2",
            params![i64::from(enabled), id],
        )?;
        Ok(())
    }

    pub fn set_roster_phase(
        conn: &Connection,
        id: i64,
        phase: RosterPhase,
    ) -> rusqlite::Result<()> {
        conn.execute(
            "UPDATE seasons SET roster_phase = ?1 WHERE id = ?2",
            params![phase.as_str(), id],
        )?;
        Ok(())
    }

    pub fn list_with_league(
        conn: &Connection,
        guild_id: u64,
    ) -> rusqlite::Result<Vec<SeasonLeague>> {
        let mut stmt = conn.prepare(
            "
            SELECT s.id, s.guild_id, s.league_id, s.slug, s.name, s.announce_channel_id,
                   s.polling_enabled, s.roster_phase, l.slug, l.name
            FROM seasons s
            JOIN leagues l ON l.id = s.league_id
            WHERE s.guild_id = ?1
            ORDER BY l.id
            ",
        )?;
        let rows = stmt.query_map(params![guild_id as i64], |row| {
            Ok(SeasonLeague {
                season: season_from_row(row)?,
                league_slug: row.get(8)?,
                league_name: row.get(9)?,
            })
        })?;
        rows.collect()
    }

    pub fn league_id_for(conn: &Connection, season_id: i64) -> rusqlite::Result<i64> {
        conn.query_row(
            "SELECT league_id FROM seasons WHERE id = ?1",
            params![season_id],
            |row| row.get(0),
        )
    }

    pub fn league_slug_for(conn: &Connection, season_id: i64) -> rusqlite::Result<String> {
        conn.query_row(
            "
            SELECT l.slug
            FROM seasons s
            JOIN leagues l ON l.id = s.league_id
            WHERE s.id = ?1
            ",
            params![season_id],
            |row| row.get(0),
        )
    }

    pub fn get_by_guild_league_slug(
        conn: &Connection,
        guild_id: u64,
        league_slug: &str,
        slug: &str,
    ) -> rusqlite::Result<Option<Self>> {
        conn.query_row(
            "
            SELECT s.id, s.guild_id, s.league_id, s.slug, s.name, s.announce_channel_id,
                   s.polling_enabled, s.roster_phase
            FROM seasons s
            JOIN leagues l ON l.id = s.league_id
            WHERE s.guild_id = ?1 AND l.slug = ?2 AND s.slug = ?3
            ",
            params![guild_id as i64, league_slug, slug],
            row_from,
        )
        .optional()
    }
}

impl SeasonDisplay {
    pub fn for_season(conn: &Connection, season_id: i64) -> rusqlite::Result<Self> {
        conn.query_row(
            "
            SELECT l.name, s.name, s.slug
            FROM seasons s
            JOIN leagues l ON l.id = s.league_id
            WHERE s.id = ?1
            ",
            params![season_id],
            |row| {
                Ok(Self {
                    league_name: row.get(0)?,
                    name: row.get(1)?,
                    slug: row.get(2)?,
                })
            },
        )
    }
}

/// Reads season columns in order:
/// `id, guild_id, league_id, slug, name, announce_channel_id, polling_enabled, roster_phase`.
fn season_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Season> {
    let channel: Option<i64> = row.get(5)?;
    let polling: i64 = row.get(6)?;
    let phase_raw: String = row.get(7)?;
    let roster_phase = RosterPhase::parse(&phase_raw).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            7,
            rusqlite::types::Type::Text,
            format!("unknown roster_phase `{phase_raw}`").into(),
        )
    })?;
    Ok(Season {
        id: row.get(0)?,
        guild_id: row.get::<_, i64>(1)? as u64,
        league_id: row.get(2)?,
        slug: row.get(3)?,
        name: row.get(4)?,
        announce_channel_id: channel.map(|id| id as u64),
        polling_enabled: polling != 0,
        roster_phase,
    })
}

fn row_from(row: &rusqlite::Row<'_>) -> rusqlite::Result<Season> {
    season_from_row(row)
}
