use poise::serenity_prelude as serenity;
use serenity::Mentionable;

use crate::{
    api::{FootballDataApi, Match},
    db::{Registration, SeasonMeta},
    league::League,
    scoring::{self, DRAW_POINTS, FinishedMatch, LOSS_POINTS, WIN_POINTS},
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
    let count = scorers.len();
    let scorer_pairs: Vec<(i64, i64)> = scorers
        .into_iter()
        .map(|scorer| (scorer.player_id, scorer.goals))
        .collect();
    let updated_at = unix_timestamp_secs();

    let conn = data.db.lock().await;
    let cached = seasons
        .iter()
        .filter(|meta| {
            league
                .cache_player_goals(&conn, meta.season.id, &scorer_pairs, &updated_at)
                .inspect_err(|_| {
                    eprintln!(
                        "Failed to cache player goal totals for season {}",
                        meta.season.id
                    )
                })
                .is_ok()
        })
        .count();

    if cached == 0 {
        String::new()
    } else {
        format!(", {count} scorers cached for {cached} season(s)")
    }
}

struct MatchUpdate {
    user_id: u64,
    team_name: String,
    points_earned: i64,
    total_points: i64,
}

pub fn is_finished_match(m: &Match) -> bool {
    m.status.as_deref() == Some("FINISHED") && full_time_score(m).is_some()
}

pub fn unix_timestamp_secs() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
        .to_string()
}

fn match_updates(
    conn: &rusqlite::Connection,
    league: League,
    season_id: i64,
    finished: &FinishedMatch,
    team_ids: [i64; 2],
) -> rusqlite::Result<Vec<MatchUpdate>> {
    team_ids
        .into_iter()
        .filter_map(|team_id| Registration::get_by_team(conn, season_id, team_id).transpose())
        .map(|registration| {
            let registration = registration?;
            Ok(MatchUpdate {
                points_earned: scoring::points_for_team_in_match(registration.team_id, finished),
                total_points: league.user_points(conn, season_id, registration.user_id)?,
                user_id: registration.user_id,
                team_name: registration.team_name,
            })
        })
        .collect()
}

/// Ingest one finished match for a season and announce the result.
///
/// Shared across soccer leagues; `league` selects the per-league tables. Reprocesses
/// only when a previously stored score changed (score correction).
pub async fn process_match(
    data: &Data,
    http: &serenity::Http,
    league: League,
    meta: &SeasonMeta,
    m: &Match,
) -> Result<(), PollError> {
    let season = &meta.season;
    let Some((home_goals, away_goals)) = full_time_score(m) else {
        return Ok(());
    };
    let (Some(home_team_id), Some(away_team_id)) = (m.home_team.id, m.away_team.id) else {
        return Ok(());
    };

    let (updates, is_correction, previous_score) = {
        let conn = data.db.lock().await;
        let previous_score = league.stored_match_score(&conn, season.id, m.id)?;

        if league.is_match_processed(&conn, season.id, m.id)? {
            if previous_score == Some((home_goals, away_goals)) {
                league.upsert_match_result(&conn, season.id, m)?;
                return Ok(());
            }
            league.unmark_match_processed(&conn, season.id, m.id)?;
        }

        let is_correction =
            previous_score.is_some() && previous_score != Some((home_goals, away_goals));
        league.upsert_match_result(&conn, season.id, m)?;

        let finished = FinishedMatch {
            home_team_id,
            away_team_id,
            home_goals,
            away_goals,
        };
        let updates = match_updates(&conn, league, season.id, &finished, [home_team_id, away_team_id])?;
        league.mark_match_processed(&conn, season.id, m.id)?;
        (updates, is_correction, previous_score)
    };

    if updates.is_empty() {
        return Ok(());
    }
    let Some(channel_id) = season.announce_channel_id else {
        return Ok(());
    };

    let stage = match m.matchday {
        Some(day) => format!("Matchday {day}"),
        None => m.stage.clone().unwrap_or_else(|| meta.league_name.clone()),
    };
    let home_name = m.home_team.name.as_deref().unwrap_or("TBD");
    let away_name = m.away_team.name.as_deref().unwrap_or("TBD");

    let correction_line = is_correction
        .then_some(previous_score)
        .flatten()
        .map(|(prev_home, prev_away)| format!("_Previous: {prev_home}–{prev_away}_\n\n"))
        .unwrap_or_default();
    let update_lines: String = updates
        .iter()
        .map(|update| {
            format!(
                "{} ({}) +{} pts → **{}** total\n",
                serenity::UserId::new(update.user_id).mention(),
                update.team_name,
                update.points_earned,
                update.total_points
            )
        })
        .collect();

    let description = format!(
        "{correction_line}**{home_name}** {home_goals}–{away_goals} **{away_name}**\n\n\
         {update_lines}\nScoring: win {WIN_POINTS}, draw {DRAW_POINTS}, loss {LOSS_POINTS}"
    );

    let (title_suffix, colour) = if is_correction {
        ("score corrected", serenity::Colour::GOLD)
    } else {
        ("full time", serenity::Colour::DARK_GREEN)
    };

    let embed = serenity::CreateEmbed::default()
        .title(format!("{stage} — {title_suffix}"))
        .description(description)
        .colour(colour);

    serenity::ChannelId::new(channel_id)
        .send_message(http, serenity::CreateMessage::new().embed(embed))
        .await?;

    Ok(())
}
