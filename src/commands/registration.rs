use poise::serenity_prelude as serenity;
use serenity::Mentionable;

use crate::{
    registration::{self, SeasonTeamsList, UnclaimedTeams},
    types::{Context, Error},
};

use super::helpers::guild_id;

/// Admin: claim a team for another member (draft: on-clock player only)
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    required_permissions = "MANAGE_GUILD"
)]
pub async fn assign(
    ctx: Context<'_>,
    #[description = "Member to claim the team for"] user: serenity::Member,
    #[description = "Team name, abbreviation, or code"] team: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let message = registration::assign_for_user(
        ctx.data(),
        guild_id,
        user.user.id.get(),
        &team,
        &user.mention().to_string(),
    )
    .await?;
    ctx.say(message).await?;
    Ok(())
}

/// Remove a claimed team
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn unclaim(
    ctx: Context<'_>,
    #[description = "Team name, abbreviation, or code"] team: String,
) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;
    let message = registration::unclaim_for_user(
        ctx.data(),
        guild_id,
        ctx.author().id.get(),
        &team,
    )
    .await?;
    ctx.say(message).await?;
    Ok(())
}

/// Show the teams you have claimed
#[poise::command(prefix_command, slash_command, guild_only, rename = "team")]
pub async fn my_team(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;
    let message =
        registration::my_team_message(ctx.data(), guild_id, ctx.author().id.get()).await?;

    ctx.send(
        poise::CreateReply::default()
            .content(message)
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

/// List all team assignments in this server
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn teams(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;
    match registration::list_season_teams(ctx.data(), guild_id).await? {
        SeasonTeamsList::Empty => {
            ctx.say("No teams picked yet. Use `/draft pick` to choose a team.")
                .await?;
        }
        SeasonTeamsList::ByUser {
            league_name,
            assignments,
        } => {
            let lines: Vec<String> = assignments
                .iter()
                .map(|(user_id, teams)| {
                    format!("<@{}> — **{}**", user_id, teams.join("**, **"))
                })
                .collect();

            let embed = serenity::CreateEmbed::default()
                .title(format!("{league_name} team assignments"))
                .description(lines.join("\n"));

            ctx.send(poise::CreateReply::default().embed(embed)).await?;
        }
    }

    Ok(())
}

/// List teams that have not been drafted yet
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn undrafted(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let guild_id = guild_id(&ctx)?;
    match registration::unclaimed_teams(ctx.data(), guild_id).await? {
        UnclaimedTeams::AllClaimed => {
            ctx.say("Every team has been drafted.").await?;
        }
        UnclaimedTeams::Available(names) => {
            let embed = serenity::CreateEmbed::default()
                .title("Undrafted teams")
                .description(
                    names
                        .iter()
                        .map(|name| format!("**{name}**"))
                        .collect::<Vec<_>>()
                        .join(", "),
                );

            ctx.send(poise::CreateReply::default().embed(embed)).await?;
        }
    }

    Ok(())
}
