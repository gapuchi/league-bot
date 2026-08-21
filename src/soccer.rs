use std::collections::{HashMap, HashSet};

use crate::api::{ApiError, FootballDataApi, Match, Team};

/// Returns full-time goals when the API has populated both sides.
/// football-data.org often marks matches FINISHED before scores are available.
pub fn full_time_score(m: &Match) -> Option<(i64, i64)> {
    match (m.score.full_time.home, m.score.full_time.away) {
        (Some(home), Some(away)) => Some((home, away)),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamRef {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamClassification {
    pub still_in: Vec<TeamRef>,
    pub eliminated: Vec<TeamRef>,
}

const KNOCKOUT_STAGES: &[&str] = &[
    "LAST_32",
    "LAST_16",
    "QUARTER_FINALS",
    "SEMI_FINALS",
    "FINAL",
];

fn match_team_ids(m: &Match) -> impl Iterator<Item = i64> + '_ {
    [m.home_team.id, m.away_team.id].into_iter().flatten()
}

fn is_group_stage(stage: Option<&str>) -> bool {
    stage == Some("GROUP_STAGE")
}

fn is_knockout_stage(stage: Option<&str>) -> bool {
    stage.is_some_and(|stage| KNOCKOUT_STAGES.contains(&stage))
}

fn knockout_loser(m: &Match) -> Option<i64> {
    let (home_goals, away_goals) = full_time_score(m)?;
    let home_id = m.home_team.id?;
    let away_id = m.away_team.id?;
    if home_goals > away_goals {
        Some(away_id)
    } else if away_goals > home_goals {
        Some(home_id)
    } else {
        None
    }
}

const THIRD_PLACE_ADVANCERS: usize = 8;
const GROUP_ADVANCEMENT_PLACES: usize = 3;
const REMAINING_MATCH_OUTCOMES: [(i64, i64); 3] = [(1, 0), (0, 0), (0, 1)];

#[derive(Default, Clone)]
struct GroupRow {
    points: i64,
    goals_for: i64,
    goals_against: i64,
}

impl GroupRow {
    fn goal_difference(&self) -> i64 {
        self.goals_for - self.goals_against
    }
}

fn group_matches<'a>(matches: &'a [Match], group: &str) -> Vec<&'a Match> {
    matches
        .iter()
        .filter(|m| is_group_stage(m.stage.as_deref()))
        .filter(|m| m.group.as_deref() == Some(group))
        .collect()
}

fn match_is_final(m: &Match) -> bool {
    matches!(
        m.status.as_deref(),
        Some("FINISHED") | Some("AWARDED")
    ) && full_time_score(m).is_some()
}

fn group_is_complete(group_matches: &[&Match]) -> bool {
    !group_matches.is_empty() && group_matches.iter().all(|m| match_is_final(m))
}

#[derive(Clone, Copy)]
struct GroupResult {
    home_id: i64,
    away_id: i64,
    home_goals: i64,
    away_goals: i64,
}

fn group_team_ids(group_matches: &[&Match]) -> HashSet<i64> {
    group_matches
        .iter()
        .flat_map(|m| match_team_ids(m))
        .collect()
}

fn results_from_group_matches(group_matches: &[&Match]) -> Vec<GroupResult> {
    group_matches
        .iter()
        .filter(|m| match_is_final(m))
        .filter_map(|m| {
            let (home_goals, away_goals) = full_time_score(m)?;
            Some(GroupResult {
                home_id: m.home_team.id?,
                away_id: m.away_team.id?,
                home_goals,
                away_goals,
            })
        })
        .collect()
}

fn standings_from_results(results: &[GroupResult]) -> HashMap<i64, GroupRow> {
    let mut rows: HashMap<i64, GroupRow> = HashMap::new();

    for result in results {
        rows.entry(result.home_id).or_default();
        rows.entry(result.away_id).or_default();
        apply_match_result(
            &mut rows,
            result.home_id,
            result.away_id,
            result.home_goals,
            result.away_goals,
        );
    }

    rows
}

fn head_to_head_standings(team_ids: &HashSet<i64>, results: &[GroupResult]) -> HashMap<i64, GroupRow> {
    let mini_results: Vec<GroupResult> = results
        .iter()
        .copied()
        .filter(|result| team_ids.contains(&result.home_id) && team_ids.contains(&result.away_id))
        .collect();
    standings_from_results(&mini_results)
}

