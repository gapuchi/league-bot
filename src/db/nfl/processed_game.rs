use rusqlite::{Connection, OptionalExtension, params};

pub struct NflProcessedGame;

impl NflProcessedGame {
    pub fn is_processed(conn: &Connection, season_id: i64, game_id: i64) -> rusqlite::Result<bool> {
        conn.query_row(
            "SELECT 1 FROM nfl_processed_games WHERE season_id = ?1 AND game_id = ?2",
            params![season_id, game_id],
            |_| Ok(()),
        )
        .optional()
        .map(|row| row.is_some())
    }

    pub fn mark(conn: &Connection, season_id: i64, game_id: i64) -> rusqlite::Result<()> {
        conn.execute(
            "INSERT OR IGNORE INTO nfl_processed_games (season_id, game_id) VALUES (?1, ?2)",
            params![season_id, game_id],
        )?;
        Ok(())
    }

    pub fn unmark(conn: &Connection, season_id: i64, game_id: i64) -> rusqlite::Result<()> {
        conn.execute(
            "DELETE FROM nfl_processed_games WHERE season_id = ?1 AND game_id = ?2",
            params![season_id, game_id],
        )?;
        Ok(())
    }
}
