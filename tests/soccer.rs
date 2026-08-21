use league_bot::{
    api::{Match, MatchTeam, Score, ScoreDetail, Team},
    soccer::classify_teams,
};

fn team(id: i64, name: &str) -> Team {
    Team {
        id,
        name: name.into(),
        short_name: None,
        tla: None,
    }
}

fn match_team(id: i64, name: &str) -> MatchTeam {
    MatchTeam {
        id: Some(id),
        name: Some(name.into()),
        short_name: None,
        tla: None,
    }
}

fn match_with(
    id: i64,
    home: &MatchTeam,
    away: &MatchTeam,
    status: &str,
    stage: &str,
    group: Option<&str>,
    score: (Option<i64>, Option<i64>),
) -> Match {
    Match {
        id,
        home_team: home.clone(),
        away_team: away.clone(),
        score: Score {
            full_time: ScoreDetail {
                home: score.0,
                away: score.1,
            },
        },
        status: Some(status.into()),
        stage: Some(stage.into()),
        group: group.map(str::to_string),
        matchday: None,
    }
}

fn ids(classification: &league_bot::soccer::TeamClassification) -> (Vec<i64>, Vec<i64>) {
    (
        classification
            .still_in
            .iter()
            .map(|team| team.id)
            .collect(),
        classification
            .eliminated
            .iter()
            .map(|team| team.id)
            .collect(),
    )
}

#[test]
fn pre_tournament_teams_all_still_in() {
    let teams = vec![team(1, "Brazil"), team(2, "France"), team(3, "Japan")];
    let classification = classify_teams(&teams, &[]);

    assert_eq!(classification.still_in.len(), 3);
    assert!(classification.eliminated.is_empty());
}

#[test]
fn knockout_loser_is_eliminated() {
    let mexico = match_team(769, "Mexico");
    let south_africa = match_team(774, "South Africa");
    let teams = vec![team(769, "Mexico"), team(774, "South Africa")];
    let matches = vec![match_with(
        1,
        &mexico,
        &south_africa,
        "FINISHED",
        "LAST_16",
        None,
        (Some(2), Some(1)),
    )];

    let (still_in, eliminated) = ids(&classify_teams(&teams, &matches));
    assert_eq!(still_in, vec![769]);
    assert_eq!(eliminated, vec![774]);
}

#[test]
fn upcoming_match_keeps_team_in_during_group_stage() {
    let brazil = match_team(1, "Brazil");
    let serbia = match_team(2, "Serbia");
    let teams = vec![team(1, "Brazil"), team(2, "Serbia")];
    let matches = vec![match_with(
        1,
        &brazil,
        &serbia,
        "TIMED",
        "GROUP_STAGE",
        Some("GROUP_D"),
        (None, None),
    )];

    let (still_in, eliminated) = ids(&classify_teams(&teams, &matches));
    assert_eq!(still_in, vec![1, 2]);
    assert!(eliminated.is_empty());
}

#[test]
fn completed_group_eliminates_fourth_place_only() {
    let team_a = match_team(1, "Team A");
    let team_b = match_team(2, "Team B");
    let team_c = match_team(3, "Team C");
    let team_d = match_team(4, "Team D");
    let teams = vec![
        team(1, "Team A"),
        team(2, "Team B"),
        team(3, "Team C"),
        team(4, "Team D"),
    ];

    let matches = vec![
        match_with(1, &team_a, &team_b, "FINISHED", "GROUP_STAGE", Some("GROUP_A"), (Some(2), Some(0))),
        match_with(2, &team_a, &team_c, "FINISHED", "GROUP_STAGE", Some("GROUP_A"), (Some(2), Some(0))),
        match_with(3, &team_a, &team_d, "FINISHED", "GROUP_STAGE", Some("GROUP_A"), (Some(2), Some(0))),
        match_with(4, &team_b, &team_c, "FINISHED", "GROUP_STAGE", Some("GROUP_A"), (Some(2), Some(0))),
        match_with(5, &team_b, &team_d, "FINISHED", "GROUP_STAGE", Some("GROUP_A"), (Some(2), Some(0))),
        match_with(6, &team_c, &team_d, "FINISHED", "GROUP_STAGE", Some("GROUP_A"), (Some(1), Some(0))),
    ];

    let (still_in, eliminated) = ids(&classify_teams(&teams, &matches));

    assert_eq!(still_in, vec![1, 2, 3]);
    assert_eq!(eliminated, vec![4]);
}

