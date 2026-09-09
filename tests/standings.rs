use league_bot::standings::{StandingRow, standings_ranks};

fn standing_row(points: f64) -> StandingRow {
    StandingRow {
        user_id: 0,
        points,
        teams: vec![],
        tiebreaker_value: 0,
        tiebreaker_player: None,
    }
}

#[test]
fn standings_ranks_tied_points_share_rank() {
    let rows = vec![
        standing_row(1.5),
        standing_row(1.5),
        standing_row(0.0),
        standing_row(0.0),
        standing_row(0.0),
        standing_row(0.0),
    ];

    assert_eq!(standings_ranks(&rows), vec![1, 1, 3, 3, 3, 3]);
}

#[test]
fn standings_footer_uses_league_scoring_and_labels() {
    use league_bot::scoring::{NFL, SOCCER};
    use league_bot::standings::standings_footer;

    assert_eq!(
        standings_footer(SOCCER, "draw", Some("goals")),
        "Win 3 · Draw 1 · Loss 0 · TB = tie-breaker goals"
    );
    assert_eq!(
        standings_footer(NFL, "tie", None),
        "Win 1 · Tie 0.5 · Loss 0 · Playoff win 3"
    );
}