fn teams_tied_on_points(rows: &HashMap<i64, GroupRow>, points: i64) -> HashSet<i64> {
    rows
        .iter()
        .filter(|(_, row)| row.points == points)
        .map(|(&team_id, _)| team_id)
        .collect()
}

/// WC 2026 group ranking: points, then head-to-head among tied teams, then overall
/// GD/GF. Fair play and FIFA ranking deferred to stable team id.
fn compare_fifa_group_teams(
    id_a: i64,
    id_b: i64,
    rows: &HashMap<i64, GroupRow>,
    results: &[GroupResult],
) -> std::cmp::Ordering {
    let row_a = &rows[&id_a];
    let row_b = &rows[&id_b];

    row_b
        .points
        .cmp(&row_a.points)
        .then_with(|| {
            if row_a.points != row_b.points {
                return std::cmp::Ordering::Equal;
            }

            let tied = teams_tied_on_points(rows, row_a.points);
            let head_to_head = head_to_head_standings(&tied, results);
            let h2h_a = head_to_head.get(&id_a).cloned().unwrap_or_default();
            let h2h_b = head_to_head.get(&id_b).cloned().unwrap_or_default();

            h2h_b
                .points
                .cmp(&h2h_a.points)
                .then_with(|| {
                    h2h_b
                        .goal_difference()
                        .cmp(&h2h_a.goal_difference())
                })
                .then_with(|| h2h_b.goals_for.cmp(&h2h_a.goals_for))
        })
        .then_with(|| row_b.goal_difference().cmp(&row_a.goal_difference()))
        .then_with(|| row_b.goals_for.cmp(&row_a.goals_for))
        .then_with(|| id_a.cmp(&id_b))
}

fn ranked_from_results(results: &[GroupResult]) -> Vec<(i64, GroupRow)> {
    let rows = standings_from_results(results);
    let mut teams: Vec<i64> = rows.keys().copied().collect();
    teams.sort_by(|&id_a, &id_b| compare_fifa_group_teams(id_a, id_b, &rows, results));
    teams
        .into_iter()
        .map(|team_id| (team_id, rows[&team_id].clone()))
        .collect()
}

fn ranked_group_standings(group_matches: &[&Match]) -> Vec<(i64, GroupRow)> {
    ranked_from_results(&results_from_group_matches(group_matches))
}

fn fourth_place_from_group(group_matches: &[&Match]) -> Option<i64> {
    ranked_group_standings(group_matches)
        .last()
        .map(|(team_id, _)| *team_id)
}

fn third_place_from_group(group_matches: &[&Match]) -> Option<(i64, GroupRow)> {
    ranked_group_standings(group_matches)
        .get(2)
        .map(|(team_id, row)| (*team_id, row.clone()))
}

fn compare_records_desc(a: &GroupRow, b: &GroupRow) -> std::cmp::Ordering {
    b.points
        .cmp(&a.points)
        .then_with(|| b.goal_difference().cmp(&a.goal_difference()))
        .then_with(|| b.goals_for.cmp(&a.goals_for))
}

fn apply_match_result(
    rows: &mut HashMap<i64, GroupRow>,
    home_id: i64,
    away_id: i64,
    home_goals: i64,
    away_goals: i64,
) {
    {
        let home = rows.get_mut(&home_id).expect("home team in group");
        home.goals_for += home_goals;
        home.goals_against += away_goals;
        if home_goals > away_goals {
            home.points += 3;
        } else if home_goals == away_goals {
            home.points += 1;
        }
    }

    {
        let away = rows.get_mut(&away_id).expect("away team in group");
        away.goals_for += away_goals;
        away.goals_against += home_goals;
        if away_goals > home_goals {
            away.points += 3;
        } else if home_goals == away_goals {
            away.points += 1;
        }
    }
}

