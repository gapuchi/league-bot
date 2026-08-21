use poise::serenity_prelude as serenity;
use serenity::Mentionable;

use crate::{
    api::{FootballDataApi, Match, Team},
    db::{Registration, SeasonMeta, WcAnnouncedElimination, league_competition_code},
    league::{League, PollOutcome},
    soccer::{classify_teams, TeamRef},
    soccer_poll,
    types::Data,
};

struct EliminationNotice {
    team: TeamRef,
    owner_id: Option<u64>,
}

pub async fn poll(
    data: &Data,
    http: &serenity::Http,
    seasons: &[SeasonMeta],
) -> Result<PollOutcome, Box<dyn std::error::Error + Send + Sync>> {
    let competition = league_competition_code("wc");
    let api = FootballDataApi::from_env(data.http.clone());
    let matches = api.fetch_competition_matches(&competition).await?;
    let teams = api.fetch_teams(&competition).await?;
    let finished_matches: Vec<&Match> =
        matches.iter().filter(|m| soccer_poll::is_finished_match(m)).collect();

    let scorers_line =
        soccer_poll::cache_scorers(data, &api, League::Wc, &competition, seasons).await;

    for meta in seasons {
        for m in &finished_matches {
            if let Err(error) = soccer_poll::process_match(data, http, League::Wc, meta, m).await {
                eprintln!(
                    "Failed to process match {} for season {}: {error}",
                    m.id, meta.season.id
                );
            }
        }
        if let Err(error) = announce_new_eliminations(data, http, meta, &teams, &matches).await {
            eprintln!(
                "Failed to announce eliminations for season {}: {error}",
                meta.season.id
            );
        }
    }

    Ok(PollOutcome {
        finished_matches: finished_matches.len(),
        scored_matches: finished_matches.len(),
        seasons: seasons.len(),
        detail: format!("WC poll{scorers_line}"),
    })
}

async fn announce_new_eliminations(
    data: &Data,
    http: &serenity::Http,
    meta: &SeasonMeta,
    teams: &[Team],
    matches: &[Match],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let season = &meta.season;
    let classification = classify_teams(teams, matches);

    let notices = {
        let conn = data.db.lock().await;
        let announced = WcAnnouncedElimination::list_for_season(&conn, season.id)?;
        classification
            .eliminated
            .into_iter()
            .filter(|team| !announced.contains(&team.id))
            .map(|team| {
                let owner_id = Registration::get_by_team(&conn, season.id, team.id)?
                    .map(|registration| registration.user_id);
                Ok(EliminationNotice { team, owner_id })
            })
            .collect::<rusqlite::Result<Vec<_>>>()?
    };

    if notices.is_empty() {
        return Ok(());
    }

    let Some(channel_id) = season.announce_channel_id else {
        return Ok(());
    };

    post_elimination_announcement(http, channel_id, &notices).await?;

    {
        let conn = data.db.lock().await;
        for notice in &notices {
            WcAnnouncedElimination::mark(&conn, season.id, notice.team.id)?;
        }
    }

    Ok(())
}

async fn post_elimination_announcement(
    http: &serenity::Http,
    channel_id: u64,
    notices: &[EliminationNotice],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let title = if notices.len() == 1 {
        format!("{} eliminated", notices[0].team.name)
    } else {
        "Teams eliminated".into()
    };

    let description = notices
        .iter()
        .map(format_elimination_line)
        .collect::<Vec<_>>()
        .join("\n");

    let embed = serenity::CreateEmbed::default()
        .title(title)
        .description(description)
        .colour(serenity::Colour::DARK_RED);

    serenity::ChannelId::new(channel_id)
        .send_message(http, serenity::CreateMessage::new().embed(embed))
        .await?;

    Ok(())
}

fn format_elimination_line(notice: &EliminationNotice) -> String {
    match notice.owner_id {
        Some(user_id) => format!(
            "{} — **{}** is out of the tournament.",
            serenity::UserId::new(user_id).mention(),
            notice.team.name
        ),
        None => format!("**{}** is out of the tournament.", notice.team.name),
    }
}
