use crate::{
    api::EspnNflApi,
    tiebreaker::RosterPlayer,
    types::{Data, Error},
};

/// Every player on the current roster of each claimed team.
pub async fn fetch_rosters(
    data: &Data,
    teams: &[(i64, String)],
) -> Result<Vec<RosterPlayer>, Error> {
    let api = EspnNflApi::new(data.http.clone());
    let mut players = Vec::new();
    for (team_id, team_name) in teams {
        let roster = api.fetch_team_roster(*team_id).await?;
        players.extend(roster.into_iter().map(|player| RosterPlayer {
            player_id: player.id,
            player_name: player.full_name,
            team_id: *team_id,
            team_name: team_name.clone(),
        }));
    }
    Ok(players)
}
