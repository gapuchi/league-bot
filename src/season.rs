use crate::{
    db::{Season, SeasonDisplay},
    types::{Data, Error},
};

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
