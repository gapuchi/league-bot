use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use poise::serenity_prelude as serenity;

use crate::{
    db::{Season, SeasonMeta},
    league::League,
    types::Data,
};

pub fn start_poller(data: Arc<Data>, cache_http: Arc<serenity::Http>) {
    tokio::spawn(async move {
        const POLL_INTERVAL: Duration = Duration::from_secs(300);
        // Let the Discord gateway finish connecting before the first HTTP request.
        tokio::time::sleep(Duration::from_secs(5)).await;

        loop {
            if let Err(error) = poll_once(&data, &cache_http).await {
                eprintln!("Poll failed: {error:#}");
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    });
}

async fn poll_once(
    data: &Data,
    http: &serenity::Http,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let season_metas = {
        let conn = data.db.lock().await;
        Season::list_live_with_meta(&conn)?
    };

    if season_metas.is_empty() {
        eprintln!("Poll complete: no live seasons configured");
        return Ok(());
    }

    let by_league = season_metas.into_iter().fold(
        HashMap::<String, Vec<SeasonMeta>>::new(),
        |mut map, meta| {
            map.entry(meta.league_slug.clone()).or_default().push(meta);
            map
        },
    );

    let mut total_matches = 0;
    let mut total_scored = 0;
    let mut total_seasons = 0;

    for (league_slug, seasons) in by_league {
        let Some(league) = League::from_slug(&league_slug) else {
            eprintln!(
                "No compiled league for \"{league_slug}\" ({} season(s) skipped)",
                seasons.len()
            );
            continue;
        };

        let outcome = league.poll(data, http, &seasons).await?;
        total_matches += outcome.finished_matches;
        total_scored += outcome.scored_matches;
        total_seasons += outcome.seasons;
        eprintln!(
            "{}: {} finished match(es) ({} with scores), {} season(s){}",
            league.slug(),
            outcome.finished_matches,
            outcome.scored_matches,
            outcome.seasons,
            if outcome.detail.is_empty() {
                String::new()
            } else {
                format!(" ({})", outcome.detail)
            }
        );
    }

    eprintln!(
        "Poll complete: {total_matches} finished match(es) ({total_scored} with scores), {total_seasons} season(s)",
    );

    Ok(())
}
