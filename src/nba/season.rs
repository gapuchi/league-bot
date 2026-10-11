//! NBA calendar and ESPN game interpretation: which season is current, which games
//! count for the pool, and how a finished game maps into the shared [`GameReport`].

use crate::{
    api::{NbaCompetitor, NbaGame},
    clock::{civil_year_month, unix_timestamp_secs},
    game_poll::GameReport,
};

pub const REGULAR_SEASON: i64 = 2;
pub const POSTSEASON: i64 = 3;

/// NBA seasons tip off in October and finish the following June, and ESPN labels each
/// season by the year it ends (the 2025-26 season is `2026`). A date from September
/// onward belongs to the season that ends the next calendar year.
pub fn season_year_at(unix_secs: u64) -> i64 {
    let (year, month) = civil_year_month(unix_secs);
    if month >= 9 { year + 1 } else { year }
}

pub fn current_season_year() -> i64 {
    season_year_at(unix_timestamp_secs())
}

/// Calendar years to query to cover a whole NBA season: the season ending in
/// `season_year` is played across the previous autumn and that spring. Games from other
/// seasons come back too and are dropped with [`belongs_to_season`].
pub fn scoreboard_years(season_year: i64) -> [i64; 2] {
    [season_year - 1, season_year]
}

pub fn belongs_to_season(game: &NbaGame, season_year: i64) -> bool {
    game.season_year == season_year
}

/// Regular-season and playoff games only — no preseason. The All-Star Game is a
/// regular-season event but its competitors are conference squads, not franchises, so
/// no one can claim them and it never scores.
pub fn counts_for_pool(game: &NbaGame) -> bool {
    matches!(game.season_type, REGULAR_SEASON | POSTSEASON)
}

fn side<'a>(game: &'a NbaGame, home_away: &str) -> Option<&'a NbaCompetitor> {
    game.competitors
        .iter()
        .find(|competitor| competitor.home_away == home_away)
}

pub fn round_title(game: &NbaGame) -> String {
    if game.season_type == POSTSEASON {
        "Playoffs".into()
    } else {
        "Regular Season".into()
    }
}

/// Provider-neutral view of a completed, pool-eligible game. `None` while the game is
/// still in progress, is preseason, or lacks a score for either side.
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
            season_year: 2026,
            season_type,
            completed,
            competitors: vec![
                competitor("home", 13, Some(112)),
                competitor("away", 2, Some(104)),
            ],
        }
    }

    #[test]
    fn season_year_follows_the_ending_year() {
        // 2025-10-24T23:30Z (tip-off) belongs to the 2026 season.
        assert_eq!(season_year_at(1_761_348_600), 2026);
        // 2026-01-15T12:00Z (midseason) belongs to the 2026 season.
        assert_eq!(season_year_at(1_768_478_400), 2026);
        // 2026-06-18T00:30Z (Finals) belongs to the 2026 season.
        assert_eq!(season_year_at(1_781_742_600), 2026);
        assert_eq!(scoreboard_years(2026), [2025, 2026]);
    }

    #[test]
    fn belongs_to_season_filters_other_seasons() {
        let mut this_season = game(REGULAR_SEASON, true);
        this_season.season_year = 2026;
        assert!(belongs_to_season(&this_season, 2026));

        let mut prior_playoffs = game(POSTSEASON, true);
        prior_playoffs.season_year = 2025;
        assert!(!belongs_to_season(&prior_playoffs, 2026));
    }

    #[test]
    fn game_report_requires_completed_pool_game_with_scores() {
        let report = game_report(&game(REGULAR_SEASON, true)).unwrap();
        assert_eq!(report.game_id, 401);
        assert_eq!((report.home_team_id, report.away_team_id), (13, 2));
        assert_eq!((report.home_score, report.away_score), (112, 104));
        assert!(!report.playoff);
        assert_eq!(report.title, "Regular Season");

        assert!(game_report(&game(REGULAR_SEASON, false)).is_none());
        assert!(game_report(&game(1, true)).is_none());

        let playoff = game_report(&game(POSTSEASON, true)).unwrap();
        assert!(playoff.playoff);
        assert_eq!(playoff.title, "Playoffs");

        let mut missing_score = game(REGULAR_SEASON, true);
        missing_score.competitors[1].score = None;
        assert!(game_report(&missing_score).is_none());
    }
}
