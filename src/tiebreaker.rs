use rusqlite::Connection;

use crate::{
    db::{Registration, Season},
    soccer::{fetch_squads_for_teams, find_players, SquadPlayerMatch},
    types::{Data, Error},
};

/// Shared soccer tie-breaker pick flow. `upsert` persists the chosen player for the season.
pub async fn pick_tiebreaker_player(
    data: &Data,
    guild_id: u64,
    user_id: u64,
    player_query: &str,
    upsert: impl FnOnce(&Connection, i64, u64, &SquadPlayerMatch) -> rusqlite::Result<()>,
) -> Result<String, Error> {
    let registrations = {
        let conn = data.db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        Registration::list_for_user(&conn, season.id, user_id)?
    };

    if registrations.is_empty() {
        return Ok(
            "Pick a team first with `/draft pick`, then pick a player from that squad.".into(),
        );
    }

    let teams: Vec<(i64, String)> = registrations
        .iter()
        .map(|r| (r.team_id, r.team_name.clone()))
        .collect();

    let api = crate::api::FootballDataApi::from_env(data.http.clone());
    let squad = fetch_squads_for_teams(&api, &teams).await?;
    let matches = find_players(&squad, player_query);

    match matches.as_slice() {
        [] => Ok(format!(
            "Couldn't find a player matching \"{player_query}\" on your claimed teams. Try a more specific name."
        )),
        [selected] => {
            let conn = data.db.lock().await;
            let season = Season::default_for_guild(&conn, guild_id)?;
            upsert(&conn, season.id, user_id, selected)?;
            Ok(format!(
                "Tie-breaker player set to **{}** ({})",
                selected.player_name, selected.team_name
            ))
        }
        _ => {
            let options: Vec<String> = matches
                .iter()
                .take(10)
                .map(|c| format!("**{}** ({})", c.player_name, c.team_name))
                .collect();
            Ok(format!(
                "Several players match \"{player_query}\". Be more specific:\n{}",
                options.join("\n")
            ))
        }
    }
}
