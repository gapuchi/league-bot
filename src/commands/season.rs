use poise::serenity_prelude as serenity;

use crate::{
    league::League,
    season,
    types::{Context, Error},
};

use super::helpers::guild_id;

/// Season lifecycle
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    subcommands("season_start", "season_end", "season_channel", "season_list"),
    subcommand_required
)]
pub async fn season(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Start a season for a league (ends that league's previous live season)
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "start",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn season_start(
    ctx: Context<'_>,
    #[description = "League to start a season for"] league: League,
    #[description = "Display name (default: league + current year, e.g. NFL 2026)"] name: Option<
        String,
    >,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let message = season::start_for_guild(ctx.data(), guild_id, league, name.as_deref()).await?;
    ctx.say(message).await?;
    Ok(())
}

/// Stop match polling for a season (data is kept)
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "end",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn season_end(
    ctx: Context<'_>,
    #[description = "League (optional when only one season is live)"] league: Option<League>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let message = season::end_for_guild(ctx.data(), guild_id, league).await?;
    ctx.say(message).await?;
    Ok(())
}

/// Set the channel for a season's match announcements
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "channel",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn season_channel(
    ctx: Context<'_>,
    #[description = "Channel for match result announcements"] channel: serenity::GuildChannel,
    #[description = "League (optional when only one season is live)"] league: Option<League>,
) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;
    let message =
        season::set_channel_for_guild(ctx.data(), guild_id, league, channel.id.get()).await?;
    ctx.say(message).await?;
    Ok(())
}

/// List every season in this server with its state
#[poise::command(prefix_command, slash_command, guild_only, rename = "list")]
pub async fn season_list(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;
    let message = season::list_for_guild(ctx.data(), guild_id).await?;
    ctx.say(message).await?;
    Ok(())
}
