use rusqlite::Connection;

use league_bot::db::{self, Season, SCHEMA_VERSION};

#[test]
fn fresh_init_seeds_catalog_without_seasons() {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();

    let version: i64 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    assert_eq!(SCHEMA_VERSION, 5);

    let leagues: i64 = conn
        .query_row("SELECT COUNT(*) FROM leagues", [], |row| row.get(0))
        .unwrap();
    assert_eq!(leagues, 4);

    let seasons: i64 = conn
        .query_row("SELECT COUNT(*) FROM seasons", [], |row| row.get(0))
        .unwrap();
    assert_eq!(seasons, 0);

    let league_tables: i64 = conn
        .query_row(
            "
            SELECT COUNT(*)
            FROM sqlite_master
            WHERE type = 'table'
              AND name IN (
                'nfl_match_results',
                'nfl_processed_games',
                'nba_match_results',
                'nba_processed_games',
                'epl_match_results',
                'epl_processed_matches',
                'epl_tiebreaker_picks',
                'epl_player_goal_totals',
                'draft_sessions',
                'draft_participants',
                'wc_announced_eliminations'
              )
            ",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(league_tables, 11);

    let dropped_tables: i64 = conn
        .query_row(
            "
            SELECT COUNT(*)
            FROM sqlite_master
            WHERE type = 'table'
              AND name IN ('teams', 'guild_config', 'nfl_tiebreaker_picks', 'nba_tiebreaker_picks', 'nba_player_points_totals')
            ",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(dropped_tables, 0);
}

#[test]
fn upgrade_adds_postseason_column_to_v1_nfl_results() {
    let conn = Connection::open_in_memory().unwrap();

    // Recreate the v1 layout of nfl_match_results (no postseason column) and
    // stamp the database at schema version 1, as an existing file would be.
    conn.execute_batch(
        "
        CREATE TABLE schema_version (version INTEGER NOT NULL);
        INSERT INTO schema_version (version) VALUES (1);
        CREATE TABLE nfl_match_results (
            season_id    INTEGER NOT NULL,
            game_id      INTEGER NOT NULL,
            home_team_id INTEGER NOT NULL,
            away_team_id INTEGER NOT NULL,
            home_score   INTEGER NOT NULL,
            away_score   INTEGER NOT NULL,
            finished_at  TEXT,
            PRIMARY KEY (season_id, game_id)
        );
        ",
    )
    .unwrap();

    db::init(&conn).unwrap();

    let version: i64 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);

    let postseason_default: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('nfl_match_results') WHERE name = 'postseason'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(postseason_default, 1);
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
    conn.query_row(
        &format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = ?1"),
        [column],
        |row| row.get::<_, i64>(0),
    )
    .unwrap()
        > 0
}

fn table_exists(conn: &Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table],
        |row| row.get::<_, i64>(0),
    )
    .unwrap()
        > 0
}

