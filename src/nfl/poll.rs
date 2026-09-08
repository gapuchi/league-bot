use poise::serenity_prelude as serenity;

use crate::{
    api::EspnNflApi,
    db::SeasonMeta,
    game_poll::{self, GameReport},
    league::{League, PollOutcome},
    nfl::season,
    types::Data,
};

type PollError = Box<dyn std::error::Error + Send + Sync>;

/// Every live NFL season tracks the current NFL calendar season (same as the soccer
/// leagues tracking the current competition edition).
pub async fn poll(
    data: &Data,
    http: &serenity::Http,
    seasons: &[SeasonMeta],
) -> Result<PollOutcome, PollError> {
    let api = EspnNflApi::new(data.http.clone());
    let season_year = season::current_season_year();
    let (start, end) = season::scoreboard_range(season_year);
    let games = api.fetch_games_between(&start, &end).await?;
    let reports: Vec<GameReport> = games.iter().filter_map(season::game_report).collect();

    let touchdowns_line = cache_touchdowns(data, &api, season_year, seasons).await;

    for meta in seasons {
        for report in &reports {
            if let Err(error) = game_poll::process_game(data, http, League::Nfl, meta, report).await
            {
                eprintln!(
                    "Failed to process game {} for season {}: {error}",
                    report.game_id, meta.season.id
                );
            }
        }
    }

    Ok(PollOutcome {
        finished_matches: reports.len(),
        scored_matches: reports.len(),
        seasons: seasons.len(),
        detail: format!("NFL {season_year} poll{touchdowns_line}"),
    })
}

/// Cache regular-season touchdown totals for tie-breakers, returning a log fragment.
async fn cache_touchdowns(
    data: &Data,
    api: &EspnNflApi,
    season_year: i64,
    seasons: &[SeasonMeta],
) -> String {
    let leaders = match api
        .fetch_touchdown_leaders(season_year, season::REGULAR_SEASON)
        .await
    {
        Ok(leaders) => leaders,
        Err(error) => {
            eprintln!("Failed to fetch NFL touchdown leaders for {season_year}: {error}");
            return String::new();
        }
    };
    let totals: Vec<(i64, i64)> = leaders
        .into_iter()
        .map(|leader| (leader.player_id, leader.touchdowns))
        .collect();
    game_poll::cache_tiebreaker_totals(data, League::Nfl, seasons, &totals, "touchdown").await
}
