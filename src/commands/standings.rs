use poise::serenity_prelude as serenity;

use crate::{
    league::League,
    standings::{
        format_standings_detail_lines, format_standings_summary_lines, standings_footer,
        standings_ranks,
    },
    types::{Context, Error},
};

use super::helpers::guild_id;

/// Show the points leaderboard
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn standings(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;
    let (league_name, rows) = {
        let conn = ctx.data().db.lock().await;
        let (season, league) = League::for_guild(&conn, guild_id)?;
        (league.display_name(), league.standings(&conn, season.id)?)
    };

    if rows.is_empty() {
        ctx.say("No standings yet — pick teams with `/draft pick` first.")
            .await?;
        return Ok(());
    }

    let footer = standings_footer();
    let ranks = standings_ranks(&rows);
    let summary_lines = format_standings_summary_lines(&rows, &ranks);
    let detail_lines = format_standings_detail_lines(&rows, &ranks);

    let summary_embed = serenity::CreateEmbed::default()
        .title(format!("{league_name} standings"))
        .description(summary_lines.join("\n"))
        .footer(serenity::CreateEmbedFooter::new(&footer));

    let reply = ctx
        .send(poise::CreateReply::default().embed(summary_embed))
        .await?;
    let message = reply.into_message().await?;

    let detail_embed = serenity::CreateEmbed::default()
        .title("Standings breakdown")
        .description(detail_lines.join("\n\n"))
        .footer(serenity::CreateEmbedFooter::new(footer));

    let thread = message
        .channel_id
        .create_thread_from_message(
            ctx.serenity_context(),
            message.id,
            serenity::CreateThread::new("Standings breakdown"),
        )
        .await?;

    thread
        .id
        .send_message(
            ctx.serenity_context(),
            serenity::CreateMessage::new().embed(detail_embed),
        )
        .await?;

    Ok(())
}
