use rusqlite::{Connection, OptionalExtension, params};

pub struct NflPlayerTouchdownTotal;

impl NflPlayerTouchdownTotal {
    /// `totals` is `(player_id, touchdowns)` pairs.
    pub fn upsert_batch(
        conn: &Connection,
        season_id: i64,
        totals: &[(i64, i64)],
        updated_at: &str,
    ) -> rusqlite::Result<()> {
        for (player_id, touchdowns) in totals {
            conn.execute(
                "
                INSERT INTO nfl_player_touchdown_totals (season_id, player_id, touchdowns, updated_at)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(season_id, player_id) DO UPDATE SET
                    touchdowns = excluded.touchdowns,
                    updated_at = excluded.updated_at
                ",
                params![season_id, player_id, touchdowns, updated_at],
            )?;
        }
        Ok(())
    }

    pub fn touchdowns_for_player(
        conn: &Connection,
        season_id: i64,
        player_id: i64,
    ) -> rusqlite::Result<i64> {
        conn.query_row(
            "
            SELECT touchdowns
            FROM nfl_player_touchdown_totals
            WHERE season_id = ?1 AND player_id = ?2
            ",
            params![season_id, player_id],
            |row| row.get(0),
        )
        .optional()
        .map(|touchdowns| touchdowns.unwrap_or(0))
    }
}
