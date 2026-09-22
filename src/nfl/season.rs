//! NFL calendar and ESPN game interpretation: which season is current, which games
//! count for the pool, and how a finished game maps into the shared [`GameReport`].

use crate::{
    api::{NflCompetitor, NflGame},
    clock::{civil_year_month, unix_timestamp_secs},
    game_poll::GameReport,
};

pub const REGULAR_SEASON: i64 = 2;
pub const POSTSEASON: i64 = 3;
/// ESPN slots the Pro Bowl Games as postseason week 4 between the conference
/// championships and the Super Bowl; its AFC/NFC squads are not claimable teams.
const PRO_BOWL_WEEK: i64 = 4;
const SUPER_BOWL_WEEK: i64 = 5;

/// NFL seasons are named for the year they kick off; the postseason runs into the
/// following February. March is the boundary.
pub fn season_year_at(unix_secs: u64) -> i64 {
    let (year, month) = civil_year_month(unix_secs);
    if month >= 3 { year } else { year - 1 }
}

pub fn current_season_year() -> i64 {
    season_year_at(unix_timestamp_secs())
}

/// Calendar years to query to cover a whole NFL season: preseason and most of the
/// regular season fall in `season_year`, while week 18 and the playoffs run into
/// `season_year + 1`. Games from other seasons come back too and are dropped with
/// [`belongs_to_season`].
pub fn scoreboard_years(season_year: i64) -> [i64; 2] {
    [season_year, season_year + 1]
}

pub fn belongs_to_season(game: &NflGame, season_year: i64) -> bool {
    game.season_year == season_year
}

/// Regular-season and playoff games only — no preseason, no Pro Bowl.
pub fn counts_for_pool(game: &NflGame) -> bool {
    match game.season_type {
        REGULAR_SEASON => true,
        POSTSEASON => game.week != Some(PRO_BOWL_WEEK),
        _ => false,
    }
}

fn side<'a>(game: &'a NflGame, home_away: &str) -> Option<&'a NflCompetitor> {
    game.competitors
        .iter()
        .find(|competitor| competitor.home_away == home_away)
}

pub fn round_title(game: &NflGame) -> String {
    match (game.season_type, game.week) {
        (POSTSEASON, Some(1)) => "Wild Card".into(),
        (POSTSEASON, Some(2)) => "Divisional Round".into(),
        (POSTSEASON, Some(3)) => "Conference Championships".into(),
        (POSTSEASON, Some(SUPER_BOWL_WEEK)) => "Super Bowl".into(),
        (POSTSEASON, _) => "Postseason".into(),
        (_, Some(week)) => format!("Week {week}"),
        (_, None) => "NFL".into(),
    }
}

/// Provider-neutral view of a completed, pool-eligible game. `None` while the game is
/// still in progress, is preseason/Pro Bowl, or lacks a score for either side.
pub fn game_report(game: &NflGame) -> Option<GameReport> {
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
        round: game.week,
        playoff: game.season_type == POSTSEASON,
        title: round_title(game),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::NflTeam;

    fn competitor(home_away: &str, id: i64, score: Option<i64>) -> NflCompetitor {
        NflCompetitor {
            home_away: home_away.into(),
            score,
            team: NflTeam {
                id,
                display_name: format!("Team {id}"),
                short_display_name: None,
                abbreviation: None,
            },
        }
    }

    fn game(season_type: i64, week: Option<i64>, completed: bool) -> NflGame {
        NflGame {
            id: 401,
            season_year: 2026,
            season_type,
            week,
            completed,
            competitors: vec![
                competitor("home", 21, Some(24)),
                competitor("away", 6, Some(20)),
            ],
        }
    }

    #[test]
    fn season_year_rolls_over_in_march() {
        // 2026-01-15T12:00Z belongs to the 2025 season.
        assert_eq!(season_year_at(1_768_478_400), 2025);
        // 2026-09-08T18:00Z belongs to the 2026 season.
        assert_eq!(season_year_at(1_788_890_400), 2026);
        // 2026-02-28T23:59Z is still the 2025 season; 2026-03-01T00:00Z rolls over.
        assert_eq!(season_year_at(1_772_323_140), 2025);
        assert_eq!(season_year_at(1_772_323_200), 2026);
        assert_eq!(scoreboard_years(2026), [2026, 2027]);
    }

    #[test]
    fn belongs_to_season_filters_other_seasons() {
        let mut this_season = game(REGULAR_SEASON, Some(18), true);
        this_season.season_year = 2026;
        assert!(belongs_to_season(&this_season, 2026));

        let mut prior_playoffs = game(POSTSEASON, Some(1), true);
        prior_playoffs.season_year = 2025;
        assert!(!belongs_to_season(&prior_playoffs, 2026));
    }

    #[test]
    fn game_report_requires_completed_pool_game_with_scores() {
        let report = game_report(&game(REGULAR_SEASON, Some(1), true)).unwrap();
        assert_eq!(report.game_id, 401);
        assert_eq!((report.home_team_id, report.away_team_id), (21, 6));
        assert_eq!((report.home_score, report.away_score), (24, 20));
        assert_eq!(report.round, Some(1));
        assert_eq!(report.title, "Week 1");

        assert!(game_report(&game(REGULAR_SEASON, Some(1), false)).is_none());
        assert!(game_report(&game(1, Some(2), true)).is_none());
        assert!(game_report(&game(POSTSEASON, Some(PRO_BOWL_WEEK), true)).is_none());

        let mut missing_score = game(REGULAR_SEASON, Some(1), true);
        missing_score.competitors[1].score = None;
        assert!(game_report(&missing_score).is_none());
    }

    #[test]
    fn postseason_rounds_are_named() {
        assert_eq!(round_title(&game(POSTSEASON, Some(1), true)), "Wild Card");
        assert_eq!(round_title(&game(POSTSEASON, Some(5), true)), "Super Bowl");
        assert_eq!(
            round_title(&game(REGULAR_SEASON, Some(12), true)),
            "Week 12"
        );
    }
}
