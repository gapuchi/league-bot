use rusqlite::Connection;

use league_bot::{
    db::{self, NflMatchResult, NflPlayerTouchdownTotal, NflTiebreakerPick, Registration, Season},
    league::League,
};

#[test]
fn nfl_standings_use_game_results_and_touchdown_tiebreaker() {
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
    // Tie between Chiefs and Cowboys: both pick up the draw points.
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

    NflTiebreakerPick::upsert(
        &conn,
        season.id,
        200,
        9001,
        "CeeDee Lamb",
        6,
        "Dallas Cowboys",
    )
    .unwrap();
    NflPlayerTouchdownTotal::upsert_batch(&conn, season.id, &[(9001, 7)], "0").unwrap();

    let rows = League::Nfl.standings(&conn, season.id).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!((rows[0].user_id, rows[0].points), (100, 3));
    // Cowboys and Chiefs are level on points; the touchdown tie-breaker ranks Cowboys first.
    assert_eq!((rows[1].user_id, rows[1].points), (200, 1));
    assert_eq!(rows[1].tiebreaker_value, 7);
    assert_eq!(rows[1].tiebreaker_player.as_deref(), Some("CeeDee Lamb"));
    assert_eq!((rows[2].user_id, rows[2].points), (300, 1));
    assert_eq!(rows[2].tiebreaker_value, 0);

    assert_eq!(
        NflMatchResult::score(&conn, season.id, 401).unwrap(),
        Some((24, 20))
    );
    assert_eq!(
        League::Nfl
            .tiebreaker_pick_for_user(&conn, season.id, 200)
            .unwrap(),
        Some(("CeeDee Lamb".into(), "Dallas Cowboys".into()))
    );

    League::Nfl
        .clear_picks_for_team(&conn, season.id, 200, 6)
        .unwrap();
    assert_eq!(
        League::Nfl
            .tiebreaker_pick_for_user(&conn, season.id, 200)
            .unwrap(),
        None
    );
}
