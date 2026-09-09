use rusqlite::Connection;

use league_bot::db::{self, GuildConfig, Season, SCHEMA_VERSION};

#[test]
fn fresh_init_seeds_catalog_without_seasons() {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();

    let version: i64 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    assert_eq!(SCHEMA_VERSION, 2);

    let leagues: i64 = conn
        .query_row("SELECT COUNT(*) FROM leagues", [], |row| row.get(0))
        .unwrap();
    assert_eq!(leagues, 4);

    let seasons: i64 = conn
        .query_row("SELECT COUNT(*) FROM seasons", [], |row| row.get(0))
        .unwrap();
    assert_eq!(seasons, 0);

    let guild_configs: i64 = conn
        .query_row("SELECT COUNT(*) FROM guild_config", [], |row| row.get(0))
        .unwrap();
    assert_eq!(guild_configs, 0);

    let league_tables: i64 = conn
        .query_row(
            "
            SELECT COUNT(*)
            FROM sqlite_master
            WHERE type = 'table'
              AND name IN (
                'nba_match_results',
                'nba_processed_games',
                'nba_tiebreaker_picks',
                'nba_player_points_totals',
                'nfl_match_results',
                'nfl_processed_games',
                'nfl_tiebreaker_picks',
                'nfl_player_touchdown_totals',
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
    assert_eq!(league_tables, 15);
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

    GuildConfig::set_default_season_id(&conn, guild_a, season_a.id).unwrap();
    GuildConfig::set_default_season_id(&conn, guild_b, season_b.id).unwrap();

    assert_eq!(
        Season::default_for_guild(&conn, guild_a).unwrap().id,
        season_a.id
    );
    assert_eq!(
        Season::default_for_guild(&conn, guild_b).unwrap().id,
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

    // Command focus on the idle season must not change the live poller set.
    GuildConfig::set_default_season_id(&conn, 222, idle.id).unwrap();
    let live_ids_after_focus: Vec<i64> = Season::list_live_with_meta(&conn)
        .unwrap()
        .into_iter()
        .map(|meta| meta.season.id)
        .collect();
    assert_eq!(live_ids_after_focus, vec![live.id]);
}
