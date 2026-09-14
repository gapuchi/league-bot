use crate::{
    league::League,
    season,
    types::{Context, Error},
};

use super::helpers::guild_id;

/// Designate a tie-breaker player from your claimed teams' rosters
#[poise::command(prefix_command, slash_command, guild_only, rename = "pick-player")]
pub async fn pick_player(
    ctx: Context<'_>,
    #[description = "Player name from one of your claimed teams (e.g. Salah)"] player: String,
    #[description = "League (optional when only one season is live)"] league: Option<League>,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let guild_id = guild_id(&ctx)?;
    let (season, league) = {
        let conn = ctx.data().db.lock().await;
        season::resolve(&conn, guild_id, league)?
    };

    let message = league
        .pick_tiebreaker_player(ctx.data(), season.id, ctx.author().id.get(), &player)
        .await?;

    ctx.say(message).await?;
    Ok(())
}
