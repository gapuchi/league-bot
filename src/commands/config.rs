use poise::serenity_prelude as serenity;
use serenity::Mentionable;

use crate::{
    db::{GuildConfig, Season, SeasonDisplay, league_exists},
    league::League,
    types::{Context, Error},
};

use super::helpers::guild_id;

/// Set the active league for commands
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "league",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn config_league(
    ctx: Context<'_>,
    #[description = "League slug (e.g. wc)"] league: String,
) -> Result<(), Error> {
    let slug = league.trim().to_lowercase();

    {
        let conn = ctx.data().db.lock().await;
        if !league_exists(&conn, &slug)? {
            ctx.say(format!("Unknown league \"{slug}\".")).await?;
            return Ok(());
        }
    }

    if !League::supports_season(&slug) {
        ctx.say(format!("League \"{slug}\" is not supported yet.")).await?;
        return Ok(());
    }

    let message = {
        let conn = ctx.data().db.lock().await;
        let guild_id = guild_id(&ctx)?;
        let Some(season) = Season::get_for_guild_league(&conn, guild_id, &slug)? else {
            ctx.say(format!(
                "No season for \"{slug}\" in this server. Use `/season start` to create one."
            ))
            .await?;
            return Ok(());
        };
        GuildConfig::set_default_season_id(&conn, guild_id, season.id)?;
        let display = SeasonDisplay::for_season(&conn, season.id)?;
        format!(
            "Active league set to **{}** — tracking **{}** (`{}`).",
            display.league_name, display.name, display.slug
        )
    };

    ctx.say(message).await?;
    Ok(())
}

/// List configured seasons
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "leagues",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn config_leagues(ctx: Context<'_>) -> Result<(), Error> {
    let (default_season_id, seasons) = {
        let conn = ctx.data().db.lock().await;
        let guild_id = guild_id(&ctx)?;
        let default_season_id =
            GuildConfig::get(&conn, guild_id)?.map(|config| config.default_season_id);
        let seasons = Season::list_with_league(&conn, guild_id)?;
        (default_season_id, seasons)
    };

    if seasons.is_empty() {
        ctx.say("No seasons configured. Use `/season start` to create one.")
            .await?;
        return Ok(());
    }

    let lines: Vec<String> = seasons
        .iter()
        .map(|entry| {
            let active = default_season_id == Some(entry.season.id);
            let marker = if active { " (active)" } else { "" };
            format!(
                "**{}** — **{}** (`{}`){marker}",
                entry.league_name, entry.season.name, entry.season.slug
            )
        })
        .collect();

    ctx.say(lines.join("\n")).await?;
    Ok(())
}

/// Set the channel for match result announcements
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "channel",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn config_channel(
    ctx: Context<'_>,
    #[description = "Channel for match result announcements"] channel: serenity::GuildChannel,
) -> Result<(), Error> {
    let channel_id = channel.id.get();

    {
        let conn = ctx.data().db.lock().await;
        let guild_id = guild_id(&ctx)?;
        let season = Season::default_for_guild(&conn, guild_id)?;
        Season::set_announce_channel(&conn, season.id, channel_id)?;
    }

    ctx.say(format!(
        "Match announcements will be posted in {}.",
        channel.mention()
    ))
    .await?;
    Ok(())
}

/// Server configuration
#[poise::command(
    prefix_command,
    slash_command,
    subcommands("config_channel", "config_league", "config_leagues")
)]
pub async fn config(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}