#[test]
fn upgrade_from_v2_drops_dead_tables_and_columns_but_keeps_data() {
    let conn = Connection::open_in_memory().unwrap();

    // Minimal v2 layout: live tables with the since-removed columns, plus the dead tables.
    conn.execute_batch(
        "
        CREATE TABLE schema_version (version INTEGER NOT NULL);
        INSERT INTO schema_version (version) VALUES (2);
        CREATE TABLE leagues (id INTEGER PRIMARY KEY, slug TEXT NOT NULL UNIQUE, name TEXT NOT NULL, sport TEXT NOT NULL);
        INSERT INTO leagues (id, slug, name, sport) VALUES (1, 'wc', 'FIFA World Cup', 'soccer');
        CREATE TABLE seasons (
            id INTEGER PRIMARY KEY, guild_id INTEGER NOT NULL, league_id INTEGER NOT NULL REFERENCES leagues(id),
            slug TEXT NOT NULL, name TEXT NOT NULL, announce_channel_id INTEGER,
            polling_enabled INTEGER NOT NULL DEFAULT 1, roster_phase TEXT NOT NULL DEFAULT 'open',
            starts_at TEXT, ends_at TEXT, UNIQUE (guild_id, league_id, slug)
        );
        INSERT INTO seasons (id, guild_id, league_id, slug, name) VALUES (7, 111, 1, 'wc-2026', 'World Cup 2026');
        CREATE TABLE registrations (
            season_id INTEGER NOT NULL, user_id INTEGER NOT NULL, team_id INTEGER NOT NULL, team_name TEXT NOT NULL,
            PRIMARY KEY (season_id, team_id)
        );
        INSERT INTO registrations VALUES (7, 100, 769, 'Mexico');
        CREATE TABLE wc_match_results (
            season_id INTEGER NOT NULL, match_id INTEGER NOT NULL, home_team_id INTEGER NOT NULL,
            away_team_id INTEGER NOT NULL, home_goals INTEGER NOT NULL, away_goals INTEGER NOT NULL,
            stage TEXT, finished_at TEXT, PRIMARY KEY (season_id, match_id)
        );
        INSERT INTO wc_match_results VALUES (7, 1, 769, 774, 2, 1, 'GROUP_STAGE', NULL);
        CREATE TABLE teams (league_id INTEGER NOT NULL, team_id INTEGER NOT NULL, name TEXT NOT NULL, PRIMARY KEY (league_id, team_id));
        CREATE TABLE nba_match_results (season_id INTEGER NOT NULL, game_id INTEGER NOT NULL, PRIMARY KEY (season_id, game_id));
        CREATE TABLE nfl_tiebreaker_picks (season_id INTEGER NOT NULL, user_id INTEGER NOT NULL, PRIMARY KEY (season_id, user_id));
        ",
    )
    .unwrap();

    db::init(&conn).unwrap();

    let version: i64 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);

    assert!(!table_exists(&conn, "teams"));
    assert!(!table_exists(&conn, "nfl_tiebreaker_picks"));
    // The v2 catalog-only nba stub is dropped, then recreated as a real result table
    // with the full column set once NBA becomes a compiled-in league.
    assert!(table_exists(&conn, "nba_match_results"));
    assert!(column_exists(&conn, "nba_match_results", "home_score"));
    assert!(!column_exists(&conn, "wc_match_results", "finished_at"));
    assert!(!column_exists(&conn, "seasons", "starts_at"));
    assert!(!column_exists(&conn, "seasons", "ends_at"));

    let season = Season::get(&conn, 7).unwrap().unwrap();
    assert_eq!(season.name, "World Cup 2026");
    assert_eq!(
        league_bot::db::WcMatchResult::score(&conn, 7, 1).unwrap(),
        Some((2, 1))
    );
    assert_eq!(
        league_bot::db::Registration::list_for_season(&conn, 7)
            .unwrap()
            .len(),
        1
    );

    // Running again on the now-current database is a no-op.
    db::init(&conn).unwrap();
}

#[test]
fn upgrade_from_v3_drops_guild_config_and_keeps_one_live_season_per_league() {
    let conn = Connection::open_in_memory().unwrap();

    conn.execute_batch(
        "
        CREATE TABLE schema_version (version INTEGER NOT NULL);
        INSERT INTO schema_version (version) VALUES (3);
        CREATE TABLE leagues (id INTEGER PRIMARY KEY, slug TEXT NOT NULL UNIQUE, name TEXT NOT NULL, sport TEXT NOT NULL);
        INSERT INTO leagues (id, slug, name, sport) VALUES (1, 'wc', 'FIFA World Cup', 'soccer'), (3, 'nfl', 'NFL', 'football');
        CREATE TABLE seasons (
            id INTEGER PRIMARY KEY, guild_id INTEGER NOT NULL, league_id INTEGER NOT NULL REFERENCES leagues(id),
            slug TEXT NOT NULL, name TEXT NOT NULL, announce_channel_id INTEGER,
            polling_enabled INTEGER NOT NULL DEFAULT 1, roster_phase TEXT NOT NULL DEFAULT 'open',
            UNIQUE (guild_id, league_id, slug)
        );
        INSERT INTO seasons (id, guild_id, league_id, slug, name) VALUES
            (1, 111, 3, 'nfl-2025', 'NFL 2025'),
            (2, 111, 3, 'nfl-2026', 'NFL 2026'),
            (3, 111, 1, 'wc-2026', 'World Cup 2026'),
            (4, 222, 3, 'nfl-2025', 'NFL 2025');
        CREATE TABLE guild_config (guild_id INTEGER PRIMARY KEY, default_season_id INTEGER NOT NULL REFERENCES seasons(id));
        INSERT INTO guild_config VALUES (111, 1);
        ",
    )
    .unwrap();

    db::init(&conn).unwrap();

    assert!(!table_exists(&conn, "guild_config"));

    let live: Vec<i64> = Season::list_live_with_meta(&conn)
        .unwrap()
        .into_iter()
        .map(|meta| meta.season.id)
        .collect();
    // Guild 111 keeps its newest NFL season and its WC season; guild 222 is untouched.
    assert_eq!(live, vec![2, 3, 4]);
    assert_eq!(Season::list_all_with_meta(&conn).unwrap().len(), 4);
    assert!(!Season::get(&conn, 1).unwrap().unwrap().polling_enabled);
}

