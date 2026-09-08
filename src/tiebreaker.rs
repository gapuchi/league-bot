use rusqlite::Connection;

use crate::{
    db::{Registration, Season},
    types::{Data, Error},
};

/// Player on the roster of one of a user's claimed teams (any sport).
#[derive(Debug, Clone)]
pub struct RosterPlayer {
    pub player_id: i64,
    pub player_name: String,
    pub team_id: i64,
    pub team_name: String,
}

pub const NO_TEAMS_MESSAGE: &str =
    "Pick a team first with `/draft pick`, then pick a player from that roster.";

/// `(team_id, team_name)` for every team the user has claimed in the focused season.
pub async fn claimed_teams(
    data: &Data,
    guild_id: u64,
    user_id: u64,
) -> Result<Vec<(i64, String)>, Error> {
    let conn = data.db.lock().await;
    let season = Season::default_for_guild(&conn, guild_id)?;
    Ok(Registration::list_for_user(&conn, season.id, user_id)?
        .into_iter()
        .map(|r| (r.team_id, r.team_name))
        .collect())
}

pub fn find_players<'a>(players: &'a [RosterPlayer], query: &str) -> Vec<&'a RosterPlayer> {
    let query = query.trim().to_lowercase();
    players
        .iter()
        .filter(|player| {
            let name = player.player_name.to_lowercase();
            name == query || name.contains(&query)
        })
        .collect()
}

/// Shared tie-breaker pick flow once rosters are loaded. `upsert` persists the chosen
/// player for the season.
pub async fn resolve_pick(
    data: &Data,
    guild_id: u64,
    user_id: u64,
    player_query: &str,
    players: &[RosterPlayer],
    upsert: impl FnOnce(&Connection, i64, u64, &RosterPlayer) -> rusqlite::Result<()>,
) -> Result<String, Error> {
    let matches = find_players(players, player_query);

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

#[cfg(test)]
mod tests {
    use super::{RosterPlayer, find_players};

    fn player(id: i64, name: &str) -> RosterPlayer {
        RosterPlayer {
            player_id: id,
            player_name: name.into(),
            team_id: 1,
            team_name: "Team".into(),
        }
    }

    #[test]
    fn find_players_matches_case_insensitive_substring() {
        let players = [player(1, "Saquon Barkley"), player(2, "Jalen Hurts")];
        let found = find_players(&players, "barkley");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].player_id, 1);
        assert!(find_players(&players, "nobody").is_empty());
    }
}
