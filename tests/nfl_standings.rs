use std::sync::Arc;

use rusqlite::Connection;
use tokio::sync::Mutex;

use league_bot::{
    db::{self, GuildConfig, NflMatchResult, Registration, Season},
    league::League,
    standings::{format_standing_detail, standings_ranks},
    types::Data,
};

fn seeded_conn() -> (Connection, Season) {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();

    let season = Season::get_or_create(&conn, 111, "nfl", "2026", "NFL 2026").unwrap();
    Registration::upsert(&conn, season.id, 100, 21, "Philadelphia Eagles").unwrap();
    Registration::upsert(&conn, season.id, 200, 6, "Dallas Cowboys").unwrap();
    Registration::upsert(&conn, season.id, 300, 12, "Kansas City Chiefs").unwrap();

    NflMatchResult {
        season_id: season.id,
        game_id: 401,
        home_team_id: 21,
        away_team_id: 6,
        home_score: 24,
        away_score: 20,
    }
    .upsert(&conn)
    .unwrap();
    // A tie: both sides collect the draw points.
    NflMatchResult {
        season_id: season.id,
        game_id: 402,
        home_team_id: 12,
        away_team_id: 6,
        home_score: 17,
        away_score: 17,
    }
    .upsert(&conn)
    .unwrap();

    (conn, season)
}

#[test]
fn nfl_standings_use_game_results_without_a_tiebreaker() {
    let (conn, season) = seeded_conn();

    let rows = League::Nfl.standings(&conn, season.id).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!((rows[0].user_id, rows[0].points), (100, 1.0));
    assert_eq!((rows[1].user_id, rows[1].points), (200, 0.5));
    assert_eq!((rows[2].user_id, rows[2].points), (300, 0.5));
    assert!(
        rows.iter()
            .all(|row| row.tiebreaker_value == 0 && row.tiebreaker_player.is_none())
    );

    // Level on points → shared rank, and no tie-breaker line in the breakdown.
    assert_eq!(standings_ranks(&rows), vec![1, 2, 2]);
    let detail = format_standing_detail(2, &rows[1], League::Nfl.tiebreaker_unit());
    assert!(!detail.contains("Tie-breaker"), "{detail}");

    assert_eq!(
        NflMatchResult::score(&conn, season.id, 401).unwrap(),
        Some((24, 20))
    );
    assert_eq!(
        League::Nfl
            .tiebreaker_pick_for_user(&conn, season.id, 200)
            .unwrap(),
        None
    );
    League::Nfl
        .clear_picks_for_team(&conn, season.id, 200, 6)
        .unwrap();
}

#[tokio::test]
async fn nfl_pick_player_is_declined_without_network() {
    let (conn, season) = seeded_conn();
    GuildConfig::set_default_season_id(&conn, 111, season.id).unwrap();
    let data = Data {
        db: Arc::new(Mutex::new(conn)),
        http: reqwest::Client::new(),
    };

    let message = League::Nfl
        .pick_tiebreaker_player(&data, 111, 200, "Lamb")
        .await
        .unwrap();
    assert!(message.contains("no tie-breaker"), "{message}");
}