#[test]
fn init_is_idempotent() {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();
    db::init(&conn).unwrap();

    let seasons: i64 = conn
        .query_row("SELECT COUNT(*) FROM seasons", [], |row| row.get(0))
        .unwrap();
    assert_eq!(seasons, 0);
}

#[test]
fn get_or_create_scopes_by_guild() {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();

    let guild_a = 111_u64;
    let guild_b = 222_u64;

    let season_a =
        Season::get_or_create(&conn, guild_a, "wc", "wc-2026", "World Cup 2026").unwrap();
    let season_b =
        Season::get_or_create(&conn, guild_b, "wc", "wc-2026", "World Cup 2026").unwrap();

    assert_ne!(season_a.id, season_b.id);
    assert_eq!(season_a.guild_id, guild_a);
    assert_eq!(season_b.guild_id, guild_b);

    assert_eq!(
        Season::latest_for_guild_league(&conn, guild_a, "wc")
            .unwrap()
            .unwrap()
            .season
            .id,
        season_a.id
    );
    assert_eq!(
        Season::latest_for_guild_league(&conn, guild_b, "wc")
            .unwrap()
            .unwrap()
            .season
            .id,
        season_b.id
    );
}

#[test]
fn announced_elimination_tracks_per_season_team() {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();

    let season =
        Season::get_or_create(&conn, 111, "wc", "wc-2026", "World Cup 2026").unwrap();

    use league_bot::db::WcAnnouncedElimination;

    let announced = WcAnnouncedElimination::list_for_season(&conn, season.id).unwrap();
    assert!(announced.is_empty());

    WcAnnouncedElimination::mark(&conn, season.id, 769).unwrap();
    WcAnnouncedElimination::mark(&conn, season.id, 769).unwrap();

    let announced = WcAnnouncedElimination::list_for_season(&conn, season.id).unwrap();
    assert_eq!(announced.len(), 1);
    assert!(announced.contains(&769));
}

#[test]
fn new_seasons_are_live_by_default_and_list_live_filters() {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();

    let live = Season::get_or_create(&conn, 111, "wc", "wc-2026", "World Cup 2026").unwrap();
    assert!(live.polling_enabled);
    assert_eq!(live.roster_phase, league_bot::db::RosterPhase::Open);

    let idle = Season::get_or_create(&conn, 222, "wc", "wc-2026", "World Cup 2026").unwrap();
    Season::set_polling_enabled(&conn, idle.id, false).unwrap();

    let live_ids: Vec<i64> = Season::list_live_with_meta(&conn)
        .unwrap()
        .into_iter()
        .map(|meta| meta.season.id)
        .collect();
    assert_eq!(live_ids, vec![live.id]);

    let all_ids: Vec<i64> = Season::list_all_with_meta(&conn)
        .unwrap()
        .into_iter()
        .map(|meta| meta.season.id)
        .collect();
    assert_eq!(all_ids.len(), 2);
    assert!(all_ids.contains(&live.id));
    assert!(all_ids.contains(&idle.id));

    assert!(Season::list_live_for_guild(&conn, 222).unwrap().is_empty());
    // An ended season is still reachable by league so its standings remain viewable.
    assert_eq!(
        Season::latest_for_guild_league(&conn, 222, "wc")
            .unwrap()
            .unwrap()
            .season
            .id,
        idle.id
    );
}
