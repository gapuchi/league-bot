use std::collections::{HashMap, HashSet};

use crate::{
    db::{Registration, RosterPhase, Season},
    draft,
    league::League,
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

async fn focused_league_teams(
    data: &Data,
    guild_id: u64,
) -> Result<(i64, League, Vec<crate::league::CatalogTeam>), Error> {
    let (season_id, league) = {
        let conn = data.db.lock().await;
        let (season, league) = League::for_guild(&conn, guild_id)?;
        (season.id, league)
    };
    let teams = league.list_teams(data).await?;
    Ok((season_id, league, teams))
}

pub async fn pick_for_user(
    data: &Data,
    guild_id: u64,
    user_id: u64,
    team_query: &str,
) -> Result<String, Error> {
    let phase = {
        let conn = data.db.lock().await;
        Season::default_for_guild(&conn, guild_id)?.roster_phase
    };

    match phase {
        RosterPhase::Drafting => {
            return draft::pick_for_user(data, guild_id, user_id, team_query).await;
        }
        RosterPhase::Frozen => {
            return Ok("The roster is frozen after the draft. Picks are locked.".into());
        }
        RosterPhase::Open => {}
    }

    let (season_id, league, api_teams) = focused_league_teams(data, guild_id).await?;
    let Some(selected) = league.find_team(&api_teams, team_query) else {
        return Ok(league.team_not_found_message(team_query));
    };

    {
        let conn = data.db.lock().await;
        if let Some(existing) = Registration::get_by_team(&conn, season_id, selected.id)?
            && existing.user_id != user_id
        {
            return Ok(format!(
                "{} is already claimed by <@{}>.",
                selected.name, existing.user_id
            ));
        }

        Registration::upsert(&conn, season_id, user_id, selected.id, &selected.name)?;
    }

    Ok(format!(
        "You've picked **{}**. You'll earn points when they play.",
        selected.name
    ))
}

pub async fn assign_for_user(
    data: &Data,
    guild_id: u64,
    user_id: u64,
    team_query: &str,
    assignee_mention: &str,
) -> Result<String, Error> {
    let phase = {
        let conn = data.db.lock().await;
        Season::default_for_guild(&conn, guild_id)?.roster_phase
    };

    match phase {
        RosterPhase::Frozen => {
            return Ok("The roster is frozen after the draft. Assignments are locked.".into());
        }
        RosterPhase::Drafting => {
            return draft::pick_for_user(data, guild_id, user_id, team_query)
                .await
                .map(|msg| {
                    if msg.starts_with("**") && msg.contains("drafted") {
                        format!("{msg}\n(via admin `/assign` for {assignee_mention})")
                    } else {
                        msg
                    }
                });
        }
        RosterPhase::Open => {}
    }

    let (season_id, league, api_teams) = focused_league_teams(data, guild_id).await?;
    let Some(selected) = league.find_team(&api_teams, team_query) else {
        return Ok(league.team_not_found_message(team_query));
    };

    {
        let conn = data.db.lock().await;
        if let Some(existing) = Registration::get_by_team(&conn, season_id, selected.id)?
            && existing.user_id != user_id
        {
            return Ok(format!(
                "**{}** is already claimed by <@{}>.",
                selected.name, existing.user_id
            ));
        }

        Registration::upsert(&conn, season_id, user_id, selected.id, &selected.name)?;
    }

    Ok(format!(
        "**{}** has been claimed by {}.",
        selected.name, assignee_mention
    ))
}

pub async fn unclaim_for_user(
    data: &Data,
    guild_id: u64,
    user_id: u64,
    team_query: &str,
) -> Result<String, Error> {
    {
        let conn = data.db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        if let Some(msg) = phase_blocks_open_claims(season.roster_phase) {
            return Ok(msg.into());
        }
    }

    let (season_id, league, api_teams) = focused_league_teams(data, guild_id).await?;
    let Some(selected) = league.find_team(&api_teams, team_query) else {
        return Ok(league.team_not_found_message(team_query));
    };

    let removed = {
        let conn = data.db.lock().await;
        league.clear_picks_for_team(&conn, season_id, user_id, selected.id)?;
        Registration::delete(&conn, season_id, user_id, selected.id)?
    };

    Ok(if removed {
        "That team has been unclaimed.".into()
    } else {
        "You haven't claimed that team. Use `/team` to see your teams.".into()
    })
}

pub async fn my_team_message(
    data: &Data,
    guild_id: u64,
    user_id: u64,
) -> Result<String, Error> {
    let (registrations, pick, tiebreaker_value) = {
        let conn = data.db.lock().await;
        let (season, league) = League::for_guild(&conn, guild_id)?;
        let registrations = Registration::list_for_user(&conn, season.id, user_id)?;
        let pick = league.tiebreaker_pick_for_user(&conn, season.id, user_id)?;
        let tiebreaker_value = league.tiebreaker_value_for_user(&conn, season.id, user_id)?;
        (registrations, pick, tiebreaker_value)
    };

    let mut message = match registrations.as_slice() {
        [] => "You haven't picked any teams yet. Use `/draft pick` to choose one.".into(),
        [registration] => format!("You're representing **{}**.", registration.team_name),
        _ => {
            let teams: Vec<&str> = registrations
                .iter()
                .map(|registration| registration.team_name.as_str())
                .collect();
            format!("You're representing: **{}**.", teams.join("**, **"))
        }
    };

    if let Some((player_name, team_name)) = pick {
        message.push_str(&format!(
            "\n\nTie-breaker: **{player_name}** ({team_name}) — **{tiebreaker_value}** goals"
        ));
    } else if !registrations.is_empty() {
        message.push_str("\n\nTie-breaker: none — use `/pick-player` to designate one.");
    }

    Ok(message)
}

pub enum SeasonTeamsList {
    Empty,
    ByUser(Vec<(u64, Vec<String>)>),
}

pub async fn list_season_teams(data: &Data, guild_id: u64) -> Result<SeasonTeamsList, Error> {
    let registrations = {
        let conn = data.db.lock().await;
        let (season, _) = League::for_guild(&conn, guild_id)?;
        Registration::list_for_season(&conn, season.id)?
    };

    if registrations.is_empty() {
        return Ok(SeasonTeamsList::Empty);
    }

    let mut by_user: HashMap<u64, Vec<String>> = HashMap::new();
    for registration in &registrations {
        by_user
            .entry(registration.user_id)
            .or_default()
            .push(registration.team_name.clone());
    }

    let mut user_ids: Vec<u64> = by_user.keys().copied().collect();
    user_ids.sort();

    let assignments = user_ids
        .into_iter()
        .map(|user_id| (user_id, by_user.remove(&user_id).unwrap_or_default()))
        .collect();

    Ok(SeasonTeamsList::ByUser(assignments))
}

pub enum UnclaimedTeams {
    AllClaimed,
    Available(Vec<String>),
}

pub async fn unclaimed_teams(data: &Data, guild_id: u64) -> Result<UnclaimedTeams, Error> {
    let (_season_id, _league, api_teams) = focused_league_teams(data, guild_id).await?;

    let claimed_team_ids = {
        let conn = data.db.lock().await;
        let (season, _) = League::for_guild(&conn, guild_id)?;
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

    Ok(if unclaimed_names.is_empty() {
        UnclaimedTeams::AllClaimed
    } else {
        UnclaimedTeams::Available(unclaimed_names)
    })
}