fn pending_group_matches<'a>(group_matches: &'a [&'a Match]) -> Vec<&'a Match> {
    group_matches
        .iter()
        .copied()
        .filter(|m| !match_is_final(m))
        .collect()
}

fn for_each_remaining_group_outcome(
    group_matches: &[&Match],
    mut apply: impl FnMut(&[GroupResult]),
) {
    let finished = results_from_group_matches(group_matches);
    let pending = pending_group_matches(group_matches);
    if pending.is_empty() {
        apply(&finished);
        return;
    }

    fn simulate(
        pending: &[&Match],
        idx: usize,
        results: Vec<GroupResult>,
        apply: &mut impl FnMut(&[GroupResult]),
    ) {
        if idx == pending.len() {
            apply(&results);
            return;
        }

        let m = pending[idx];
        let Some(home_id) = m.home_team.id else {
            return;
        };
        let Some(away_id) = m.away_team.id else {
            return;
        };

        for (home_goals, away_goals) in REMAINING_MATCH_OUTCOMES {
            let mut results = results.clone();
            results.push(GroupResult {
                home_id,
                away_id,
                home_goals,
                away_goals,
            });
            simulate(pending, idx + 1, results, apply);
        }
    }

    simulate(&pending, 0, finished, &mut apply);
}

fn min_third_place_record(group_matches: &[&Match]) -> GroupRow {
    let mut weakest_third = None;

    for_each_remaining_group_outcome(group_matches, |results| {
        let third = ranked_from_results(results)
            .get(2)
            .map(|(_, row)| row.clone())
            .unwrap_or_default();
        let replace = weakest_third.as_ref().is_none_or(|current| {
            compare_records_desc(&third, current) == std::cmp::Ordering::Greater
        });
        if replace {
            weakest_third = Some(third);
        }
    });

    weakest_third.unwrap_or_default()
}

/// Teams that cannot finish in the top three of an incomplete group.
fn mathematically_eliminated_from_group(group_matches: &[&Match]) -> Vec<i64> {
    if group_is_complete(group_matches) {
        return Vec::new();
    }

    let all_teams = group_team_ids(group_matches);
    let mut can_advance = HashSet::new();

    for_each_remaining_group_outcome(group_matches, |results| {
        for (team_id, _) in ranked_from_results(results)
            .into_iter()
            .take(GROUP_ADVANCEMENT_PLACES)
        {
            can_advance.insert(team_id);
        }
    });

    all_teams
        .into_iter()
        .filter(|team_id| !can_advance.contains(team_id))
        .collect()
}

fn compare_third_place_candidate(
    a_record: &GroupRow,
    a_id: Option<i64>,
    b_record: &GroupRow,
    b_id: Option<i64>,
) -> std::cmp::Ordering {
    compare_records_desc(a_record, b_record).then_with(|| match (a_id, b_id) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    })
}

fn best_case_third_place_rank(
    team_id: i64,
    confirmed: &[(i64, GroupRow)],
    min_thirds: &[GroupRow],
) -> usize {
    let mut pool: Vec<(Option<i64>, GroupRow)> = confirmed
        .iter()
        .map(|(id, row)| (Some(*id), row.clone()))
        .chain(min_thirds.iter().cloned().map(|row| (None, row)))
        .collect();

    pool.sort_by(|(id_a, a), (id_b, b)| {
        compare_third_place_candidate(a, *id_a, b, *id_b)
    });

    pool.iter()
        .position(|(id, _)| id == &Some(team_id))
        .map(|idx| idx + 1)
        .unwrap_or(usize::MAX)
}

/// Third-place teams locked out of the top eight even if every remaining group
/// produces the weakest possible third-place finisher.
fn early_eliminated_third_place_team_ids(
    confirmed: &[(i64, GroupRow)],
    incomplete_groups: &[Vec<&Match>],
) -> Vec<i64> {
    let total_slots = confirmed.len() + incomplete_groups.len();
    if total_slots <= THIRD_PLACE_ADVANCERS {
        return Vec::new();
    }

    let min_thirds: Vec<GroupRow> = incomplete_groups
        .iter()
        .map(|group_matches| min_third_place_record(group_matches))
        .collect();

    confirmed
        .iter()
        .filter_map(|&(team_id, _)| {
            let rank = best_case_third_place_rank(team_id, confirmed, &min_thirds);
            (rank > THIRD_PLACE_ADVANCERS).then_some(team_id)
        })
        .collect()
}

