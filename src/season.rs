//! Season lifecycle and the one rule every gameplay command shares: which season a
//! command targets. There is no per-guild "current season" state — callers pass the
//! league explicitly, or it is inferred when the guild has exactly one live season.

use rusqlite::Connection;

use crate::{
    clock,
    db::{Season, SeasonMeta},
    league::League,
    types::{Data, Error, UserError},
};

/// Resolve the season a command targets.
///
/// `Some(league)` picks that league's live season, falling back to its newest ended one
/// so history stays reachable. `None` requires the guild to have exactly one live
/// season; otherwise the user is told which leagues to choose from.
pub fn resolve(
    conn: &Connection,
    guild_id: u64,
    league: Option<League>,
) -> Result<(Season, League), Error> {
    if let Some(league) = league {
        let meta = Season::latest_for_guild_league(conn, guild_id, league.slug())?.ok_or_else(
            || {
                UserError(format!(
                    "No {} season in this server. An admin can `/season start`.",
                    league.display_name()
                ))
            },
        )?;
        return Ok((meta.season, league));
    }

    let mut live = Season::list_live_for_guild(conn, guild_id)?;
    match live.len() {
        0 => Err(UserError(
            "No live season in this server. An admin can `/season start`; pass `league` to view an ended season."
                .into(),
        )
        .into()),
        1 => {
            let meta = live.remove(0);
            let league = League::for_season(conn, meta.season.id)?;
            Ok((meta.season, league))
        }
        _ => {
            let names: Vec<&str> = live.iter().map(|meta| meta.league_name.as_str()).collect();
            Err(UserError(format!(
                "Which league? This server has live seasons for {}. Add the `league` option.",
                names.join(", ")
            ))
            .into())
        }
    }
}

/// Lowercase, hyphen-separated form of a display name, used as the unique season key.
pub fn slugify(name: &str) -> String {
    let mut slug = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    slug.trim_end_matches('-').to_owned()
}

/// Create (or resume) a season and make it the league's only live one in the guild.
pub async fn start_for_guild(
    data: &Data,
    guild_id: u64,
    league: League,
    name: Option<&str>,
) -> Result<String, Error> {
    let name = match name.map(str::trim) {
        Some(name) if !name.is_empty() => name.to_owned(),
        _ => format!("{} {}", league.display_name(), clock::current_year()),
    };
    let slug = slugify(&name);
    if slug.is_empty() {
        return Ok("Season name needs at least one letter or digit.".into());
    }

    let conn = data.db.lock().await;
    let existed = Season::get_by_guild_league_slug(&conn, guild_id, league.slug(), &slug)?;
    let season = Season::get_or_create(&conn, guild_id, league.slug(), &slug, &name)?;

    let ended: Vec<SeasonMeta> = Season::list_live_for_guild(&conn, guild_id)?
        .into_iter()
        .filter(|meta| meta.league_slug == league.slug() && meta.season.id != season.id)
        .collect();
    for meta in &ended {
        Season::set_polling_enabled(&conn, meta.season.id, false)?;
    }
    let ended_line = match ended.as_slice() {
        [] => String::new(),
        [prior] => format!(" Ended **{}**.", prior.season.name),
        many => format!(
            " Ended {}.",
            many.iter()
                .map(|meta| format!("**{}**", meta.season.name))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };

    Ok(match existed {
        None => format!(
            "Started **{}** (`{slug}`) for {}. Match polling enabled.{ended_line} Set `/season channel` for announcements.",
            season.name,
            league.display_name()
        ),
        Some(_) if !season.polling_enabled => {
            Season::set_polling_enabled(&conn, season.id, true)?;
            format!(
                "Resumed **{}** (`{slug}`) — match polling enabled.{ended_line}",
                season.name
            )
        }
        Some(_) => format!("**{}** (`{slug}`) is already running.", season.name),
    })
}

pub async fn end_for_guild(
    data: &Data,
    guild_id: u64,
    league: Option<League>,
) -> Result<String, Error> {
    let conn = data.db.lock().await;
    let (season, _) = resolve(&conn, guild_id, league)?;
    if !season.polling_enabled {
        return Ok(format!(
            "**{}** (`{}`) is already ended — match polling is off.",
            season.name, season.slug
        ));
    }
    Season::set_polling_enabled(&conn, season.id, false)?;
    Ok(format!(
        "Ended **{}** (`{}`) — match polling stopped.",
        season.name, season.slug
    ))
}

pub async fn set_channel_for_guild(
    data: &Data,
    guild_id: u64,
    league: Option<League>,
    channel_id: u64,
) -> Result<String, Error> {
    let conn = data.db.lock().await;
    let (season, _) = resolve(&conn, guild_id, league)?;
    Season::set_announce_channel(&conn, season.id, channel_id)?;
    Ok(format!(
        "**{}** announcements will be posted in <#{channel_id}>.",
        season.name
    ))
}

pub async fn list_for_guild(data: &Data, guild_id: u64) -> Result<String, Error> {
    let conn = data.db.lock().await;
    let seasons = Season::list_for_guild(&conn, guild_id)?;
    if seasons.is_empty() {
        return Ok("No seasons in this server yet. An admin can `/season start`.".into());
    }
    Ok(seasons.iter().map(describe).collect::<Vec<_>>().join("\n"))
}

fn describe(meta: &SeasonMeta) -> String {
    let season = &meta.season;
    let state = if season.polling_enabled { "live" } else { "ended" };
    let channel = match season.announce_channel_id {
        Some(id) => format!("<#{id}>"),
        None => "no channel".into(),
    };
    format!(
        "**{}** — **{}** (`{}`) · {state} · roster {} · {channel}",
        meta.league_name,
        season.name,
        season.slug,
        season.roster_phase.as_str()
    )
}

#[cfg(test)]
mod tests {
    use super::slugify;

    #[test]
    fn slugify_collapses_punctuation_and_case() {
        assert_eq!(slugify("NFL 2026"), "nfl-2026");
        assert_eq!(slugify("Premier League 2026/27"), "premier-league-2026-27");
        assert_eq!(slugify("  World Cup!! 2026  "), "world-cup-2026");
        assert_eq!(slugify("!!!"), "");
    }
}
