/// Points a league awards for the three match outcomes. Fractional points are allowed
/// (e.g. NFL's half-point tie), so values are `f64`; every league uses multiples of 0.5.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScoringRules {
    pub win: f64,
    pub draw: f64,
    pub loss: f64,
}

/// World Cup and Premier League: standard 3 / 1 / 0.
pub const SOCCER: ScoringRules = ScoringRules {
    win: 3.0,
    draw: 1.0,
    loss: 0.0,
};

/// NFL: 1 for a win, half a point for a tie, nothing for a loss.
pub const NFL: ScoringRules = ScoringRules {
    win: 1.0,
    draw: 0.5,
    loss: 0.0,
};

pub struct FinishedMatch {
    pub home_team_id: i64,
    pub away_team_id: i64,
    pub home_goals: i64,
    pub away_goals: i64,
}

pub fn points_for_result(rules: ScoringRules, team_goals: i64, opponent_goals: i64) -> f64 {
    if team_goals > opponent_goals {
        rules.win
    } else if team_goals == opponent_goals {
        rules.draw
    } else {
        rules.loss
    }
}

pub fn points_for_team_in_match(rules: ScoringRules, team_id: i64, m: &FinishedMatch) -> f64 {
    if team_id == m.home_team_id {
        points_for_result(rules, m.home_goals, m.away_goals)
    } else if team_id == m.away_team_id {
        points_for_result(rules, m.away_goals, m.home_goals)
    } else {
        0.0
    }
}

pub fn points_for_team(rules: ScoringRules, team_id: i64, matches: &[FinishedMatch]) -> f64 {
    matches
        .iter()
        .map(|m| points_for_team_in_match(rules, team_id, m))
        .sum()
}

pub fn points_for_teams(rules: ScoringRules, team_ids: &[i64], matches: &[FinishedMatch]) -> f64 {
    team_ids
        .iter()
        .flat_map(|team_id| {
            matches
                .iter()
                .map(|m| points_for_team_in_match(rules, *team_id, m))
        })
        .sum()
}
