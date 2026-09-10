use rusqlite::{Connection, OptionalExtension};

pub const SCHEMA_VERSION: i64 = 2;
pub const WC_LEAGUE_SLUG: &str = "wc";
pub const NBA_LEAGUE_SLUG: &str = "nba";
pub const NFL_LEAGUE_SLUG: &str = "nfl";
pub const EPL_LEAGUE_SLUG: &str = "epl";

const CREATE_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS leagues (
    id                      INTEGER PRIMARY KEY,
    slug                    TEXT NOT NULL UNIQUE,
    name                    TEXT NOT NULL,
    sport                   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS seasons (
    id                      INTEGER PRIMARY KEY,
    guild_id                INTEGER NOT NULL,
    league_id               INTEGER NOT NULL REFERENCES leagues(id),
    slug                    TEXT NOT NULL,
    name                    TEXT NOT NULL,
    announce_channel_id     INTEGER,
    polling_enabled         INTEGER NOT NULL DEFAULT 1,
    roster_phase            TEXT NOT NULL DEFAULT 'open',
    starts_at               TEXT,
    ends_at                 TEXT,
    UNIQUE (guild_id, league_id, slug)
);

CREATE TABLE IF NOT EXISTS draft_sessions (
    season_id               INTEGER PRIMARY KEY REFERENCES seasons(id),
    order_kind              TEXT NOT NULL,
    status                  TEXT NOT NULL,
    created_at              TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS draft_participants (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    position                INTEGER NOT NULL,
    user_id                 INTEGER NOT NULL,
    PRIMARY KEY (season_id, position),
    UNIQUE (season_id, user_id)
);

CREATE TABLE IF NOT EXISTS guild_config (
    guild_id                INTEGER PRIMARY KEY,
    default_season_id       INTEGER NOT NULL REFERENCES seasons(id)
);

CREATE TABLE IF NOT EXISTS teams (
    league_id               INTEGER NOT NULL,
    team_id                 INTEGER NOT NULL,
    name                    TEXT NOT NULL,
    short_name              TEXT,
    code                    TEXT,
    PRIMARY KEY (league_id, team_id),
    FOREIGN KEY (league_id) REFERENCES leagues(id)
);

CREATE TABLE IF NOT EXISTS registrations (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    user_id                 INTEGER NOT NULL,
    team_id                 INTEGER NOT NULL,
    team_name               TEXT NOT NULL,
    PRIMARY KEY (season_id, team_id)
);

CREATE INDEX IF NOT EXISTS idx_registrations_season_user
    ON registrations (season_id, user_id);

CREATE TABLE IF NOT EXISTS wc_match_results (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    match_id                INTEGER NOT NULL,
    home_team_id            INTEGER NOT NULL,
    away_team_id            INTEGER NOT NULL,
    home_goals              INTEGER NOT NULL,
    away_goals              INTEGER NOT NULL,
    stage                   TEXT,
    finished_at             TEXT,
    PRIMARY KEY (season_id, match_id)
);

CREATE TABLE IF NOT EXISTS wc_processed_matches (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    match_id                INTEGER NOT NULL,
    PRIMARY KEY (season_id, match_id)
);

CREATE TABLE IF NOT EXISTS wc_announced_eliminations (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    team_id                 INTEGER NOT NULL,
    PRIMARY KEY (season_id, team_id)
);

CREATE TABLE IF NOT EXISTS wc_tiebreaker_picks (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    user_id                 INTEGER NOT NULL,
    player_id               INTEGER NOT NULL,
    player_name             TEXT NOT NULL,
    team_id                 INTEGER NOT NULL,
    team_name               TEXT NOT NULL,
    PRIMARY KEY (season_id, user_id)
);

CREATE TABLE IF NOT EXISTS wc_player_goal_totals (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    player_id               INTEGER NOT NULL,
    goals                   INTEGER NOT NULL,
    updated_at              TEXT NOT NULL,
    PRIMARY KEY (season_id, player_id)
);

CREATE TABLE IF NOT EXISTS nba_match_results (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    game_id                 INTEGER NOT NULL,
    home_team_id            INTEGER NOT NULL,
    away_team_id            INTEGER NOT NULL,
    home_points             INTEGER NOT NULL,
    away_points             INTEGER NOT NULL,
    finished_at             TEXT,
    PRIMARY KEY (season_id, game_id)
);

CREATE TABLE IF NOT EXISTS nba_processed_games (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    game_id                 INTEGER NOT NULL,
    PRIMARY KEY (season_id, game_id)
);

CREATE TABLE IF NOT EXISTS nba_tiebreaker_picks (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    user_id                 INTEGER NOT NULL,
    player_id               INTEGER NOT NULL,
    player_name             TEXT NOT NULL,
    team_id                 INTEGER NOT NULL,
    team_name               TEXT NOT NULL,
    PRIMARY KEY (season_id, user_id)
);

CREATE TABLE IF NOT EXISTS nba_player_points_totals (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    player_id               INTEGER NOT NULL,
    points                  INTEGER NOT NULL,
    updated_at              TEXT NOT NULL,
    PRIMARY KEY (season_id, player_id)
);

CREATE TABLE IF NOT EXISTS nfl_match_results (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    game_id                 INTEGER NOT NULL,
    home_team_id            INTEGER NOT NULL,
    away_team_id            INTEGER NOT NULL,
    home_score              INTEGER NOT NULL,
    away_score              INTEGER NOT NULL,
    postseason              INTEGER NOT NULL DEFAULT 0,
    finished_at             TEXT,
    PRIMARY KEY (season_id, game_id)
);

CREATE TABLE IF NOT EXISTS nfl_processed_games (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    game_id                 INTEGER NOT NULL,
    PRIMARY KEY (season_id, game_id)
);

CREATE TABLE IF NOT EXISTS nfl_tiebreaker_picks (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    user_id                 INTEGER NOT NULL,
    player_id               INTEGER NOT NULL,
    player_name             TEXT NOT NULL,
    team_id                 INTEGER NOT NULL,
    team_name               TEXT NOT NULL,
    PRIMARY KEY (season_id, user_id)
);

CREATE TABLE IF NOT EXISTS nfl_player_touchdown_totals (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    player_id               INTEGER NOT NULL,
    touchdowns              INTEGER NOT NULL,
    updated_at              TEXT NOT NULL,
    PRIMARY KEY (season_id, player_id)
);

CREATE TABLE IF NOT EXISTS epl_match_results (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    match_id                INTEGER NOT NULL,
    home_team_id            INTEGER NOT NULL,
    away_team_id            INTEGER NOT NULL,
    home_goals              INTEGER NOT NULL,
    away_goals              INTEGER NOT NULL,
    matchday                INTEGER,
    finished_at             TEXT,
    PRIMARY KEY (season_id, match_id)
);

CREATE TABLE IF NOT EXISTS epl_processed_matches (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    match_id                INTEGER NOT NULL,
    PRIMARY KEY (season_id, match_id)
);

CREATE TABLE IF NOT EXISTS epl_tiebreaker_picks (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    user_id                 INTEGER NOT NULL,
    player_id               INTEGER NOT NULL,
    player_name             TEXT NOT NULL,
    team_id                 INTEGER NOT NULL,
    team_name               TEXT NOT NULL,
    PRIMARY KEY (season_id, user_id)
);

CREATE TABLE IF NOT EXISTS epl_player_goal_totals (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    player_id               INTEGER NOT NULL,
    goals                   INTEGER NOT NULL,
    updated_at              TEXT NOT NULL,
    PRIMARY KEY (season_id, player_id)
);
";

/// Initialize the schema on a fresh database, or bring an existing one up to
/// `SCHEMA_VERSION`. `CREATE_SCHEMA` (all `IF NOT EXISTS`) seeds fresh databases and
/// adds whole missing tables; `migrate` handles in-place column additions to tables that
/// already existed, which `CREATE TABLE IF NOT EXISTS` cannot do.
pub fn run(conn: &Connection) -> rusqlite::Result<()> {
    let existing_version = existing_version(conn)?;
    conn.execute_batch(CREATE_SCHEMA)?;
    seed_catalog(conn)?;
    if let Some(from_version) = existing_version {
        migrate(conn, from_version)?;
    }
    set_version(conn, SCHEMA_VERSION)?;
    Ok(())
}

/// Version recorded before this run, or `None` when the database is fresh (no
/// `schema_version` table yet), in which case `CREATE_SCHEMA` already builds the
/// current layout and no migration is needed.
fn existing_version(conn: &Connection) -> rusqlite::Result<Option<i64>> {
    let has_schema_version = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_version'",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !has_schema_version {
        return Ok(None);
    }
    let version = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
            row.get(0)
        })
        .optional()?
        .unwrap_or(1);
    Ok(Some(version))
}

