use crate::{season, types::{Context, Error}};

use super::helpers::guild_id;

/// Season lifecycle and status
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    subcommands("season_start", "season_end", "season_status"),
    subcommand_required
)]
pub async fn season(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Create or resume a season and set command focus
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "start",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn season_start(
    ctx: Context<'_>,
    #[description = "League slug (e.g. wc)"] league: String,
    #[description = "Season slug (e.g. wc-2026)"] slug: String,
    #[description = "Display name (e.g. World Cup 2026)"] name: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let message =
        season::start_for_guild(ctx.data(), guild_id, &league, &slug, &name).await?;
    ctx.say(message).await?;
    Ok(())
}

/// Stop match polling for the focused season
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "end",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn season_end(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let message = season::end_for_guild(ctx.data(), guild_id).await?;
    ctx.say(message).await?;
    Ok(())
}

/// Show the focused season and polling status
#[poise::command(prefix_command, slash_command, guild_only, rename = "status")]
pub async fn season_status(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;
    let message = season::status_for_guild(ctx.data(), guild_id).await?;
    ctx.say(message).await?;
    Ok(())
}
