mod order;

pub use order::next_picker;

use rand::seq::SliceRandom;

use crate::{
    db::{
        DraftOrderKind, DraftParticipant, DraftSession, DraftSessionStatus, Registration,
        RosterPhase, Season,
    },
    league::League,
    types::{Data, Error},
};

#[derive(Debug, Clone)]
pub struct DraftStatus {
    pub order: Vec<u64>,
    pub order_kind: DraftOrderKind,
    pub pick_index: usize,
    pub on_the_clock: Option<u64>,
    pub session_status: DraftSessionStatus,
    pub roster_phase: RosterPhase,
    pub remaining_teams: usize,
}

pub async fn start_for_guild(
    data: &Data,
    guild_id: u64,
    mut user_ids: Vec<u64>,
) -> Result<String, Error> {
    user_ids.sort_unstable();
    user_ids.dedup();
    if user_ids.len() < 2 {
        return Ok("Need at least two distinct players to start a draft.".into());
    }

    let season = {
        let conn = data.db.lock().await;
        Season::default_for_guild(&conn, guild_id)?
    };

    if season.roster_phase != RosterPhase::Open {
        return Ok(format!(
            "Cannot start a draft while roster phase is **{}**.",
            season.roster_phase.as_str()
        ));
    }

    {
        let conn = data.db.lock().await;
        let existing = Registration::list_for_season(&conn, season.id)?;
        if !existing.is_empty() {
            return Ok(
                "Clear all team claims before starting a draft (season must have no registrations)."
                    .into(),
            );
        }
        if let Some(session) = DraftSession::get(&conn, season.id)?
            && session.status == DraftSessionStatus::Active
        {
            return Ok("A draft is already active for this season. Use `/draft status`.".into());
        }
    }

    {
        let mut rng = rand::rng();
        user_ids.shuffle(&mut rng);
    }

    let created_at = unix_timestamp_secs();
    {
        let conn = data.db.lock().await;
        DraftSession::upsert(
            &conn,
            season.id,
            DraftOrderKind::Snake,
            DraftSessionStatus::Active,
            &created_at,
        )?;
        DraftParticipant::replace_all(&conn, season.id, &user_ids)?;
        Season::set_roster_phase(&conn, season.id, RosterPhase::Drafting)?;
    }

    let on_clock = next_picker(&user_ids, 0, DraftOrderKind::Snake).unwrap();
    let order_line = user_ids
        .iter()
        .enumerate()
        .map(|(i, id)| format!("{}. <@{}>", i + 1, id))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(format!(
        "Snake draft started (order randomized).\n\n{order_line}\n\nOn the clock: <@{}> — use `/draft pick`.",
        on_clock
    ))
}

pub async fn status_for_guild(data: &Data, guild_id: u64) -> Result<String, Error> {
    let status = load_status(data, guild_id).await?;
    let Some(status) = status else {
        return Ok("No draft for this season. An admin can `/draft start` with a list of players.".into());
    };

    let order_line = status
        .order
        .iter()
        .enumerate()
        .map(|(i, id)| format!("{}. <@{}>", i + 1, id))
        .collect::<Vec<_>>()
        .join("\n");

    let clock = match status.on_the_clock {
        Some(id) if status.session_status == DraftSessionStatus::Active => {
            format!("<@{id}>")
        }
        _ => "—".into(),
    };

    Ok(format!(
        "**Draft status** (`{}`, phase `{}`)\nPick #{} · On the clock: {} · {} team(s) left\n\nOrder:\n{order_line}",
        status.order_kind.as_str(),
        status.roster_phase.as_str(),
        status.pick_index + 1,
        clock,
        status.remaining_teams,
    ))
}

/// On-clock user picks a team for themselves.
pub async fn pick_for_user(
    data: &Data,
    guild_id: u64,
    user_id: u64,
    team_query: &str,
) -> Result<String, Error> {
    pick_internal(data, guild_id, user_id, user_id, team_query, false).await
}

