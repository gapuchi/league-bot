use league_bot::{
    db::Registration,
    soccer::TeamRef,
    wc::remaining::group_teams_by_user,
};

fn registration(user_id: u64, team_id: i64, name: &str) -> Registration {
    Registration {
        user_id,
        team_id,
        team_name: name.into(),
    }
}

#[test]
fn groups_teams_by_registered_owner() {
    let teams = vec![
        TeamRef {
            id: 1,
            name: "Brazil".into(),
        },
        TeamRef {
            id: 2,
            name: "Mexico".into(),
        },
        TeamRef {
            id: 3,
            name: "Japan".into(),
        },
    ];
    let registrations = vec![
        registration(10, 1, "Brazil"),
        registration(10, 2, "Mexico"),
        registration(20, 3, "Japan"),
    ];

    let (by_user, unassigned) = group_teams_by_user(&teams, &registrations);
    assert!(unassigned.is_empty());
    assert_eq!(by_user.len(), 2);
    assert_eq!(by_user[0], (10, vec!["Brazil".into(), "Mexico".into()]));
    assert_eq!(by_user[1], (20, vec!["Japan".into()]));
}

#[test]
fn unassigned_teams_listed_separately() {
    let teams = vec![TeamRef {
        id: 99,
        name: "Turkey".into(),
    }];
    let (by_user, unassigned) = group_teams_by_user(&teams, &[]);
    assert!(by_user.is_empty());
    assert_eq!(unassigned, vec!["Turkey".to_string()]);
}
