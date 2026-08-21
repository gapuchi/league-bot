use poise::serenity_prelude as serenity;

use crate::{
    api::{FootballDataApi, Match},
    db::league_competition_code,
    league::{League, PollOutcome},
    soccer_poll,
    types::Data,
};

pub async fn poll(
    data: &Data,
    http: &serenity::Http,
    seasons: &[crate::db::SeasonMeta],
) -> Result<PollOutcome, Box<dyn std::error::Error + Send + Sync>> {
    let competition = league_competition_code("epl");
    let api = FootballDataApi::from_env(data.http.clone());
    let matches = api.fetch_competition_matches(&competition).await?;
    let finished_matches: Vec<&Match> =
        matches.iter().filter(|m| soccer_poll::is_finished_match(m)).collect();

    let scorers_line =
        soccer_poll::cache_scorers(data, &api, League::Epl, &competition, seasons).await;

    for meta in seasons {
        for m in &finished_matches {
            if let Err(error) = soccer_poll::process_match(data, http, League::Epl, meta, m).await {
                eprintln!(
                    "Failed to process match {} for season {}: {error}",
                    m.id, meta.season.id
                );
            }
        }
    }

    Ok(PollOutcome {
        finished_matches: finished_matches.len(),
        scored_matches: finished_matches.len(),
        seasons: seasons.len(),
        detail: format!("EPL poll{scorers_line}"),
    })
}