#[test]
fn third_place_stays_in_until_all_groups_finish() {
    // Group A: Korea 3rd, Czech 4th; Group B: Bosnia 3rd, Qatar 4th; Group C in progress
    let matches = vec![
        match_with(1, &match_team(1, "Mexico"), &match_team(2, "South Africa"), "FINISHED", "GROUP_STAGE", Some("GROUP_A"), (Some(2), Some(0))),
        match_with(2, &match_team(1, "Mexico"), &match_team(3, "South Korea"), "FINISHED", "GROUP_STAGE", Some("GROUP_A"), (Some(2), Some(0))),
        match_with(3, &match_team(1, "Mexico"), &match_team(4, "Czechia"), "FINISHED", "GROUP_STAGE", Some("GROUP_A"), (Some(2), Some(0))),
        match_with(4, &match_team(2, "South Africa"), &match_team(3, "South Korea"), "FINISHED", "GROUP_STAGE", Some("GROUP_A"), (Some(1), Some(0))),
        match_with(5, &match_team(2, "South Africa"), &match_team(4, "Czechia"), "FINISHED", "GROUP_STAGE", Some("GROUP_A"), (Some(1), Some(0))),
        match_with(6, &match_team(3, "South Korea"), &match_team(4, "Czechia"), "FINISHED", "GROUP_STAGE", Some("GROUP_A"), (Some(1), Some(0))),
        match_with(7, &match_team(5, "Switzerland"), &match_team(6, "Canada"), "FINISHED", "GROUP_STAGE", Some("GROUP_B"), (Some(2), Some(1))),
        match_with(8, &match_team(5, "Switzerland"), &match_team(7, "Bosnia-Herzegovina"), "FINISHED", "GROUP_STAGE", Some("GROUP_B"), (Some(4), Some(1))),
        match_with(9, &match_team(5, "Switzerland"), &match_team(8, "Qatar"), "FINISHED", "GROUP_STAGE", Some("GROUP_B"), (Some(2), Some(0))),
        match_with(10, &match_team(6, "Canada"), &match_team(7, "Bosnia-Herzegovina"), "FINISHED", "GROUP_STAGE", Some("GROUP_B"), (Some(1), Some(1))),
        match_with(11, &match_team(6, "Canada"), &match_team(8, "Qatar"), "FINISHED", "GROUP_STAGE", Some("GROUP_B"), (Some(6), Some(0))),
        match_with(12, &match_team(7, "Bosnia-Herzegovina"), &match_team(8, "Qatar"), "FINISHED", "GROUP_STAGE", Some("GROUP_B"), (Some(3), Some(1))),
        match_with(13, &match_team(9, "Brazil"), &match_team(10, "Morocco"), "TIMED", "GROUP_STAGE", Some("GROUP_C"), (None, None)),
    ];

    let teams = vec![
        team(1, "Mexico"),
        team(2, "South Africa"),
        team(3, "South Korea"),
        team(4, "Czechia"),
        team(5, "Switzerland"),
        team(6, "Canada"),
        team(7, "Bosnia-Herzegovina"),
        team(8, "Qatar"),
        team(9, "Brazil"),
        team(10, "Morocco"),
    ];

    let (still_in, eliminated) = ids(&classify_teams(&teams, &matches));

    assert!(still_in.contains(&3), "South Korea should still be in");
    assert!(still_in.contains(&7), "Bosnia-Herzegovina should still be in");
    assert!(eliminated.contains(&4));
    assert!(eliminated.contains(&8));
    assert!(!eliminated.contains(&3));
    assert!(!eliminated.contains(&7));
}

#[test]
fn third_place_eliminated_early_when_locked_out_of_top_eight() {
    // Eleven groups finished; team 9 is 9th on the third-place table with one group still playing.
    let mut matches = Vec::new();
    let mut teams = Vec::new();
    let mut match_id = 1_i64;

    for group_idx in 0..11 {
        let base = group_idx * 4 + 1;
        let group = format!("GROUP_{group_idx}");
        let group_teams = [
            match_team(base, &format!("A{group_idx}")),
            match_team(base + 1, &format!("B{group_idx}")),
            match_team(base + 2, &format!("C{group_idx}")),
            match_team(base + 3, &format!("D{group_idx}")),
        ];
        for group_team in &group_teams {
            teams.push(team(
                group_team.id.unwrap(),
                group_team.name.as_deref().unwrap(),
            ));
        }

        let round_robin = [
            (0, 1, (2, 0)),
            (0, 2, (2, 0)),
            (0, 3, (2, 0)),
            (1, 2, (2, 0)),
            (1, 3, (2, 0)),
            (2, 3, if group_idx == 8 {
                (1, 0)
            } else {
                (2, 0)
            }),
        ];
        for (home, away, score) in round_robin {
            matches.push(match_with(
                match_id,
                &group_teams[home],
                &group_teams[away],
                "FINISHED",
                "GROUP_STAGE",
                Some(&group),
                (Some(score.0), Some(score.1)),
            ));
            match_id += 1;
        }
    }

    matches.push(match_with(
        match_id,
        &match_team(45, "L1"),
        &match_team(46, "L2"),
        "TIMED",
        "GROUP_STAGE",
        Some("GROUP_11"),
        (None, None),
    ));
    teams.push(team(45, "L1"));
    teams.push(team(46, "L2"));

    let (_, eliminated) = ids(&classify_teams(&teams, &matches));
    assert!(eliminated.contains(&39), "9th third-place team should be out early");
    assert!(!eliminated.contains(&31), "8th third-place team should still be in");
}

