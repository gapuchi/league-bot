use poise::serenity_prelude as serenity;
use serenity::Mentionable;

use crate::{
    db::{GuildConfig, Season, SeasonDisplay, league_exists},
    league::League,
    season,
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
                "No season for \"{slug}\" in this server. Use `/config season` to create one."
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
        ctx.say("No seasons configured. Use `/config season` to create one.")
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

/// Create a season for this server
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "season",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn config_season(
    ctx: Context<'_>,
    #[description = "League slug (e.g. wc)"] league: String,
    #[description = "Season slug (e.g. wc-2026)"] slug: String,
    #[description = "Display name (e.g. World Cup 2026)"] name: String,
) -> Result<(), Error> {
    let league_slug = league.trim().to_lowercase();
    let season_slug = slug.trim().to_lowercase();
    let season_name = name.trim();

    if season_name.is_empty() {
        ctx.say("Season name is required.").await?;
        return Ok(());
    }

    {
        let conn = ctx.data().db.lock().await;
        if !league_exists(&conn, &league_slug)? {
            ctx.say(format!("Unknown league \"{league_slug}\".")).await?;
            return Ok(());
        }
    }

    if !League::supports_season(&league_slug) {
        ctx.say(format!("League \"{league_slug}\" is not supported yet.")).await?;
        return Ok(());
    }

    let message = {
        let conn = ctx.data().db.lock().await;
        let guild_id = guild_id(&ctx)?;
        let season = Season::get_or_create(
            &conn,
            guild_id,
            &league_slug,
            &season_slug,
            season_name,
        )?;
        GuildConfig::set_default_season_id(&conn, guild_id, season.id)?;
        let display = SeasonDisplay::for_season(&conn, season.id)?;
        format!(
            "Created season for **{}** — tracking **{}** (`{}`). Set `/config channel` for announcements.",
            display.league_name, display.name, display.slug
        )
    };

    ctx.say(message).await?;
    Ok(())
}

/// End the focused season (stop match polling)
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "season-end",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn config_season_end(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let message = season::end_for_guild(ctx.data(), guild_id).await?;
    ctx.say(message).await?;
    Ok(())
}

/// Server configuration
#[poise::command(
    prefix_command,
    slash_command,
    subcommands(
        "config_channel",
        "config_league",
        "config_leagues",
        "config_season",
        "config_season_end"
    )
)]
pub async fn config(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}
