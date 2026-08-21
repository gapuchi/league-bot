use std::collections::HashMap;

use crate::{
    db::{league_competition_code, Registration, Season},
    soccer::{self, TeamClassification, TeamRef},
    types::{Data, Error},
};

pub enum RemainingResult {
    WrongLeague,
    NoRegistrations,
    Report(RemainingReport),
}

pub struct RemainingReport {
    pub still_in_by_user: Vec<(u64, Vec<String>)>,
    pub unassigned_still_in: Vec<String>,
}

pub async fn list_for_guild(data: &Data, guild_id: u64) -> Result<RemainingResult, Error> {
    let (competition, season_id) = {
        let conn = data.db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        let league_slug = Season::league_slug_for(&conn, season.id)?;
        if league_slug != "wc" {
            return Ok(RemainingResult::WrongLeague);
        }
        (league_competition_code(&league_slug), season.id)
    };

    let api = crate::api::FootballDataApi::from_env(data.http.clone());
    let teams = api.fetch_teams(&competition).await?;
    let matches = api.fetch_competition_matches(&competition).await?;
    let classification = soccer::classify_teams(&teams, &matches);

    let registrations = {
        let conn = data.db.lock().await;
        Registration::list_for_season(&conn, season_id)?
    };

    if registrations.is_empty() {
        return Ok(RemainingResult::NoRegistrations);
    }

    Ok(RemainingResult::Report(build_report(
        &classification,
        &registrations,
    )))
}

fn build_report(
    classification: &TeamClassification,
    registrations: &[Registration],
) -> RemainingReport {
    let (still_in_by_user, unassigned_still_in) =
        group_teams_by_user(&classification.still_in, registrations);

    RemainingReport {
        still_in_by_user,
        unassigned_still_in,
    }
}

pub fn group_teams_by_user(
    teams: &[TeamRef],
    registrations: &[Registration],
) -> (Vec<(u64, Vec<String>)>, Vec<String>) {
    let owner_by_team: HashMap<i64, u64> = registrations
        .iter()
        .map(|registration| (registration.team_id, registration.user_id))
        .collect();

    let mut by_user = teams.iter().filter_map(|team| {
        owner_by_team
            .get(&team.id)
            .map(|&user_id| (user_id, team.name.clone()))
    }).fold(HashMap::<u64, Vec<String>>::new(), |mut map, (user_id, name)| {
        map.entry(user_id).or_default().push(name);
        map
    });
    for names in by_user.values_mut() {
        names.sort();
    }

    let mut unassigned: Vec<String> = teams
        .iter()
        .filter(|team| !owner_by_team.contains_key(&team.id))
        .map(|team| team.name.clone())
        .collect();
    unassigned.sort();

    let mut grouped: Vec<(u64, Vec<String>)> = by_user.into_iter().collect();
    grouped.sort_unstable_by_key(|(user_id, _)| *user_id);

    (grouped, unassigned)
}

pub fn format_grouped_field(by_user: &[(u64, Vec<String>)], unassigned: &[String]) -> String {
    let user_lines = by_user.iter().map(|(user_id, teams)| {
        format!("<@{}> — **{}**", user_id, teams.join("**, **"))
    });
    let unassigned_line =
        (!unassigned.is_empty()).then(|| format!("Unassigned — **{}**", unassigned.join("**, **")));

    let lines: Vec<String> = user_lines.chain(unassigned_line).collect();
    if lines.is_empty() {
        "—".into()
    } else {
        lines.join("\n")
    }
}
