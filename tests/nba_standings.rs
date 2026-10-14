use std::sync::Arc;

use rusqlite::Connection;
use tokio::sync::Mutex;

use league_bot::{
    db::{self, NbaMatchResult, Registration, Season},
    league::League,
    standings::{format_standing_detail, standings_ranks},
    types::Data,
};

fn seeded_conn() -> (Connection, Season) {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();

    let season = Season::get_or_create(&conn, 111, "nba", "2026", "NBA 2026", 1).unwrap();
    Registration::upsert(&conn, season.id, 100, 13, "Los Angeles Lakers").unwrap();
    Registration::upsert(&conn, season.id, 200, 2, "Boston Celtics").unwrap();
    Registration::upsert(&conn, season.id, 300, 14, "Los Angeles Clippers").unwrap();

    NbaMatchResult {
        season_id: season.id,
        game_id: 401,
        home_team_id: 13,
        away_team_id: 2,
        home_score: 112,
        away_score: 104,
        postseason: false,
    }
    .upsert(&conn)
    .unwrap();
    NbaMatchResult {
        season_id: season.id,
        game_id: 402,
        home_team_id: 14,
        away_team_id: 2,
        home_score: 98,
        away_score: 101,
        postseason: false,
    }
    .upsert(&conn)
    .unwrap();

    (conn, season)
}

#[test]
fn nba_standings_use_game_results_without_a_tiebreaker() {
    let (conn, season) = seeded_conn();

    let rows = League::Nba.standings(&conn, season.id).unwrap();
    assert_eq!(rows.len(), 3);
    // Lakers and Celtics each have 1 win; Clippers have 1 loss. Tied points share a
    // rank and sort by user_id.
    assert_eq!((rows[0].user_id, rows[0].points), (100, 1.0));
    assert_eq!((rows[1].user_id, rows[1].points), (200, 1.0));
    assert_eq!((rows[2].user_id, rows[2].points), (300, 0.0));
    assert!(
        rows.iter()
            .all(|row| row.tiebreaker_value == 0 && row.tiebreaker_player.is_none())
    );

    assert_eq!(standings_ranks(&rows), vec![1, 1, 3]);
    let detail = format_standing_detail(1, &rows[0], League::Nba.tiebreaker_unit());
    assert!(!detail.contains("Tie-breaker"), "{detail}");

    assert_eq!(
        NbaMatchResult::score(&conn, season.id, 401).unwrap(),
        Some((112, 104))
    );
    assert_eq!(
        League::Nba
            .tiebreaker_pick_for_user(&conn, season.id, 200)
            .unwrap(),
        None
    );
    League::Nba
        .clear_picks_for_team(&conn, season.id, 200, 2)
        .unwrap();
}

#[test]
fn nba_playoff_win_awards_two_points() {
    let (conn, season) = seeded_conn();

    NbaMatchResult {
        season_id: season.id,
        game_id: 500,
        home_team_id: 13,
        away_team_id: 14,
        home_score: 110,
        away_score: 99,
        postseason: true,
    }
    .upsert(&conn)
    .unwrap();

    let rows = League::Nba.standings(&conn, season.id).unwrap();
    let lakers = rows.iter().find(|r| r.user_id == 100).unwrap();
    // 1 for the regular-season win + 2 for the playoff win.
    assert_eq!(lakers.points, 3.0);
}

#[tokio::test]
async fn nba_pick_player_is_declined_without_network() {
    let (conn, season) = seeded_conn();
    let data = Data {
        db: Arc::new(Mutex::new(conn)),
        http: reqwest::Client::new(),
    };

    let message = League::Nba
        .pick_tiebreaker_player(&data, season.id, 200, "Tatum")
        .await
        .unwrap();
    assert!(message.contains("no tie-breaker"), "{message}");
}