async fn pick_internal(
    data: &Data,
    guild_id: u64,
    actor_id: u64,
    beneficiary_id: u64,
    team_query: &str,
    admin_proxy: bool,
) -> Result<String, Error> {
    let (season_id, league, order, order_kind, phase) = {
        let conn = data.db.lock().await;
        let (season, league) = League::for_guild(&conn, guild_id)?;
        if season.roster_phase != RosterPhase::Drafting {
            return Ok(match season.roster_phase {
                RosterPhase::Open => {
                    "No active draft. An admin can `/draft start` when ready.".into()
                }
                RosterPhase::Frozen => {
                    "The roster is frozen after the draft. Picks are locked.".into()
                }
                RosterPhase::Drafting => unreachable!(),
            });
        }
        let session = DraftSession::get(&conn, season.id)?
            .ok_or("Draft session missing while roster phase is drafting.")?;
        if session.status != DraftSessionStatus::Active {
            return Ok("This draft is already complete.".into());
        }
        let order = DraftParticipant::user_ids_ordered(&conn, season.id)?;
        (
            season.id,
            league,
            order,
            session.order_kind,
            season.roster_phase,
        )
    };
    let _ = phase;

    let pick_index = {
        let conn = data.db.lock().await;
        Registration::list_for_season(&conn, season_id)?.len()
    };
    let Some(on_clock) = next_picker(&order, pick_index, order_kind) else {
        return Ok("Draft order is empty.".into());
    };

    if beneficiary_id != on_clock {
        return Ok(format!(
            "It is <@{}>'s turn to pick (not <@{}>).",
            on_clock, beneficiary_id
        ));
    }
    if !admin_proxy && actor_id != on_clock {
        return Ok(format!("It is <@{}>'s turn to pick.", on_clock));
    }

    let api_teams = league.list_teams(data).await?;
    let Some(selected) = league.find_team(&api_teams, team_query) else {
        return Ok(league.team_not_found_message(team_query));
    };

    {
        let conn = data.db.lock().await;
        if let Some(existing) = Registration::get_by_team(&conn, season_id, selected.id)? {
            return Ok(format!(
                "**{}** is already claimed by <@{}>.",
                selected.name, existing.user_id
            ));
        }
        Registration::upsert(
            &conn,
            season_id,
            beneficiary_id,
            selected.id,
            &selected.name,
        )?;
    }

    let remaining = count_unclaimed(data, guild_id, league).await?;
    if remaining == 0 {
        let conn = data.db.lock().await;
        DraftSession::set_status(&conn, season_id, DraftSessionStatus::Complete)?;
        Season::set_roster_phase(&conn, season_id, RosterPhase::Frozen)?;
        return Ok(format!(
            "**{}** drafted by <@{}>.\n\nAll teams are taken — draft complete. Roster is **frozen**.",
            selected.name, beneficiary_id
        ));
    }

    let next_index = pick_index + 1;
    let next = next_picker(&order, next_index, order_kind).unwrap();
    Ok(format!(
        "**{}** drafted by <@{}>.\n\nOn the clock: <@{}> · {} team(s) left.",
        selected.name, beneficiary_id, next, remaining
    ))
}

/// Undo the most recent draft pick. Only the user who made that pick may call this,
/// and only before anyone else picks (or the draft freezes).
pub async fn unpick_for_user(
    data: &Data,
    guild_id: u64,
    user_id: u64,
) -> Result<String, Error> {
    let (season_id, order, order_kind) = {
        let conn = data.db.lock().await;
        let (season, _league) = League::for_guild(&conn, guild_id)?;
        if season.roster_phase != RosterPhase::Drafting {
            return Ok(match season.roster_phase {
                RosterPhase::Open => {
                    "No active draft. Use `/unclaim` while the roster is open.".into()
                }
                RosterPhase::Frozen => {
                    "The roster is frozen after the draft. Unpicks are locked.".into()
                }
                RosterPhase::Drafting => unreachable!(),
            });
        }
        let session = DraftSession::get(&conn, season.id)?
            .ok_or("Draft session missing while roster phase is drafting.")?;
        if session.status != DraftSessionStatus::Active {
            return Ok("This draft is already complete.".into());
        }
        let order = DraftParticipant::user_ids_ordered(&conn, season.id)?;
        (season.id, order, session.order_kind)
    };

    let latest = {
        let conn = data.db.lock().await;
        Registration::latest_for_season(&conn, season_id)?
    };
    let Some(latest) = latest else {
        return Ok("No picks to undo yet.".into());
    };

    if latest.user_id != user_id {
        return Ok(format!(
            "Only the last picker (<@{}>) can `/draft unpick`.",
            latest.user_id
        ));
    }

    {
        let conn = data.db.lock().await;
        let league = League::for_season(&conn, season_id)?;
        league.clear_picks_for_team(&conn, season_id, latest.user_id, latest.team_id)?;
        Registration::delete(&conn, season_id, latest.user_id, latest.team_id)?;
    }

    let pick_index = {
        let conn = data.db.lock().await;
        Registration::list_for_season(&conn, season_id)?.len()
    };
    let on_clock = next_picker(&order, pick_index, order_kind).unwrap_or(user_id);

    Ok(format!(
        "**{}** unpicked by <@{}>.\n\nOn the clock: <@{}>.",
        latest.team_name, user_id, on_clock
    ))
}

