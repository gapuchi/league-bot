use crate::{
    league::League,
    types::{Context, Error},
};

use super::helpers::guild_id;

/// Designate a tie-breaker player from your claimed teams' squads
#[poise::command(prefix_command, slash_command, guild_only, rename = "pick-player")]
pub async fn pick_player(
    ctx: Context<'_>,
    #[description = "Player name from one of your claimed teams (e.g. Salah)"] player: String,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let guild_id = guild_id(&ctx)?;
    let league = {
        let conn = ctx.data().db.lock().await;
        League::for_guild(&conn, guild_id)?.1
    };

    let message = league
        .pick_tiebreaker_player(ctx.data(), guild_id, ctx.author().id.get(), &player)
        .await?;

    ctx.say(message).await?;
    Ok(())
}
