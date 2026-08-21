use poise::serenity_prelude as serenity;

use crate::{
    league::League,
    remaining::{self, RemainingResult},
    types::{Context, Error},
};

use super::super::helpers::{ensure_focused_league, guild_id};

/// List World Cup teams still in the tournament
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn remaining(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    if !ensure_focused_league(&ctx, League::Wc).await? {
        return Ok(());
    }

    let guild_id = guild_id(&ctx)?;
    match remaining::list_for_guild(ctx.data(), guild_id).await? {
        RemainingResult::NotWorldCup => {
            ctx.say("This command is only available when the active season is World Cup. Use `/season status` to check, or `/config league` to switch.")
                .await?;
        }
        RemainingResult::NoRegistrations => {
            ctx.say("No teams assigned yet. Use `/draft pick` to choose a nation.")
                .await?;
        }
        RemainingResult::Report(report) => {
            let embed = serenity::CreateEmbed::default()
                .title("World Cup teams remaining")
                .description(remaining::format_grouped_field(
                    &report.still_in_by_user,
                    &report.unassigned_still_in,
                ));

            ctx.send(poise::CreateReply::default().embed(embed)).await?;
        }
    }

    Ok(())
}
