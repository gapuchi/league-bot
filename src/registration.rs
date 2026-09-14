use std::collections::{HashMap, HashSet};

use crate::{
    db::{Registration, RosterPhase, Season},
    draft,
    league::{CatalogTeam, League},
    season,
    types::{Data, Error},
};

fn phase_blocks_open_claims(phase: RosterPhase) -> Option<&'static str> {
    match phase {
        RosterPhase::Open => None,
        RosterPhase::Drafting => {
            Some("A draft is in progress. On-clock players use `/draft pick`; the last picker may `/draft unpick`; admins may `/assign` only to the player on the clock.")
        }
        RosterPhase::Frozen => {
            Some("The roster is frozen after the draft. Claims and unclaims are locked.")
        }
    }
}

async fn target_with_teams(
    data: &Data,
    guild_id: u64,
    league: Option<League>,
) -> Result<(Season, League, Vec<CatalogTeam>), Error> {
    let (season, league) = {
        let conn = data.db.lock().await;
        season::resolve(&conn, guild_id, league)?
    };
    let teams = league.list_teams(data).await?;
    Ok((season, league, teams))
}

async fn claim(
    data: &Data,
    season_id: i64,
    league: League,
    teams: &[CatalogTeam],
    user_id: u64,
    team_query: &str,
) -> Result<Result<String, String>, Error> {
    let Some(selected) = league.find_team(teams, team_query) else {
        return Ok(Err(league.team_not_found_message(team_query)));
    };

    let conn = data.db.lock().await;
    if let Some(existing) = Registration::get_by_team(&conn, season_id, selected.id)?
        && existing.user_id != user_id
    {
        return Ok(Err(format!(
            "**{}** is already claimed by <@{}>.",
            selected.name, existing.user_id
        )));
    }
    Registration::upsert(&conn, season_id, user_id, selected.id, &selected.name)?;
    Ok(Ok(selected.name.clone()))
}

/// Open-roster claim. During a draft the caller is redirected to `/draft pick`.
pub async fn claim_for_user(
    data: &Data,
    guild_id: u64,
    league: Option<League>,
    user_id: u64,
    team_query: &str,
) -> Result<String, Error> {
    let (season, league, teams) = target_with_teams(data, guild_id, league).await?;
    if let Some(msg) = phase_blocks_open_claims(season.roster_phase) {
        return Ok(msg.into());
    }

    Ok(match claim(data, season.id, league, &teams, user_id, team_query).await? {
        Ok(name) => format!("You've claimed **{name}**. You'll earn points when they play."),
        Err(msg) => msg,
    })
}

pub async fn assign_for_user(
    data: &Data,
    guild_id: u64,
    league: Option<League>,
    user_id: u64,
    team_query: &str,
    assignee_mention: &str,
) -> Result<String, Error> {
    let (season, league, teams) = target_with_teams(data, guild_id, league).await?;

    match season.roster_phase {
        RosterPhase::Frozen => {
            return Ok("The roster is frozen after the draft. Assignments are locked.".into());
        }
        RosterPhase::Drafting => {
            let outcome =
                draft::pick_for_user(data, guild_id, Some(league), user_id, team_query).await?;
            let is_pick = matches!(&outcome, draft::PickOutcome::Picked { .. });
            let msg = outcome.into_message();
            return Ok(if is_pick {
                format!("{msg}\n(via admin `/assign` for {assignee_mention})")
            } else {
                msg
            });
        }
        RosterPhase::Open => {}
    }

    Ok(match claim(data, season.id, league, &teams, user_id, team_query).await? {
        Ok(name) => format!("**{name}** has been claimed by {assignee_mention}."),
        Err(msg) => msg,
    })
}

pub async fn unclaim_for_user(
    data: &Data,
    guild_id: u64,
    league: Option<League>,
    user_id: u64,
    team_query: &str,
) -> Result<String, Error> {
    let (season, league, teams) = target_with_teams(data, guild_id, league).await?;
    if let Some(msg) = phase_blocks_open_claims(season.roster_phase) {
        return Ok(msg.into());
    }
    let Some(selected) = league.find_team(&teams, team_query) else {
        return Ok(league.team_not_found_message(team_query));
    };

    let removed = {
        let conn = data.db.lock().await;
        league.clear_picks_for_team(&conn, season.id, user_id, selected.id)?;
        Registration::delete(&conn, season.id, user_id, selected.id)?
    };

    Ok(if removed {
        format!("**{}** has been unclaimed.", selected.name)
    } else {
        "You haven't claimed that team. Use `/team` to see your teams.".into()
    })
}

