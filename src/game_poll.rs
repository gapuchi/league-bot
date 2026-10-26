//! Sport-agnostic ingest of one finished game: persist the score, award points to the
//! owners of both teams, and announce in the channel where the season was started.
//! League modules map their provider payload into a [`GameReport`]; per-league tables
//! are reached through `League`.

use poise::serenity_prelude as serenity;
use serenity::Mentionable;

use crate::{
    clock::unix_timestamp_secs,
    db::{Registration, SeasonMeta},
    league::League,
    scoring::{self, FinishedMatch},
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
    /// Playoff/postseason game — scored with `ScoringRules::playoff_win` when set.
    pub playoff: bool,
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
            playoff: self.playoff,
        }
    }

    fn score(&self) -> (i64, i64) {
        (self.home_score, self.away_score)
    }
}

struct GameUpdate {
    user_id: u64,
    team_name: String,
    points_earned: f64,
    total_points: f64,
}

/// One finished game's announcement content. [`process_game`] builds these and the poller
/// hands a season's batch to [`announce_results`], which renders them into a single embed.
pub struct GameAnnouncement {
    /// Field heading, e.g. `Week 5 — final`.
    heading: String,
    /// Field body: score line, per-owner point updates, and the scoring rule.
    body: String,
    /// A corrected score tints the whole embed gold.
    is_correction: bool,
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
                points_earned: scoring::points_for_team_in_match(
                    league.scoring(),
                    registration.team_id,
                    finished,
                ),
                total_points: league.user_points(conn, season_id, registration.user_id)?,
                user_id: registration.user_id,
                team_name: registration.team_name,
            })
        })
        .collect()
}

/// Ingest one finished game for a season, returning its announcement content when there is
/// something new to report. Callers batch these into a single embed per season.
///
/// Idempotent per `(season, game_id)`; reprocesses only when a previously stored score
/// changed (score correction). Returns `None` when the game was already announced with the
/// same score or when neither team is claimed.
pub async fn process_game(
    data: &Data,
    league: League,
    meta: &SeasonMeta,
    report: &GameReport,
) -> Result<Option<GameAnnouncement>, PollError> {
    let season = &meta.season;

    let (updates, is_correction, previous_score) = {
        let conn = data.db.lock().await;
        let previous_score = league.stored_match_score(&conn, season.id, report.game_id)?;

        if league.is_match_processed(&conn, season.id, report.game_id)? {
            if previous_score == Some(report.score()) {
                league.upsert_match_result(&conn, season.id, report)?;
                return Ok(None);
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
        return Ok(None);
    }

    let correction_line = is_correction
        .then_some(previous_score)
        .flatten()
        .map(|(prev_home, prev_away)| format!("_Previous: {prev_home}–{prev_away}_\n"))
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
    let rules = league.scoring();
    let win_points = match (report.playoff, rules.playoff_win) {
        (true, Some(win)) => win,
        _ => rules.win,
    };
    let win_label = if report.playoff && rules.playoff_win.is_some() {
        "playoff win"
    } else {
        "win"
    };

    let title_suffix = if is_correction {
        "score corrected"
    } else {
        league.finished_label()
    };
    let heading = format!("{} — {title_suffix}", report.title);
    let body = format!(
        "{correction_line}**{}** {}–{} **{}**\n\
         {update_lines}Scoring: {win_label} {win_points}, {draw_label} {}, loss {}",
        report.home_name,
        report.home_score,
        report.away_score,
        report.away_name,
        rules.draw,
        rules.loss
    );

    Ok(Some(GameAnnouncement {
        heading,
        body,
        is_correction,
    }))
}

/// Post a season's new results as a single combined embed, one field per game. Discord caps
/// an embed at 25 fields, so a longer slate spills into additional embeds (sent as their own
/// messages). No-op when there is nothing new to report.
pub async fn announce_results(
    http: &serenity::Http,
    channel_id: u64,
    league: League,
    announcements: Vec<GameAnnouncement>,
) -> Result<(), PollError> {
    if announcements.is_empty() {
        return Ok(());
    }

    let channel = serenity::ChannelId::new(channel_id);
    for chunk in announcements.chunks(25) {
        let colour = if chunk.iter().any(|announcement| announcement.is_correction) {
            serenity::Colour::GOLD
        } else {
            serenity::Colour::DARK_GREEN
        };
        let embed = chunk.iter().fold(
            serenity::CreateEmbed::default()
                .title(format!("{} {}", league.emoji(), league.display_name()))
                .colour(colour),
            |embed, announcement| embed.field(&announcement.heading, &announcement.body, false),
        );
        channel
            .send_message(http, serenity::CreateMessage::new().embed(embed))
            .await?;
    }

    Ok(())
}