#[test]
fn mathematically_eliminated_despite_upcoming_group_match() {
    let team_a = match_team(1, "Team A");
    let team_b = match_team(2, "Team B");
    let team_c = match_team(3, "Team C");
    let team_d = match_team(4, "Team D");
    let teams = vec![
        team(1, "Team A"),
        team(2, "Team B"),
        team(3, "Team C"),
        team(4, "Team D"),
    ];
    let matches = vec![
        match_with(1, &team_a, &team_d, "FINISHED", "GROUP_STAGE", Some("GROUP_A"), (Some(2), Some(0))),
        match_with(2, &team_b, &team_d, "FINISHED", "GROUP_STAGE", Some("GROUP_A"), (Some(2), Some(0))),
        match_with(3, &team_c, &team_d, "FINISHED", "GROUP_STAGE", Some("GROUP_A"), (Some(2), Some(0))),
        match_with(4, &team_a, &team_b, "FINISHED", "GROUP_STAGE", Some("GROUP_A"), (Some(2), Some(0))),
        match_with(5, &team_a, &team_c, "FINISHED", "GROUP_STAGE", Some("GROUP_A"), (Some(2), Some(0))),
        match_with(6, &team_b, &team_c, "TIMED", "GROUP_STAGE", Some("GROUP_A"), (None, None)),
    ];

    let (_, eliminated) = ids(&classify_teams(&teams, &matches));

    assert!(eliminated.contains(&4), "bottom team should be out despite others still playing");
    assert!(!eliminated.contains(&1));
    assert!(!eliminated.contains(&2));
    assert!(!eliminated.contains(&3));
}

#[test]
fn turkey_eliminated_after_losses_to_both_rivals_on_points() {
    // Group D 2026: Turkey lost to Australia and Paraguay; H2H blocks any path to 3rd.
    let usa = match_team(1, "USA");
    let australia = match_team(2, "Australia");
    let paraguay = match_team(3, "Paraguay");
    let turkey = match_team(4, "Turkey");
    let teams = vec![
        team(1, "USA"),
        team(2, "Australia"),
        team(3, "Paraguay"),
        team(4, "Turkey"),
    ];
    let matches = vec![
        match_with(1, &usa, &paraguay, "FINISHED", "GROUP_STAGE", Some("GROUP_D"), (Some(4), Some(1))),
        match_with(2, &australia, &turkey, "FINISHED", "GROUP_STAGE", Some("GROUP_D"), (Some(2), Some(0))),
        match_with(3, &turkey, &paraguay, "FINISHED", "GROUP_STAGE", Some("GROUP_D"), (Some(0), Some(1))),
        match_with(4, &usa, &australia, "FINISHED", "GROUP_STAGE", Some("GROUP_D"), (Some(2), Some(0))),
        match_with(5, &turkey, &usa, "TIMED", "GROUP_STAGE", Some("GROUP_D"), (None, None)),
        match_with(6, &paraguay, &australia, "TIMED", "GROUP_STAGE", Some("GROUP_D"), (None, None)),
    ];

    let (_, eliminated) = ids(&classify_teams(&teams, &matches));

    assert!(eliminated.contains(&4), "Turkey should be out on 2026 head-to-head rules");
}

#[test]
fn third_place_eliminates_both_teams() {
    let home = match_team(1, "France");
    let away = match_team(2, "Portugal");
    let teams = vec![team(1, "France"), team(2, "Portugal")];
    let matches = vec![match_with(
        1,
        &home,
        &away,
        "FINISHED",
        "THIRD_PLACE",
        None,
        (Some(2), Some(1)),
    )];

    let (still_in, eliminated) = ids(&classify_teams(&teams, &matches));
    assert!(still_in.is_empty());
    assert_eq!(eliminated, vec![1, 2]);
}

#[test]
fn placeholder_knockout_slots_are_ignored() {
    let teams = vec![team(769, "Mexico")];
    let placeholder = Match {
        id: 537417,
        home_team: MatchTeam {
            id: None,
            name: None,
            short_name: None,
            tla: None,
        },
        away_team: MatchTeam {
            id: None,
            name: None,
            short_name: None,
            tla: None,
        },
        score: Score {
            full_time: ScoreDetail {
                home: None,
                away: None,
            },
        },
        status: Some("TIMED".into()),
        stage: Some("LAST_32".into()),
        group: None,
        matchday: None,
    };

    let classification = classify_teams(&teams, &[placeholder]);
    assert_eq!(classification.still_in.len(), 1);
    assert!(classification.eliminated.is_empty());
}