/// Classifies every competition team as still in the tournament or eliminated.
pub fn classify_teams(teams: &[Team], matches: &[Match]) -> TeamClassification {
    let mut eliminated_ids = HashSet::new();
    for m in matches {
        if m.status.as_deref() != Some("FINISHED") {
            continue;
        }

        let stage = m.stage.as_deref();
        if is_knockout_stage(stage) {
            if let Some(loser_id) = knockout_loser(m) {
                eliminated_ids.insert(loser_id);
            }
        } else if stage == Some("THIRD_PLACE") {
            for team_id in match_team_ids(m) {
                eliminated_ids.insert(team_id);
            }
        }
    }

    let mut groups = HashSet::new();
    for m in matches {
        if is_group_stage(m.stage.as_deref()) && let Some(group) = &m.group {
            groups.insert(group.clone());
        }
    }

    let mut third_place_candidates = Vec::new();
    let mut incomplete_group_matches = Vec::new();

    for group in &groups {
        let group_matches = group_matches(matches, group);
        if !group_is_complete(&group_matches) {
            incomplete_group_matches.push(group_matches.clone());
            for team_id in mathematically_eliminated_from_group(&group_matches) {
                eliminated_ids.insert(team_id);
            }
            continue;
        }

        if let Some(fourth_place) = fourth_place_from_group(&group_matches) {
            eliminated_ids.insert(fourth_place);
        }

        if let Some(third_place) = third_place_from_group(&group_matches) {
            third_place_candidates.push(third_place);
        }
    }

    for team_id in early_eliminated_third_place_team_ids(
        &third_place_candidates,
        &incomplete_group_matches,
    ) {
        eliminated_ids.insert(team_id);
    }

    let mut still_in = Vec::new();
    let mut eliminated = Vec::new();

    for team in teams {
        let team_ref = TeamRef {
            id: team.id,
            name: team.name.clone(),
        };
        if eliminated_ids.contains(&team.id) {
            eliminated.push(team_ref);
        } else {
            still_in.push(team_ref);
        }
    }

    still_in.sort_by(|a, b| a.name.cmp(&b.name));
    eliminated.sort_by(|a, b| a.name.cmp(&b.name));

    TeamClassification {
        still_in,
        eliminated,
    }
}

#[derive(Debug, Clone)]
pub struct SquadPlayerMatch {
    pub player_id: i64,
    pub player_name: String,
    pub team_id: i64,
    pub team_name: String,
}

pub async fn fetch_squads_for_teams(
    api: &FootballDataApi,
    teams: &[(i64, String)],
) -> Result<Vec<SquadPlayerMatch>, ApiError> {
    let mut players = Vec::new();
    for (team_id, team_name) in teams {
        let squad = api.fetch_team_squad(*team_id).await?;
        players.extend(squad.into_iter().filter_map(|player| {
            let is_player = player
                .role
                .as_ref()
                .is_none_or(|role| role == "PLAYER");
            is_player.then(|| SquadPlayerMatch {
                player_id: player.id,
                player_name: player.name,
                team_id: *team_id,
                team_name: team_name.clone(),
            })
        }));
    }
    Ok(players)
}