/// End an in-progress draft early and freeze the roster without drafting every team.
pub async fn freeze_for_guild(data: &Data, guild_id: u64) -> Result<String, Error> {
    let season = {
        let conn = data.db.lock().await;
        Season::default_for_guild(&conn, guild_id)?
    };

    match season.roster_phase {
        RosterPhase::Open => {
            return Ok(
                "No active draft. Start one with `/draft start`, or use `/draft end` only while drafting."
                    .into(),
            );
        }
        RosterPhase::Frozen => return Ok("Roster is already **frozen**.".into()),
        RosterPhase::Drafting => {}
    }

    {
        let conn = data.db.lock().await;
        let Some(session) = DraftSession::get(&conn, season.id)? else {
            return Ok("No draft session for this season.".into());
        };
        if session.status != DraftSessionStatus::Active {
            Season::set_roster_phase(&conn, season.id, RosterPhase::Frozen)?;
            return Ok("Roster is **frozen**.".into());
        }
        DraftSession::set_status(&conn, season.id, DraftSessionStatus::Complete)?;
        Season::set_roster_phase(&conn, season.id, RosterPhase::Frozen)?;
    }

    Ok("Draft ended. Roster is **frozen**.".into())
}

pub async fn current_picker_for_guild(data: &Data, guild_id: u64) -> Result<Option<u64>, Error> {
    Ok(load_status(data, guild_id)
        .await?
        .and_then(|s| s.on_the_clock))
}

async fn load_status(data: &Data, guild_id: u64) -> Result<Option<DraftStatus>, Error> {
    let (season, league, session, order) = {
        let conn = data.db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        let league = League::for_season(&conn, season.id)?;
        let Some(session) = DraftSession::get(&conn, season.id)? else {
            return Ok(None);
        };
        let order = DraftParticipant::user_ids_ordered(&conn, season.id)?;
        (season, league, session, order)
    };

    let pick_index = {
        let conn = data.db.lock().await;
        Registration::list_for_season(&conn, season.id)?.len()
    };
    let on_the_clock = if session.status == DraftSessionStatus::Active {
        next_picker(&order, pick_index, session.order_kind)
    } else {
        None
    };
    let remaining_teams = count_unclaimed(data, guild_id, league).await?;

    Ok(Some(DraftStatus {
        order,
        order_kind: session.order_kind,
        pick_index,
        on_the_clock,
        session_status: session.status,
        roster_phase: season.roster_phase,
        remaining_teams,
    }))
}

async fn count_unclaimed(data: &Data, guild_id: u64, league: League) -> Result<usize, Error> {
    let api_teams = league.list_teams(data).await?;
    let claimed = {
        let conn = data.db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        Registration::list_for_season(&conn, season.id)?
            .into_iter()
            .map(|r| r.team_id)
            .collect::<std::collections::HashSet<_>>()
    };
    Ok(api_teams
        .iter()
        .filter(|t| !claimed.contains(&t.id))
        .count())
}

fn unix_timestamp_secs() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .to_string()
}
