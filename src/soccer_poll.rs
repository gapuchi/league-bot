//! football-data.org specifics shared by the soccer leagues: which matches count as
//! finished, mapping a `Match` into the shared [`GameReport`], and scorer caching.

use poise::serenity_prelude as serenity;

use crate::{
    api::{FootballDataApi, Match},
    db::SeasonMeta,
    game_poll::{self, GameReport},
    league::League,
    soccer::full_time_score,
    types::Data,
};

type PollError = Box<dyn std::error::Error + Send + Sync>;

/// Cache top-scorer goal totals for every polled season, returning a log fragment.
pub async fn cache_scorers(
    data: &Data,
    api: &FootballDataApi,
    league: League,
    competition: &str,
    seasons: &[SeasonMeta],
) -> String {
    let scorers = match api.fetch_scorers(competition).await {
        Ok(scorers) => scorers,
        Err(error) => {
            eprintln!("Failed to fetch scorers for {competition}: {error}");
            return String::new();
        }
    };
    let totals: Vec<(i64, i64)> = scorers
        .into_iter()
        .map(|scorer| (scorer.player_id, scorer.goals))
        .collect();
    game_poll::cache_tiebreaker_totals(data, league, seasons, &totals, "goal").await
}

pub fn is_finished_match(m: &Match) -> bool {
    m.status.as_deref() == Some("FINISHED") && full_time_score(m).is_some()
}

/// Provider-neutral view of a finished match. `None` until both team ids and the
/// full-time score are populated (knockout placeholders, delayed scores).
pub fn game_report(m: &Match, league_name: &str) -> Option<GameReport> {
    let (home_score, away_score) = full_time_score(m)?;
    let (Some(home_team_id), Some(away_team_id)) = (m.home_team.id, m.away_team.id) else {
        return None;
    };
    let title = match m.matchday {
        Some(day) => format!("Matchday {day}"),
        None => m.stage.clone().unwrap_or_else(|| league_name.to_owned()),
    };
    Some(GameReport {
        game_id: m.id,
        home_team_id,
        away_team_id,
        home_name: m.home_team.name.clone().unwrap_or_else(|| "TBD".into()),
        away_name: m.away_team.name.clone().unwrap_or_else(|| "TBD".into()),
        home_score,
        away_score,
        stage: m.stage.clone(),
        round: m.matchday,
        playoff: false,
        title,
    })
}

/// Ingest one finished match for a season and announce the result.
pub async fn process_match(
    data: &Data,
    http: &serenity::Http,
    league: League,
    meta: &SeasonMeta,
    m: &Match,
) -> Result<(), PollError> {
    let Some(report) = game_report(m, &meta.league_name) else {
        return Ok(());
    };
    game_poll::process_game(data, http, league, meta, &report).await
}