fn migrate(conn: &Connection, from_version: i64) -> rusqlite::Result<()> {
    if from_version < 2 {
        add_column_if_missing(
            conn,
            "nfl_match_results",
            "postseason",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
    }
    Ok(())
}

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> rusqlite::Result<()> {
    if !column_exists(conn, table, column)? {
        conn.execute_batch(&format!(
            "ALTER TABLE {table} ADD COLUMN {column} {definition}"
        ))?;
    }
    Ok(())
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> rusqlite::Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn set_version(conn: &Connection, version: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM schema_version", [])?;
    conn.execute("INSERT INTO schema_version (version) VALUES (?1)", [version])?;
    Ok(())
}

fn seed_catalog(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "
        INSERT INTO leagues (id, slug, name, sport)
        VALUES (1, ?1, 'FIFA World Cup', 'soccer')
        ON CONFLICT(id) DO NOTHING
        ",
        [WC_LEAGUE_SLUG],
    )?;
    conn.execute(
        "
        INSERT INTO leagues (id, slug, name, sport)
        VALUES (2, ?1, 'NBA', 'basketball')
        ON CONFLICT(id) DO NOTHING
        ",
        [NBA_LEAGUE_SLUG],
    )?;
    conn.execute(
        "
        INSERT INTO leagues (id, slug, name, sport)
        VALUES (3, ?1, 'NFL', 'football')
        ON CONFLICT(id) DO NOTHING
        ",
        [NFL_LEAGUE_SLUG],
    )?;
    conn.execute(
        "
        INSERT INTO leagues (id, slug, name, sport)
        VALUES (4, ?1, 'Premier League', 'soccer')
        ON CONFLICT(id) DO NOTHING
        ",
        [EPL_LEAGUE_SLUG],
    )?;
    Ok(())
}
