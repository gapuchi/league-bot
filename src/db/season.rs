use rusqlite::{Connection, OptionalExtension, params};

use super::league;

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

#[derive(Debug, Clone)]
pub struct Season {
    pub id: i64,
    pub guild_id: u64,
    pub league_id: i64,
    pub slug: String,
    pub name: String,
    pub announce_channel_id: Option<u64>,
    /// When true, the background poller includes this season.
    pub polling_enabled: bool,
    pub roster_phase: RosterPhase,
}

#[derive(Debug, Clone)]
pub struct SeasonMeta {
    pub season: Season,
    pub league_slug: String,
    pub league_name: String,
}

const META_SELECT: &str = "
    SELECT s.id, s.guild_id, s.league_id, s.slug, s.name, s.announce_channel_id,
           s.polling_enabled, s.roster_phase, l.slug, l.name
    FROM seasons s
    JOIN leagues l ON l.id = s.league_id
";

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
            season_from_row,
        )
        .optional()
    }

    pub fn get_meta(conn: &Connection, id: i64) -> rusqlite::Result<Option<SeasonMeta>> {
        conn.query_row(
            &format!("{META_SELECT} WHERE s.id = ?1"),
            params![id],
            meta_from_row,
        )
        .optional()
    }

    pub fn list_all_with_meta(conn: &Connection) -> rusqlite::Result<Vec<SeasonMeta>> {
        let mut stmt = conn.prepare(&format!("{META_SELECT} ORDER BY s.id"))?;
        let rows = stmt.query_map([], meta_from_row)?;
        rows.collect()
    }

    /// Seasons the background poller should process.
    pub fn list_live_with_meta(conn: &Connection) -> rusqlite::Result<Vec<SeasonMeta>> {
        let mut stmt =
            conn.prepare(&format!("{META_SELECT} WHERE s.polling_enabled = 1 ORDER BY s.id"))?;
        let rows = stmt.query_map([], meta_from_row)?;
        rows.collect()
    }

    /// Every season a guild has ever started, newest league first then newest season.
    pub fn list_for_guild(conn: &Connection, guild_id: u64) -> rusqlite::Result<Vec<SeasonMeta>> {
        let mut stmt = conn.prepare(&format!(
            "{META_SELECT} WHERE s.guild_id = ?1 ORDER BY l.id, s.id DESC"
        ))?;
        let rows = stmt.query_map(params![guild_id as i64], meta_from_row)?;
        rows.collect()
    }

    pub fn list_live_for_guild(
        conn: &Connection,
        guild_id: u64,
    ) -> rusqlite::Result<Vec<SeasonMeta>> {
        let mut stmt = conn.prepare(&format!(
            "{META_SELECT} WHERE s.guild_id = ?1 AND s.polling_enabled = 1 ORDER BY l.id"
        ))?;
        let rows = stmt.query_map(params![guild_id as i64], meta_from_row)?;
        rows.collect()
    }

    /// The guild's season for a league: the live one if any, otherwise the newest.
    pub fn latest_for_guild_league(
        conn: &Connection,
        guild_id: u64,
        league_slug: &str,
    ) -> rusqlite::Result<Option<SeasonMeta>> {
        conn.query_row(
            &format!(
                "{META_SELECT}
                 WHERE s.guild_id = ?1 AND l.slug = ?2
                 ORDER BY s.polling_enabled DESC, s.id DESC
                 LIMIT 1"
            ),
            params![guild_id as i64, league_slug],
            meta_from_row,
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
            season_from_row,
        )
        .optional()
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

/// Season columns followed by `l.slug, l.name` (see `META_SELECT`).
fn meta_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SeasonMeta> {
    Ok(SeasonMeta {
        season: season_from_row(row)?,
        league_slug: row.get(8)?,
        league_name: row.get(9)?,
    })
}
