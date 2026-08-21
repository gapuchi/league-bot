use league_bot::{
    api::{Match, MatchTeam, Score, ScoreDetail},
    soccer::full_time_score,
};

fn sample_match(home: Option<i64>, away: Option<i64>) -> Match {
    Match {
        id: 1,
        home_team: MatchTeam {
            id: Some(769),
            name: Some("Mexico".into()),
            short_name: None,
            tla: None,
        },
        away_team: MatchTeam {
            id: Some(774),
            name: Some("South Africa".into()),
            short_name: None,
            tla: None,
        },
        score: Score {
            full_time: ScoreDetail { home, away },
        },
        status: Some("FINISHED".into()),
        stage: Some("GROUP_STAGE".into()),
        group: Some("GROUP_A".into()),
        matchday: None,
    }
}

#[test]
fn full_time_score_requires_both_sides() {
    assert_eq!(full_time_score(&sample_match(None, None)), None);
    assert_eq!(full_time_score(&sample_match(Some(2), None)), None);
    assert_eq!(full_time_score(&sample_match(None, Some(0))), None);
    assert_eq!(full_time_score(&sample_match(Some(2), Some(0))), Some((2, 0)));
}
