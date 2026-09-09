use crate::db::Registration;
use crate::scoring::{self, FinishedMatch, ScoringRules};
use std::collections::HashMap;

/// League-agnostic standings row returned across the `League` seam.
pub struct StandingRow {
    pub user_id: u64,
    pub points: f64,
    pub teams: Vec<(String, f64)>,
    /// Tie-breaker stat total (e.g. goals) — see `League::tiebreaker_unit`; 0 when the
    /// league has none.
    pub tiebreaker_value: i64,
    pub tiebreaker_player: Option<String>,
}

/// `draw_label` / `tiebreaker_unit` come from the focused `League` (e.g. `draw`/`goals`);
/// `rules` supply the per-league points shown in the footer.
pub fn standings_footer(
    rules: ScoringRules,
    draw_label: &str,
    tiebreaker_unit: Option<&str>,
) -> String {
    let draw = capitalize_first(draw_label);
    let mut footer = format!(
        "Win {} · {draw} {} · Loss {}",
        rules.win, rules.draw, rules.loss
    );
    if let Some(playoff_win) = rules.playoff_win {
        footer.push_str(&format!(" · Playoff win {playoff_win}"));
    }
    if let Some(unit) = tiebreaker_unit {
        footer.push_str(&format!(" · TB = tie-breaker {unit}"));
    }
    footer
}

fn capitalize_first(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Build ranked standings from finished matches and registrations.
///
/// `tiebreaker_for` returns `(goals, player_name)` for each user.
pub fn build_rows(
    rules: ScoringRules,
    matches: &[FinishedMatch],
    registrations: &[Registration],
    mut tiebreaker_for: impl FnMut(u64) -> rusqlite::Result<(i64, Option<String>)>,
) -> rusqlite::Result<Vec<StandingRow>> {
    let by_user = registrations.iter().fold(
        HashMap::<u64, (Vec<i64>, Vec<String>)>::new(),
        |mut map, registration| {
            let entry = map.entry(registration.user_id).or_default();
            entry.0.push(registration.team_id);
            entry.1.push(registration.team_name.clone());
            map
        },
    );

    let mut rows = by_user
        .into_iter()
        .map(|(user_id, (team_ids, team_names))| {
            let (tiebreaker_value, tiebreaker_player) = tiebreaker_for(user_id)?;
            let mut teams: Vec<(String, f64)> = team_ids
                .iter()
                .zip(&team_names)
                .map(|(team_id, team_name)| {
                    (
                        team_name.clone(),
                        scoring::points_for_team(rules, *team_id, matches),
                    )
                })
                .collect();
            teams.sort_by(|a, b| a.0.cmp(&b.0));
            Ok(StandingRow {
                user_id,
                points: scoring::points_for_teams(rules, &team_ids, matches),
                teams,
                tiebreaker_value,
                tiebreaker_player,
            })
        })
        .collect::<rusqlite::Result<Vec<_>>>()?;

    rows.sort_by(|a, b| {
        b.points
            .total_cmp(&a.points)
            .then_with(|| b.tiebreaker_value.cmp(&a.tiebreaker_value))
            .then_with(|| a.user_id.cmp(&b.user_id))
    });
    Ok(rows)
}

pub fn points_for_user_teams(
    rules: ScoringRules,
    matches: &[FinishedMatch],
    registrations: &[Registration],
) -> f64 {
    let team_ids: Vec<i64> = registrations.iter().map(|r| r.team_id).collect();
    scoring::points_for_teams(rules, &team_ids, matches)
}

pub fn format_standing_summary(rank: usize, row: &StandingRow) -> String {
    format!(
        "**{rank}** · <@{}> — **{}** pts",
        row.user_id, row.points,
    )
}

pub fn format_standing_detail(
    rank: usize,
    row: &StandingRow,
    tiebreaker_unit: Option<&str>,
) -> String {
    let team_lines = row.teams.iter().map(|(team_name, points)| {
        format!("\n   • **{team_name}** — {points} pts")
    });
    let tb_line = match (tiebreaker_unit, &row.tiebreaker_player) {
        (None, _) => String::new(),
        (Some(unit), Some(player)) => format!(
            "\n   • Tie-breaker: **{player}** — {} {unit}",
            row.tiebreaker_value
        ),
        (Some(unit), None) => format!("\n   • Tie-breaker — {} {unit}", row.tiebreaker_value),
    };
    format_standing_summary(rank, row)
        + &team_lines.collect::<String>()
        + &tb_line
}

pub fn format_standings_summary_lines(rows: &[StandingRow], ranks: &[usize]) -> Vec<String> {
    rows.iter()
        .zip(ranks)
        .map(|(row, rank)| format_standing_summary(*rank, row))
        .collect()
}

pub fn format_standings_detail_lines(
    rows: &[StandingRow],
    ranks: &[usize],
    tiebreaker_unit: Option<&str>,
) -> Vec<String> {
    rows.iter()
        .zip(ranks)
        .map(|(row, rank)| format_standing_detail(*rank, row, tiebreaker_unit))
        .collect()
}

pub fn standings_ranks(rows: &[StandingRow]) -> Vec<usize> {
    let mut ranks = Vec::with_capacity(rows.len());
    let mut i = 0;
    while i < rows.len() {
        let mut j = i;
        while j + 1 < rows.len()
            && rows[j].points.total_cmp(&rows[j + 1].points) == std::cmp::Ordering::Equal
        {
            j += 1;
        }
        let rank = i + 1;
        for _ in i..=j {
            ranks.push(rank);
        }
        i = j + 1;
    }
    ranks
}
