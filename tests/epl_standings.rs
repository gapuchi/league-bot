use rusqlite::Connection;

use league_bot::{
    db::{self, EplMatchResult, Registration, Season},
    league::League,
};

#[test]
fn epl_standings_use_match_results() {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();

    let season = Season::get_or_create(&conn, 111, "epl", "2025-26", "Premier League 2025-26").unwrap();
    Registration::upsert(&conn, season.id, 100, 64, "Liverpool FC").unwrap();
    Registration::upsert(&conn, season.id, 200, 57, "Arsenal FC").unwrap();

    EplMatchResult {
        season_id: season.id,
        match_id: 1,
        home_team_id: 64,
        away_team_id: 57,
        home_goals: 2,
        away_goals: 1,
        matchday: Some(1),
    }
    .upsert(&conn)
    .unwrap();

    let rows = League::Epl.standings(&conn, season.id).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].user_id, 100);
    assert_eq!(rows[0].points, 3.0);
    assert_eq!(rows[1].user_id, 200);
    assert_eq!(rows[1].points, 0.0);
}
