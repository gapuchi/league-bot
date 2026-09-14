use rusqlite::{Connection, OptionalExtension};

pub const SCHEMA_VERSION: i64 = 4;
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

CREATE TABLE IF NOT EXISTS nfl_match_results (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    game_id                 INTEGER NOT NULL,
    home_team_id            INTEGER NOT NULL,
    away_team_id            INTEGER NOT NULL,
    home_score              INTEGER NOT NULL,
    away_score              INTEGER NOT NULL,
    postseason              INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (season_id, game_id)
);

CREATE TABLE IF NOT EXISTS nfl_processed_games (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    game_id                 INTEGER NOT NULL,
    PRIMARY KEY (season_id, game_id)
);

CREATE TABLE IF NOT EXISTS epl_match_results (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    match_id                INTEGER NOT NULL,
    home_team_id            INTEGER NOT NULL,
    away_team_id            INTEGER NOT NULL,
    home_goals              INTEGER NOT NULL,
    away_goals              INTEGER NOT NULL,
    matchday                INTEGER,
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

/// Create any missing tables, then bring an existing database forward one version at a
/// time. `CREATE TABLE IF NOT EXISTS` never alters existing tables, so every change to
/// `CREATE_SCHEMA` that affects existing rows needs a matching `upgrade` step.
pub fn run(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(CREATE_SCHEMA)?;
    seed_catalog(conn)?;

    let version: Option<i64> = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| row.get(0))
        .optional()?;
    if let Some(from) = version {
        upgrade(conn, from)?;
    }

    set_version(conn, SCHEMA_VERSION)?;
    Ok(())
}

fn upgrade(conn: &Connection, from: i64) -> rusqlite::Result<()> {
    if from < 2 {
        conn.execute_batch(
            "ALTER TABLE nfl_match_results ADD COLUMN postseason INTEGER NOT NULL DEFAULT 0",
        )?;
    }
    if from < 3 {
        conn.execute_batch(
            "
            DROP TABLE IF EXISTS nba_match_results;
            DROP TABLE IF EXISTS nba_processed_games;
            DROP TABLE IF EXISTS nba_tiebreaker_picks;
            DROP TABLE IF EXISTS nba_player_points_totals;
            DROP TABLE IF EXISTS nfl_tiebreaker_picks;
            DROP TABLE IF EXISTS nfl_player_touchdown_totals;
            DROP TABLE IF EXISTS teams;
            ",
        )?;
        for table in ["wc_match_results", "epl_match_results", "nfl_match_results"] {
            drop_column_if_exists(conn, table, "finished_at")?;
        }
        drop_column_if_exists(conn, "seasons", "starts_at")?;
        drop_column_if_exists(conn, "seasons", "ends_at")?;
    }
    if from < 4 {
        // Commands now resolve the season from what is live, so a guild may have at
        // most one live season per league; keep the newest and end the rest.
        conn.execute_batch(
            "
            DROP TABLE IF EXISTS guild_config;
            UPDATE seasons
            SET polling_enabled = 0
            WHERE polling_enabled = 1
              AND id NOT IN (
                  SELECT MAX(id) FROM seasons WHERE polling_enabled = 1
                  GROUP BY guild_id, league_id
              );
            ",
        )?;
    }
    Ok(())
}

fn drop_column_if_exists(conn: &Connection, table: &str, column: &str) -> rusqlite::Result<()> {
    let present: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = ?1"),
        [column],
        |row| row.get(0),
    )?;
    if present > 0 {
        conn.execute_batch(&format!("ALTER TABLE {table} DROP COLUMN {column}"))?;
    }
    Ok(())
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
