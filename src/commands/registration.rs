use poise::serenity_prelude as serenity;
use serenity::Mentionable;

use crate::{
    league::League,
    registration::{self, SeasonTeamsList, UnclaimedTeams},
    types::{Context, Error},
};

use super::helpers::guild_id;

/// Claim a team while the roster is open
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn claim(
    ctx: Context<'_>,
    #[description = "Team name, abbreviation, or code"] team: String,
    #[description = "League (optional when only one season is live)"] league: Option<League>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let message = registration::claim_for_user(
        ctx.data(),
        guild_id,
        league,
        ctx.author().id.get(),
        &team,
    )
    .await?;
    ctx.say(message).await?;
    Ok(())
}

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
    #[description = "League (optional when only one season is live)"] league: Option<League>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let message = registration::assign_for_user(
        ctx.data(),
        guild_id,
        league,
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
    #[description = "League (optional when only one season is live)"] league: Option<League>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let message = registration::unclaim_for_user(
        ctx.data(),
        guild_id,
        league,
        ctx.author().id.get(),
        &team,
    )
    .await?;
    ctx.say(message).await?;
    Ok(())
}

/// Show the teams you have claimed
#[poise::command(prefix_command, slash_command, guild_only, rename = "team")]
pub async fn my_team(
    ctx: Context<'_>,
    #[description = "League (optional when only one season is live)"] league: Option<League>,
) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;
    let message =
        registration::my_team_message(ctx.data(), guild_id, league, ctx.author().id.get())
            .await?;

    ctx.send(
        poise::CreateReply::default()
            .content(message)
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

/// List all team assignments in a season
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn teams(
    ctx: Context<'_>,
    #[description = "League (optional when only one season is live)"] league: Option<League>,
) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;
    match registration::list_season_teams(ctx.data(), guild_id, league).await? {
        SeasonTeamsList::Empty => {
            ctx.say("No teams claimed yet. Use `/claim` to choose a team.")
                .await?;
        }
        SeasonTeamsList::ByUser { title, assignments } => {
            let lines: Vec<String> = assignments
                .iter()
                .map(|(user_id, teams)| {
                    format!("<@{}> — **{}**", user_id, teams.join("**, **"))
                })
                .collect();

            let embed = serenity::CreateEmbed::default()
                .title(title)
                .description(lines.join("\n"));

            ctx.send(poise::CreateReply::default().embed(embed)).await?;
        }
    }

    Ok(())
}

/// List teams nobody has claimed yet
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn undrafted(
    ctx: Context<'_>,
    #[description = "League (optional when only one season is live)"] league: Option<League>,
) -> Result<(), Error> {
    ctx.defer().await?;

    let guild_id = guild_id(&ctx)?;
    match registration::unclaimed_teams(ctx.data(), guild_id, league).await? {
        UnclaimedTeams::AllClaimed => {
            ctx.say("Every team has been claimed.").await?;
        }
        UnclaimedTeams::Available(names) => {
            let embed = serenity::CreateEmbed::default()
                .title("Unclaimed teams")
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