pub async fn my_team_message(
    data: &Data,
    guild_id: u64,
    league: Option<League>,
    user_id: u64,
) -> Result<String, Error> {
    let (season, league) = {
        let conn = data.db.lock().await;
        season::resolve(&conn, guild_id, league)?
    };
    let (registrations, pick, tiebreaker_value) = {
        let conn = data.db.lock().await;
        (
            Registration::list_for_user(&conn, season.id, user_id)?,
            league.tiebreaker_pick_for_user(&conn, season.id, user_id)?,
            league.tiebreaker_value_for_user(&conn, season.id, user_id)?,
        )
    };

    let mut message = match registrations.as_slice() {
        [] => format!(
            "You haven't claimed any {} teams yet. Use `/claim` to choose one.",
            league.display_name()
        ),
        [registration] => format!(
            "{}: you're representing **{}**.",
            season.name, registration.team_name
        ),
        _ => {
            let teams: Vec<&str> = registrations
                .iter()
                .map(|registration| registration.team_name.as_str())
                .collect();
            format!(
                "{}: you're representing **{}**.",
                season.name,
                teams.join("**, **")
            )
        }
    };

    match (league.tiebreaker_unit(), pick) {
        (None, _) => {}
        (Some(unit), Some((player_name, team_name))) => {
            message.push_str(&format!(
                "\n\nTie-breaker: **{player_name}** ({team_name}) — **{tiebreaker_value}** {unit}"
            ));
        }
        (Some(_), None) if !registrations.is_empty() => {
            message.push_str("\n\nTie-breaker: none — use `/pick-player` to designate one.");
        }
        (Some(_), None) => {}
    }

    Ok(message)
}

pub enum SeasonTeamsList {
    Empty,
    ByUser {
        title: String,
        assignments: Vec<(u64, Vec<String>)>,
    },
}

pub async fn list_season_teams(
    data: &Data,
    guild_id: u64,
    league: Option<League>,
) -> Result<SeasonTeamsList, Error> {
    let (season, registrations) = {
        let conn = data.db.lock().await;
        let (season, _) = season::resolve(&conn, guild_id, league)?;
        let registrations = Registration::list_for_season(&conn, season.id)?;
        (season, registrations)
    };

    if registrations.is_empty() {
        return Ok(SeasonTeamsList::Empty);
    }

    let mut assignments: Vec<(u64, Vec<String>)> = registrations
        .iter()
        .fold(
            HashMap::<u64, Vec<String>>::new(),
            |mut map, registration| {
                map.entry(registration.user_id)
                    .or_default()
                    .push(registration.team_name.clone());
                map
            },
        )
        .into_iter()
        .collect();
    assignments.sort_unstable_by_key(|(user_id, _)| *user_id);

    Ok(SeasonTeamsList::ByUser {
        title: format!("{} team assignments", season.name),
        assignments,
    })
}

pub enum UnclaimedTeams {
    AllClaimed,
    Available(Vec<String>),
}

pub async fn unclaimed_teams(
    data: &Data,
    guild_id: u64,
    league: Option<League>,
) -> Result<UnclaimedTeams, Error> {
    let (season, _, api_teams) = target_with_teams(data, guild_id, league).await?;

    let claimed_team_ids = {
        let conn = data.db.lock().await;
        Registration::list_for_season(&conn, season.id)?
            .iter()
            .map(|registration| registration.team_id)
            .collect::<HashSet<_>>()
    };

    let mut unclaimed_names: Vec<String> = api_teams
        .iter()
        .filter(|team| !claimed_team_ids.contains(&team.id))
        .map(|team| team.name.clone())
        .collect();
    unclaimed_names.sort();

    Ok(match unclaimed_names.as_slice() {
        [] => UnclaimedTeams::AllClaimed,
        _ => UnclaimedTeams::Available(unclaimed_names),
    })
}
