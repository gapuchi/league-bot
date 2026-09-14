use crate::{
    draft,
    league::League,
    types::{Context, Error},
};

use super::helpers::{guild_id, parse_user_ids};

/// Pre-season snake draft
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    subcommands(
        "draft_start",
        "draft_status",
        "draft_pick",
        "draft_unpick",
        "draft_end",
        "draft_cancel"
    ),
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
    #[description = "Mention every player, e.g. @alice @bob @carol (order will be randomized)"]
    players: String,
    #[description = "League (optional when only one season is live)"] league: Option<League>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let user_ids = parse_user_ids(&players);
    if user_ids.is_empty() {
        ctx.say("Mention the players to include, e.g. `/draft start players: @alice @bob @carol`.")
            .await?;
        return Ok(());
    }
    let message = draft::start_for_guild(ctx.data(), guild_id, league, user_ids).await?;
    ctx.say(message).await?;
    Ok(())
}

/// Show draft order, whose turn, and remaining teams
#[poise::command(prefix_command, slash_command, guild_only, rename = "status")]
pub async fn draft_status(
    ctx: Context<'_>,
    #[description = "League (optional when only one season is live)"] league: Option<League>,
) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;
    let message = draft::status_for_guild(ctx.data(), guild_id, league).await?;
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
pub async fn draft_end(
    ctx: Context<'_>,
    #[description = "League (optional when only one season is live)"] league: Option<League>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let message = draft::freeze_for_guild(ctx.data(), guild_id, league).await?;
    ctx.say(message).await?;
    Ok(())
}

/// Cancel the draft, clear its picks, and reopen the roster
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "cancel",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn draft_cancel(
    ctx: Context<'_>,
    #[description = "League (optional when only one season is live)"] league: Option<League>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let message = draft::cancel_for_guild(ctx.data(), guild_id, league).await?;
    ctx.say(message).await?;
    Ok(())
}

/// Make your pick while on the draft clock
#[poise::command(prefix_command, slash_command, guild_only, rename = "pick")]
pub async fn draft_pick(
    ctx: Context<'_>,
    #[description = "Team name, abbreviation, or code"] team: String,
    #[description = "League (optional when only one season is live)"] league: Option<League>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let message =
        draft::pick_for_user(ctx.data(), guild_id, league, ctx.author().id.get(), &team)
            .await?
            .into_message();
    ctx.say(message).await?;
    Ok(())
}

/// Undo your most recent draft pick (only before the next person picks)
#[poise::command(prefix_command, slash_command, guild_only, rename = "unpick")]
pub async fn draft_unpick(
    ctx: Context<'_>,
    #[description = "League (optional when only one season is live)"] league: Option<League>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let message =
        draft::unpick_for_user(ctx.data(), guild_id, league, ctx.author().id.get()).await?;
    ctx.say(message).await?;
    Ok(())
}
