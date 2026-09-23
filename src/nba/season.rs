//! NBA calendar and ESPN game interpretation: which season is current, which games
//! count for the pool, and how a finished game maps into the shared [`GameReport`].

use crate::{
    api::{NbaCompetitor, NbaGame},
    clock::{civil_year_month, unix_timestamp_secs},
    game_poll::GameReport,
};

pub const REGULAR_SEASON: i64 = 2;
pub const POSTSEASON: i64 = 3;
/// ESPN's Play-In Tournament sits between the regular season and the playoffs.
pub const PLAY_IN: i64 = 5;

/// NBA seasons are named for the year they tip off in October; the playoffs run into
/// the following June. The offseason (July–September) is the boundary.
pub fn season_year_at(unix_secs: u64) -> i64 {
    let (year, month) = civil_year_month(unix_secs);
    if month >= 8 { year } else { year - 1 }
}

pub fn current_season_year() -> i64 {
    season_year_at(unix_timestamp_secs())
}

/// Calendar months (`YYYYMM`) covering tip-off through the Finals. ESPN's scoreboard
/// rejects day ranges and caps a year query at 1000 events, so the poller fetches one
/// month at a time.
pub fn scoreboard_months(season_year: i64) -> Vec<String> {
    let mut months = Vec::with_capacity(9);
    for month in 10..=12 {
        months.push(format!("{season_year}{month:02}"));
    }
    for month in 1..=6 {
        months.push(format!("{}{month:02}", season_year + 1));
    }
    months
}

/// Regular-season, Play-In, and playoff games — no preseason, no All-Star Game.
pub fn counts_for_pool(game: &NbaGame) -> bool {
    matches!(game.season_type, REGULAR_SEASON | POSTSEASON | PLAY_IN)
}

fn side<'a>(game: &'a NbaGame, home_away: &str) -> Option<&'a NbaCompetitor> {
    game.competitors
        .iter()
        .find(|competitor| competitor.home_away == home_away)
}

pub fn round_title(game: &NbaGame) -> String {
    match game.season_type {
        POSTSEASON => "Playoffs".into(),
        PLAY_IN => "Play-In".into(),
        _ => "Regular Season".into(),
    }
}

/// Provider-neutral view of a completed, pool-eligible game. `None` while the game is
/// still in progress, is preseason/All-Star, or lacks a score for either side.
pub fn game_report(game: &NbaGame) -> Option<GameReport> {
    if !game.completed || !counts_for_pool(game) {
        return None;
    }
    let home = side(game, "home")?;
    let away = side(game, "away")?;
    Some(GameReport {
        game_id: game.id,
        home_team_id: home.team.id,
        away_team_id: away.team.id,
        home_name: home.team.display_name.clone(),
        away_name: away.team.display_name.clone(),
        home_score: home.score?,
        away_score: away.score?,
        stage: None,
        round: None,
        playoff: game.season_type == POSTSEASON,
        title: round_title(game),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::NbaTeam;

    fn competitor(home_away: &str, id: i64, score: Option<i64>) -> NbaCompetitor {
        NbaCompetitor {
            home_away: home_away.into(),
            score,
            team: NbaTeam {
                id,
                display_name: format!("Team {id}"),
                short_display_name: None,
                abbreviation: None,
            },
        }
    }

    fn game(season_type: i64, completed: bool) -> NbaGame {
        NbaGame {
            id: 401,
            season_type,
            completed,
            competitors: vec![
                competitor("home", 13, Some(118)),
                competitor("away", 2, Some(110)),
            ],
        }
    }

    #[test]
    fn season_year_rolls_over_in_offseason() {
        // 2025-10-22T00:00Z (tip-off) belongs to the 2025 season.
        assert_eq!(season_year_at(1_761_091_200), 2025);
        // 2026-06-01T00:00Z (Finals) still belongs to the 2025 season.
        assert_eq!(season_year_at(1_780_272_000), 2025);
        // 2026-08-01T00:00Z rolls over into the 2026 season.
        assert_eq!(season_year_at(1_785_542_400), 2026);
        assert_eq!(
            scoreboard_months(2025),
            vec![
                "202510", "202511", "202512", "202601", "202602", "202603", "202604", "202605",
                "202606",
            ]
        );
    }

    #[test]
    fn game_report_requires_completed_pool_game_with_scores() {
        let report = game_report(&game(REGULAR_SEASON, true)).unwrap();
        assert_eq!(report.game_id, 401);
        assert_eq!((report.home_team_id, report.away_team_id), (13, 2));
        assert_eq!((report.home_score, report.away_score), (118, 110));
        assert!(!report.playoff);
        assert_eq!(report.title, "Regular Season");

        let playoff = game_report(&game(POSTSEASON, true)).unwrap();
        assert!(playoff.playoff);
        assert_eq!(playoff.title, "Playoffs");

        let play_in = game_report(&game(PLAY_IN, true)).unwrap();
        assert!(!play_in.playoff);
        assert_eq!(play_in.title, "Play-In");

        assert!(game_report(&game(REGULAR_SEASON, false)).is_none());
        assert!(game_report(&game(1, true)).is_none());

        let mut missing_score = game(REGULAR_SEASON, true);
        missing_score.competitors[1].score = None;
        assert!(game_report(&missing_score).is_none());
    }
}