pub fn find_players<'a>(
    players: &'a [SquadPlayerMatch],
    query: &str,
) -> Vec<&'a SquadPlayerMatch> {
    let query = query.trim().to_lowercase();
    players
        .iter()
        .filter(|player| {
            let name = player.player_name.to_lowercase();
            name == query || name.contains(&query)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{Match, MatchTeam, Score, ScoreDetail};

    fn row(points: i64, goals_for: i64, goals_against: i64) -> GroupRow {
        GroupRow {
            points,
            goals_for,
            goals_against,
        }
    }

    fn match_with(
        id: i64,
        home_id: Option<i64>,
        away_id: Option<i64>,
        status: &str,
        group: Option<&str>,
        score: (Option<i64>, Option<i64>),
    ) -> Match {
        Match {
            id,
            home_team: MatchTeam {
                id: home_id,
                name: home_id.map(|id| format!("Home {id}")),
                short_name: None,
                tla: None,
            },
            away_team: MatchTeam {
                id: away_id,
                name: away_id.map(|id| format!("Away {id}")),
                short_name: None,
                tla: None,
            },
            score: Score {
                full_time: ScoreDetail {
                    home: score.0,
                    away: score.1,
                },
            },
            status: Some(status.into()),
            stage: Some("GROUP_STAGE".into()),
            group: group.map(str::to_string),
            matchday: None,
        }
    }

    #[test]
    fn in_play_score_does_not_count_toward_standings() {
        let group = [
            match_with(1, Some(1), Some(4), "FINISHED", Some("GROUP_E"), (Some(1), Some(0))),
            match_with(2, Some(4), Some(3), "FINISHED", Some("GROUP_E"), (Some(0), Some(0))),
            match_with(3, Some(1), Some(2), "FINISHED", Some("GROUP_E"), (Some(2), Some(0))),
            match_with(4, Some(2), Some(3), "FINISHED", Some("GROUP_E"), (Some(2), Some(0))),
            // Live 0-0 must not lock in a draw while Ecuador can still win.
            match_with(5, Some(4), Some(1), "IN_PLAY", Some("GROUP_E"), (Some(0), Some(0))),
            match_with(6, Some(3), Some(2), "TIMED", Some("GROUP_E"), (None, None)),
        ];
        let group_matches: Vec<_> = group.iter().collect();

        assert_eq!(results_from_group_matches(&group_matches).len(), 4);
        assert!(!mathematically_eliminated_from_group(&group_matches).contains(&4));
    }

    fn knockout_match(id: i64, home_id: i64, away_id: i64, score: (i64, i64)) -> Match {
        Match {
            id,
            home_team: MatchTeam {
                id: Some(home_id),
                name: Some(format!("Home {home_id}")),
                short_name: None,
                tla: None,
            },
            away_team: MatchTeam {
                id: Some(away_id),
                name: Some(format!("Away {away_id}")),
                short_name: None,
                tla: None,
            },
            score: Score {
                full_time: ScoreDetail {
                    home: Some(score.0),
                    away: Some(score.1),
                },
            },
            status: Some("FINISHED".into()),
            stage: Some("LAST_16".into()),
            group: None,
            matchday: None,
        }
    }

    fn team(id: i64, name: &str) -> Team {
        Team {
            id,
            name: name.into(),
            short_name: None,
            tla: None,
        }
    }

    fn classification_ids(classification: &TeamClassification) -> (HashSet<i64>, HashSet<i64>) {
        (
            classification.still_in.iter().map(|t| t.id).collect(),
            classification.eliminated.iter().map(|t| t.id).collect(),
        )
    }

    fn locked_out_bottom_team_group_a() -> Vec<Match> {
        vec![
            match_with(1, Some(1), Some(4), "FINISHED", Some("GROUP_A"), (Some(2), Some(0))),
            match_with(2, Some(2), Some(4), "FINISHED", Some("GROUP_A"), (Some(2), Some(0))),
            match_with(3, Some(3), Some(4), "FINISHED", Some("GROUP_A"), (Some(2), Some(0))),
            match_with(4, Some(1), Some(2), "FINISHED", Some("GROUP_A"), (Some(2), Some(0))),
            match_with(5, Some(1), Some(3), "FINISHED", Some("GROUP_A"), (Some(2), Some(0))),
            match_with(6, Some(2), Some(3), "TIMED", Some("GROUP_A"), (None, None)),
        ]
    }

    fn complete_group_a_top_three_in() -> Vec<Match> {
        vec![
            match_with(1, Some(1), Some(2), "FINISHED", Some("GROUP_A"), (Some(2), Some(0))),
            match_with(2, Some(1), Some(3), "FINISHED", Some("GROUP_A"), (Some(2), Some(0))),
            match_with(3, Some(1), Some(4), "FINISHED", Some("GROUP_A"), (Some(2), Some(0))),
            match_with(4, Some(2), Some(3), "FINISHED", Some("GROUP_A"), (Some(2), Some(0))),
            match_with(5, Some(2), Some(4), "FINISHED", Some("GROUP_A"), (Some(2), Some(0))),
            match_with(6, Some(3), Some(4), "FINISHED", Some("GROUP_A"), (Some(1), Some(0))),
        ]
    }

    fn complete_group_refs(group: &[Match]) -> Vec<&Match> {
        group.iter().collect()
    }

    fn unstarted_group_a() -> Vec<Match> {
        vec![
            match_with(1, Some(1), Some(2), "TIMED", Some("GROUP_A"), (None, None)),
            match_with(2, Some(1), Some(3), "TIMED", Some("GROUP_A"), (None, None)),
            match_with(3, Some(1), Some(4), "TIMED", Some("GROUP_A"), (None, None)),
            match_with(4, Some(2), Some(3), "TIMED", Some("GROUP_A"), (None, None)),
            match_with(5, Some(2), Some(4), "TIMED", Some("GROUP_A"), (None, None)),
            match_with(6, Some(3), Some(4), "TIMED", Some("GROUP_A"), (None, None)),
        ]
    }

    // --- fourth place / complete group ---

    #[test]
    fn complete_group_eliminates_only_fourth_place() {
        let group = complete_group_a_top_three_in();
        let group_refs = complete_group_refs(&group);

        assert_eq!(fourth_place_from_group(&group_refs), Some(4));
        assert_eq!(third_place_from_group(&group_refs).map(|(id, _)| id), Some(3));
    }

    #[test]
    fn mathematically_eliminated_empty_for_complete_group() {
        let group = complete_group_a_top_three_in();
        assert!(mathematically_eliminated_from_group(&complete_group_refs(&group)).is_empty());
    }

    // --- mathematical elimination (incomplete group) ---

    #[test]
    fn mathematically_eliminated_when_cannot_reach_top_three() {
        let group = locked_out_bottom_team_group_a();
        let group_matches = complete_group_refs(&group);

        let mut eliminated = mathematically_eliminated_from_group(&group_matches);
        eliminated.sort_unstable();
        assert_eq!(eliminated, vec![4]);
    }

    #[test]
    fn nobody_mathematically_eliminated_before_group_starts() {
        let group = unstarted_group_a();
        assert!(mathematically_eliminated_from_group(&complete_group_refs(&group)).is_empty());
    }

    #[test]
    fn contender_with_two_matches_left_not_mathematically_eliminated() {
        // After two rounds: 1 leads, 2/3/4 tight; two fixtures remain so 4 can still reach 3rd.
        let partial = [
            match_with(1, Some(1), Some(2), "FINISHED", Some("GROUP_A"), (Some(1), Some(0))),
            match_with(2, Some(3), Some(4), "FINISHED", Some("GROUP_A"), (Some(1), Some(0))),
            match_with(3, Some(1), Some(3), "FINISHED", Some("GROUP_A"), (Some(1), Some(0))),
            match_with(4, Some(2), Some(4), "TIMED", Some("GROUP_A"), (None, None)),
            match_with(5, Some(1), Some(4), "TIMED", Some("GROUP_A"), (None, None)),
            match_with(6, Some(2), Some(3), "TIMED", Some("GROUP_A"), (None, None)),
        ];
        let group_matches: Vec<_> = partial.iter().collect();

        assert!(!mathematically_eliminated_from_group(&group_matches).contains(&4));
    }

    #[test]
    fn turkey_shaped_group_eliminated_on_head_to_head() {
        let group = [
            match_with(1, Some(1), Some(3), "FINISHED", Some("GROUP_D"), (Some(4), Some(1))),
            match_with(2, Some(2), Some(4), "FINISHED", Some("GROUP_D"), (Some(2), Some(0))),
            match_with(3, Some(4), Some(3), "FINISHED", Some("GROUP_D"), (Some(0), Some(1))),
            match_with(4, Some(1), Some(2), "FINISHED", Some("GROUP_D"), (Some(2), Some(0))),
            match_with(5, Some(4), Some(1), "TIMED", Some("GROUP_D"), (None, None)),
            match_with(6, Some(3), Some(2), "TIMED", Some("GROUP_D"), (None, None)),
        ];
        let group_matches: Vec<_> = group.iter().collect();

        let mut eliminated = mathematically_eliminated_from_group(&group_matches);
        eliminated.sort_unstable();
        assert_eq!(eliminated, vec![4]);
    }

    #[test]
    fn finished_without_score_simulated_like_timed() {
        let timed = locked_out_bottom_team_group_a();
        let mut finished_no_score = locked_out_bottom_team_group_a();
        finished_no_score[5] = match_with(
            6,
            Some(2),
            Some(3),
            "FINISHED",
            Some("GROUP_A"),
            (None, None),
        );

        assert_eq!(
            mathematically_eliminated_from_group(&complete_group_refs(&timed)),
            mathematically_eliminated_from_group(&complete_group_refs(&finished_no_score)),
        );
    }

    #[test]
    fn min_third_from_unstarted_group_is_weak() {
        let group = unstarted_group_a();
        let group_refs = complete_group_refs(&group);
        let min_third = min_third_place_record(&group_refs);

        assert!(min_third.points <= 1, "weakest third should be at most one point");
    }

    // --- third-place table / early cut ---

    #[test]
    fn third_place_cut_drops_bottom_four_of_twelve() {
        let candidates: Vec<(i64, GroupRow)> = (1..=12)
            .map(|team_id| (team_id, row(13 - team_id, 5, 4)))
            .collect();

        let eliminated = early_eliminated_third_place_team_ids(&candidates, &[]);
        assert_eq!(eliminated, vec![9, 10, 11, 12]);
    }

    #[test]
    fn third_place_cut_spared_when_eight_or_fewer_candidates() {
        let candidates = vec![(1, row(4, 3, 2)), (2, row(3, 2, 2))];
        assert!(early_eliminated_third_place_team_ids(&candidates, &[]).is_empty());
    }

    #[test]
    fn third_place_eliminated_early_when_ninth_with_one_group_left() {
        let candidates: Vec<(i64, GroupRow)> = (1..=11)
            .map(|team_id| (team_id, row(12 - team_id, 5, 4)))
            .collect();

        let incomplete_group = [
            match_with(
                1,
                Some(100),
                Some(101),
                "TIMED",
                Some("GROUP_L"),
                (None, None),
            ),
            match_with(
                2,
                Some(102),
                Some(103),
                "TIMED",
                Some("GROUP_L"),
                (None, None),
            ),
        ];
        let incomplete = vec![incomplete_group.iter().collect::<Vec<_>>()];

        let eliminated = early_eliminated_third_place_team_ids(&candidates, &incomplete);
        assert_eq!(eliminated, vec![9, 10, 11]);
    }

    #[test]
    fn eighth_third_place_stays_in_with_one_group_remaining() {
        let candidates: Vec<(i64, GroupRow)> = (1..=11)
            .map(|team_id| (team_id, row(12 - team_id, 5, 4)))
            .collect();

        let incomplete_group = [
            match_with(1, Some(100), Some(101), "TIMED", Some("GROUP_L"), (None, None)),
        ];
        let incomplete = vec![complete_group_refs(&incomplete_group)];

        let eliminated = early_eliminated_third_place_team_ids(&candidates, &incomplete);
        assert!(!eliminated.contains(&8));
        assert!(eliminated.contains(&9));
    }

    #[test]
    fn third_place_tiebreak_uses_team_id() {
        let candidates = vec![
            (1, row(4, 3, 1)),
            (2, row(4, 3, 1)),
        ];

        assert_eq!(best_case_third_place_rank(1, &candidates, &[]), 1);
        assert_eq!(best_case_third_place_rank(2, &candidates, &[]), 2);
    }

    #[test]
    fn phantom_min_third_loses_tie_to_confirmed_team() {
        let candidates = vec![(5, row(3, 2, 2))];
        let min_thirds = vec![row(3, 2, 2)];

        assert_eq!(best_case_third_place_rank(5, &candidates, &min_thirds), 1);
    }

    // --- classify_teams integration ---

    #[test]
    fn classify_mathematically_eliminated_overrides_upcoming_fixture() {
        let teams = vec![
            team(1, "A"),
            team(2, "B"),
            team(3, "C"),
            team(4, "D"),
        ];
        let matches = locked_out_bottom_team_group_a();
        let (_, eliminated) = classification_ids(&classify_teams(&teams, &matches));

        assert!(eliminated.contains(&4));
        assert!(!eliminated.contains(&1));
    }

    #[test]
    fn classify_complete_group_keeps_third_in() {
        let teams = vec![team(1, "A"), team(2, "B"), team(3, "C"), team(4, "D")];
        let matches = complete_group_a_top_three_in();
        let (still_in, eliminated) = classification_ids(&classify_teams(&teams, &matches));

        assert_eq!(still_in, HashSet::from([1, 2, 3]));
        assert_eq!(eliminated, HashSet::from([4]));
    }

    #[test]
    fn classify_knockout_loser_out_winner_in() {
        let teams = vec![team(10, "Winner"), team(20, "Loser")];
        let matches = vec![knockout_match(1, 10, 20, (2, 1))];
        let (still_in, eliminated) = classification_ids(&classify_teams(&teams, &matches));

        assert_eq!(still_in, HashSet::from([10]));
        assert_eq!(eliminated, HashSet::from([20]));
    }

    #[test]
    fn classify_knockout_draw_eliminates_neither() {
        let teams = vec![team(10, "Home"), team(20, "Away")];
        let matches = vec![knockout_match(1, 10, 20, (1, 1))];
        let (still_in, eliminated) = classification_ids(&classify_teams(&teams, &matches));

        assert_eq!(still_in, HashSet::from([10, 20]));
        assert!(eliminated.is_empty());
    }

    #[test]
    fn classify_third_place_playoff_eliminates_both() {
        let teams = vec![team(1, "France"), team(2, "Portugal")];
        let matches = vec![Match {
            id: 1,
            home_team: MatchTeam {
                id: Some(1),
                name: Some("France".into()),
                short_name: None,
                tla: None,
            },
            away_team: MatchTeam {
                id: Some(2),
                name: Some("Portugal".into()),
                short_name: None,
                tla: None,
            },
            score: Score {
                full_time: ScoreDetail {
                    home: Some(2),
                    away: Some(1),
                },
            },
            status: Some("FINISHED".into()),
            stage: Some("THIRD_PLACE".into()),
            group: None,
            matchday: None,
        }];
        let (still_in, eliminated) = classification_ids(&classify_teams(&teams, &matches));

        assert!(still_in.is_empty());
        assert_eq!(eliminated, HashSet::from([1, 2]));
    }

    #[test]
    fn classify_partitions_teams_without_overlap() {
        let teams: Vec<_> = (1..=5).map(|id| team(id, &format!("T{id}"))).collect();
        let matches = locked_out_bottom_team_group_a();
        let classification = classify_teams(&teams, &matches);
        let (still_in, eliminated) = classification_ids(&classification);

        assert_eq!(still_in.len() + eliminated.len(), teams.len());
        assert!(still_in.is_disjoint(&eliminated));
    }

    #[test]
    fn classify_all_twelve_third_place_cuts_bottom_four() {
        let mut teams = Vec::new();
        let mut matches = Vec::new();
        let mut match_id = 1_i64;

        for group_idx in 0..12 {
            let base = group_idx * 4 + 1;
            let group = format!("GROUP_{group_idx}");
            for offset in 0..4 {
                teams.push(team(base + offset, &format!("G{group_idx}T{offset}")));
            }

            let round_robin = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
            for (home, away) in round_robin {
                let home_id = base + home;
                let away_id = base + away;
                let (home_goals, away_goals) = if home == 0 {
                    (2, 0)
                } else if home == 1 && away == 2 {
                    (2, 0)
                } else if home == 2 && away == 3 {
                    (1, 0)
                } else {
                    (2, 0)
                };
                matches.push(match_with(
                    match_id,
                    Some(home_id),
                    Some(away_id),
                    "FINISHED",
                    Some(group.as_str()),
                    (Some(home_goals), Some(away_goals)),
                ));
                match_id += 1;
            }
        }

        let (_, eliminated) = classification_ids(&classify_teams(&teams, &matches));
        let mut third_place_ids: Vec<i64> = (0..12)
            .map(|group_idx| group_idx * 4 + 3)
            .collect();
        third_place_ids.sort_unstable();

        for third_place_id in third_place_ids.iter().take(8) {
            assert!(!eliminated.contains(third_place_id));
        }
        for third_place_id in third_place_ids.iter().skip(8) {
            assert!(eliminated.contains(third_place_id));
        }
    }
}
