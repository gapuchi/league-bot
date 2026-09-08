//! Sport-agnostic ingest of one finished game: persist the score, award points to the
//! owners of both teams, and announce in the season channel. League modules map their
//! provider payload into a [`GameReport`]; per-league tables are reached through `League`.

use poise::serenity_prelude as serenity;
use serenity::Mentionable;

use crate::{
    db::{Registration, SeasonMeta},
    league::League,
    scoring::{self, DRAW_POINTS, FinishedMatch, LOSS_POINTS, WIN_POINTS},
    types::Data,
};

type PollError = Box<dyn std::error::Error + Send + Sync>;

/// Finished game in provider-neutral form.
pub struct GameReport {
    pub game_id: i64,
    pub home_team_id: i64,
    pub away_team_id: i64,
    pub home_name: String,
    pub away_name: String,
    pub home_score: i64,
    pub away_score: i64,
    /// Competition stage as the provider names it (soccer group/knockout); persisted by WC.
    pub stage: Option<String>,
    /// Matchday (soccer) or week (NFL); persisted by EPL, title-only for NFL.
    pub round: Option<i64>,
    /// Announcement title prefix, e.g. `Matchday 3` or `Week 5`.
    pub title: String,
}

impl GameReport {
    pub fn as_finished_match(&self) -> FinishedMatch {
        FinishedMatch {
            home_team_id: self.home_team_id,
            away_team_id: self.away_team_id,
            home_goals: self.home_score,
            away_goals: self.away_score,
        }
    }

    fn score(&self) -> (i64, i64) {
        (self.home_score, self.away_score)
    }
}

struct GameUpdate {
    user_id: u64,
    team_name: String,
    points_earned: i64,
    total_points: i64,
}

pub fn unix_timestamp_secs() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// Cache `(player_id, total)` tie-breaker stats for every polled season, returning a log
/// fragment such as `, 120 goal scorers cached for 2 season(s)`.
pub async fn cache_tiebreaker_totals(
    data: &Data,
    league: League,
    seasons: &[SeasonMeta],
    totals: &[(i64, i64)],
    stat_label: &str,
) -> String {
    let updated_at = unix_timestamp_secs().to_string();
    let conn = data.db.lock().await;
    let cached = seasons
        .iter()
        .filter(|meta| {
            league
                .cache_tiebreaker_totals(&conn, meta.season.id, totals, &updated_at)
                .inspect_err(|error| {
                    eprintln!(
                        "Failed to cache {stat_label} totals for season {}: {error}",
                        meta.season.id
                    )
                })
                .is_ok()
        })
        .count();

    if cached == 0 {
        String::new()
    } else {
        format!(
            ", {} {stat_label} scorers cached for {cached} season(s)",
            totals.len()
        )
    }
}

fn game_updates(
    conn: &rusqlite::Connection,
    league: League,
    season_id: i64,
    finished: &FinishedMatch,
) -> rusqlite::Result<Vec<GameUpdate>> {
    [finished.home_team_id, finished.away_team_id]
        .into_iter()
        .filter_map(|team_id| Registration::get_by_team(conn, season_id, team_id).transpose())
        .map(|registration| {
            let registration = registration?;
            Ok(GameUpdate {
                points_earned: scoring::points_for_team_in_match(registration.team_id, finished),
                total_points: league.user_points(conn, season_id, registration.user_id)?,
                user_id: registration.user_id,
                team_name: registration.team_name,
            })
        })
        .collect()
}

/// Ingest one finished game for a season and announce the result.
///
/// Idempotent per `(season, game_id)`; reprocesses only when a previously stored score
/// changed (score correction).
pub async fn process_game(
    data: &Data,
    http: &serenity::Http,
    league: League,
    meta: &SeasonMeta,
    report: &GameReport,
) -> Result<(), PollError> {
    let season = &meta.season;

    let (updates, is_correction, previous_score) = {
        let conn = data.db.lock().await;
        let previous_score = league.stored_match_score(&conn, season.id, report.game_id)?;

        if league.is_match_processed(&conn, season.id, report.game_id)? {
            if previous_score == Some(report.score()) {
                league.upsert_match_result(&conn, season.id, report)?;
                return Ok(());
            }
            league.unmark_match_processed(&conn, season.id, report.game_id)?;
        }

        let is_correction = previous_score.is_some() && previous_score != Some(report.score());
        league.upsert_match_result(&conn, season.id, report)?;

        let updates = game_updates(&conn, league, season.id, &report.as_finished_match())?;
        league.mark_match_processed(&conn, season.id, report.game_id)?;
        (updates, is_correction, previous_score)
    };

    if updates.is_empty() {
        return Ok(());
    }
    let Some(channel_id) = season.announce_channel_id else {
        return Ok(());
    };

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

    let draw_label = league.draw_label();
    let description = format!(
        "{correction_line}**{}** {}–{} **{}**\n\n\
         {update_lines}\nScoring: win {WIN_POINTS}, {draw_label} {DRAW_POINTS}, loss {LOSS_POINTS}",
        report.home_name, report.home_score, report.away_score, report.away_name
    );

    let (title_suffix, colour) = if is_correction {
        ("score corrected", serenity::Colour::GOLD)
    } else {
        (league.finished_label(), serenity::Colour::DARK_GREEN)
    };

    let embed = serenity::CreateEmbed::default()
        .title(format!("{} — {title_suffix}", report.title))
        .description(description)
        .colour(colour);

    serenity::ChannelId::new(channel_id)
        .send_message(http, serenity::CreateMessage::new().embed(embed))
        .await?;

    Ok(())
}
