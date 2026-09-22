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
        seasons: seasons.len(),
        detail: format!("NFL {season_year} poll"),
    })
}
