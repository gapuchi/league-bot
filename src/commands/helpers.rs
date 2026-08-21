use crate::{
    league::League,
    types::{Context, Error},
};

pub(crate) fn guild_id(ctx: &Context<'_>) -> Result<u64, Error> {
    Ok(ctx
        .guild_id()
        .ok_or("This command must be used in a server.")?
        .get())
}

/// Ensures command focus is `expected`. On mismatch, replies and returns `false`.
pub(crate) async fn ensure_focused_league(
    ctx: &Context<'_>,
    expected: League,
) -> Result<bool, Error> {
    let guild_id = guild_id(ctx)?;
    let league = {
        let conn = ctx.data().db.lock().await;
        let (_, league) = League::for_guild(&conn, guild_id)?;
        league
    };
    if league != expected {
        ctx.say(format!(
            "This command is only available when the active season is {}. Use `/season status` to check, or `/config league` to switch.",
            expected.display_name()
        ))
        .await?;
        return Ok(false);
    }
    Ok(true)
}
