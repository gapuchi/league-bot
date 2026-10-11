use poise::serenity_prelude as serenity;

use crate::{
    api::EspnNbaApi,
    db::SeasonMeta,
    game_poll::{self, GameReport},
    league::{League, PollOutcome},
    nba::season,
    types::Data,
};

type PollError = Box<dyn std::error::Error + Send + Sync>;

/// Every live NBA season tracks the current NBA calendar season (same as the soccer
/// leagues tracking the current competition edition).
pub async fn poll(
    data: &Data,
    http: &serenity::Http,
    seasons: &[SeasonMeta],
) -> Result<PollOutcome, PollError> {
    let api = EspnNbaApi::new(data.http.clone());
    let season_year = season::current_season_year();
    let mut games = Vec::new();
    for year in season::scoreboard_years(season_year) {
        games.extend(api.fetch_games_for_year(year).await?);
    }
    let reports: Vec<GameReport> = games
        .iter()
        .filter(|game| season::belongs_to_season(game, season_year))
        .filter_map(season::game_report)
        .collect();

    for meta in seasons {
        for report in &reports {
            if let Err(error) = game_poll::process_game(data, http, League::Nba, meta, report).await
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
        seasons: seasons.len(),
        detail: format!("NBA {season_year} poll"),
    })
}
