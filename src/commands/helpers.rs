use crate::{
    league::League,
    types::{Context, Error},
};

pub(crate) fn guild_id(ctx: &Context<'_>) -> Result<u64, Error> {
    Ok(ctx
        .guild_id()
        .ok_or("This command must be used in a server.")?
        .get())
}

/// User ids from free text: `<@123>` / `<@!123>` mentions or bare snowflakes, in order
/// of appearance. Slash text options deliver `@`-autocompleted members as `<@id>`.
pub(crate) fn parse_user_ids(text: &str) -> Vec<u64> {
    text.split_whitespace()
        .filter_map(|token| {
            let token = token.trim_matches(|c| c == ',' || c == ';');
            let digits = token
                .strip_prefix("<@")
                .and_then(|rest| rest.strip_suffix('>'))
                .map(|inner| inner.trim_start_matches('!'))
                .unwrap_or(token);
            digits.parse::<u64>().ok()
        })
        .collect()
}

/// Ensures command focus is `expected`. On mismatch, replies and returns `false`.
pub(crate) async fn ensure_focused_league(
    ctx: &Context<'_>,
    expected: League,
) -> Result<bool, Error> {
    let guild_id = guild_id(ctx)?;
    let league = {
        let conn = ctx.data().db.lock().await;
        let (_, league) = League::for_guild(&conn, guild_id)?;
        league
    };
    if league != expected {
        ctx.say(format!(
            "This command is only available when the active season is {}. Use `/season status` to check, or `/config league` to switch.",
            expected.display_name()
        ))
        .await?;
        return Ok(false);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::parse_user_ids;

    #[test]
    fn parse_user_ids_accepts_mentions_and_raw_ids() {
        assert_eq!(
            parse_user_ids("<@111> <@!222>, 333 @not-an-id"),
            vec![111, 222, 333]
        );
        assert!(parse_user_ids("").is_empty());
        assert!(parse_user_ids("alice bob").is_empty());
    }
}
