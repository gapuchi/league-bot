use crate::{
    db::{GuildConfig, Season, SeasonDisplay, league_exists},
    league::League,
    types::{Data, Error},
};

pub async fn start_for_guild(
    data: &Data,
    guild_id: u64,
    league_slug: &str,
    season_slug: &str,
    name: &str,
) -> Result<String, Error> {
    let league_slug = league_slug.trim().to_lowercase();
    let season_slug = season_slug.trim().to_lowercase();
    let name = name.trim();

    if name.is_empty() {
        return Ok("Season name is required.".into());
    }

    {
        let conn = data.db.lock().await;
        if !league_exists(&conn, &league_slug)? {
            return Ok(format!("Unknown league \"{league_slug}\"."));
        }
    }

    if !League::supports_season(&league_slug) {
        return Ok(format!("League \"{league_slug}\" is not supported yet."));
    }

    let message = {
        let conn = data.db.lock().await;
        let existed =
            Season::get_by_guild_league_slug(&conn, guild_id, &league_slug, &season_slug)?;
        let season = Season::get_or_create(&conn, guild_id, &league_slug, &season_slug, name)?;
        GuildConfig::set_default_season_id(&conn, guild_id, season.id)?;
        let display = SeasonDisplay::for_season(&conn, season.id)?;

        if existed.is_none() {
            format!(
                "Started season for **{}** — tracking **{}** (`{}`). Match polling enabled. Set `/config channel` for announcements.",
                display.league_name, display.name, display.slug
            )
        } else if !season.polling_enabled {
            Season::set_polling_enabled(&conn, season.id, true)?;
            format!(
                "Resumed season **{}** (`{}`) — match polling enabled.",
                display.name, display.slug
            )
        } else {
            format!(
                "Season **{}** (`{}`) is already running.",
                display.name, display.slug
            )
        }
    };

    Ok(message)
}

/// Stop match polling for the guild's command-focus season.
pub async fn end_for_guild(data: &Data, guild_id: u64) -> Result<String, Error> {
    let message = {
        let conn = data.db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        let display = SeasonDisplay::for_season(&conn, season.id)?;
        if !season.polling_enabled {
            return Ok(format!(
                "Season **{}** (`{}`) is already ended — match polling is off.",
                display.name, display.slug
            ));
        }
        Season::set_polling_enabled(&conn, season.id, false)?;
        format!(
            "Ended season **{}** (`{}`) — match polling stopped.",
            display.name, display.slug
        )
    };

    Ok(message)
}

pub async fn status_for_guild(data: &Data, guild_id: u64) -> Result<String, Error> {
    let conn = data.db.lock().await;
    let season = Season::default_for_guild(&conn, guild_id)?;
    let display = SeasonDisplay::for_season(&conn, season.id)?;
    let polling = if season.polling_enabled {
        "match polling **on**"
    } else {
        "match polling **off**"
    };
    Ok(format!(
        "This bot is tracking **{}** (`{}`) for **{}** — {polling}.",
        display.name, display.slug, display.league_name
    ))
}
