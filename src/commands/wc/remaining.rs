use poise::serenity_prelude as serenity;

use crate::{
    types::{Context, Error},
    wc::remaining::{self, RemainingResult},
};

use super::super::helpers::guild_id;

/// List World Cup teams still in the tournament
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn remaining(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let guild_id = guild_id(&ctx)?;
    match remaining::list_for_guild(ctx.data(), guild_id).await? {
        RemainingResult::NoRegistrations => {
            ctx.say("No teams claimed yet. Use `/claim` to choose a nation.")
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
