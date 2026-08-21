use poise::serenity_prelude as serenity;

use crate::{
    draft, registration,
    types::{Context, Error},
};

use super::helpers::guild_id;

/// Team selection and pre-season draft commands
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    subcommands("draft_start", "draft_status", "draft_pick", "draft_end"),
    subcommand_required
)]
pub async fn draft(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Start a snake draft with a randomized pick order
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "start",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn draft_start(
    ctx: Context<'_>,
    #[description = "Players in the draft (order will be randomized)"]
    users: Vec<serenity::User>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let user_ids: Vec<u64> = users.iter().map(|u| u.id.get()).collect();
    let message = draft::start_for_guild(ctx.data(), guild_id, user_ids).await?;
    ctx.say(message).await?;
    Ok(())
}

/// Show draft order, whose turn, and remaining teams
#[poise::command(prefix_command, slash_command, guild_only, rename = "status")]
pub async fn draft_status(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;
    let message = draft::status_for_guild(ctx.data(), guild_id).await?;
    ctx.say(message).await?;
    Ok(())
}

/// End the draft early and freeze the roster
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "end",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn draft_end(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let message = draft::freeze_for_guild(ctx.data(), guild_id).await?;
    ctx.say(message).await?;
    Ok(())
}

/// Pick a team, or make your pick while on the draft clock
#[poise::command(prefix_command, slash_command, guild_only, rename = "pick")]
pub async fn draft_pick(
    ctx: Context<'_>,
    #[description = "Team name, abbreviation, or code"] team: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let message =
        registration::pick_for_user(ctx.data(), guild_id, ctx.author().id.get(), &team).await?;
    ctx.say(message).await?;
    Ok(())
}
