use rusqlite::{Connection, OptionalExtension, params};

use super::{season::Season, team};

pub struct Registration {
    pub user_id: u64,
    pub team_id: i64,
    pub team_name: String,
}

impl Registration {
    pub fn upsert(
        conn: &Connection,
        season_id: i64,
        user_id: u64,
        team_id: i64,
        team_name: &str,
    ) -> rusqlite::Result<()> {
        let league_id = Season::league_id_for(conn, season_id)?;
        team::upsert_name(conn, league_id, team_id, team_name)?;
        conn.execute(
            "
            INSERT INTO registrations (season_id, user_id, team_id, team_name)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(season_id, team_id) DO UPDATE SET
                user_id = excluded.user_id,
                team_name = excluded.team_name
            ",
            params![season_id, user_id as i64, team_id, team_name],
        )?;
        Ok(())
    }

    pub fn delete(
        conn: &Connection,
        season_id: i64,
        user_id: u64,
        team_id: i64,
    ) -> rusqlite::Result<bool> {
        let changed = conn.execute(
            "DELETE FROM registrations WHERE season_id = ?1 AND user_id = ?2 AND team_id = ?3",
            params![season_id, user_id as i64, team_id],
        )?;
        Ok(changed > 0)
    }

    pub fn get_by_team(
        conn: &Connection,
        season_id: i64,
        team_id: i64,
    ) -> rusqlite::Result<Option<Self>> {
        conn.query_row(
            "
            SELECT user_id, team_id, team_name
            FROM registrations
            WHERE season_id = ?1 AND team_id = ?2
            ",
            params![season_id, team_id],
            |row| {
                Ok(Registration {
                    user_id: row.get::<_, i64>(0)? as u64,
                    team_id: row.get(1)?,
                    team_name: row.get(2)?,
                })
            },
        )
        .optional()
    }

    pub fn list_for_season(conn: &Connection, season_id: i64) -> rusqlite::Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "
            SELECT user_id, team_id, team_name
            FROM registrations
            WHERE season_id = ?1
            ORDER BY team_name
            ",
        )?;
        let rows = stmt.query_map(params![season_id], |row| {
            Ok(Registration {
                user_id: row.get::<_, i64>(0)? as u64,
                team_id: row.get(1)?,
                team_name: row.get(2)?,
            })
        })?;
        rows.collect()
    }

    /// Most recently inserted registration for the season (draft pick order).
    pub fn latest_for_season(
        conn: &Connection,
        season_id: i64,
    ) -> rusqlite::Result<Option<Self>> {
        conn.query_row(
            "
            SELECT user_id, team_id, team_name
            FROM registrations
            WHERE season_id = ?1
            ORDER BY rowid DESC
            LIMIT 1
            ",
            params![season_id],
            |row| {
                Ok(Registration {
                    user_id: row.get::<_, i64>(0)? as u64,
                    team_id: row.get(1)?,
                    team_name: row.get(2)?,
                })
            },
        )
        .optional()
    }

    pub fn list_for_user(
        conn: &Connection,
        season_id: i64,
        user_id: u64,
    ) -> rusqlite::Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "
            SELECT user_id, team_id, team_name
            FROM registrations
            WHERE season_id = ?1 AND user_id = ?2
            ORDER BY team_name
            ",
        )?;
        let rows = stmt.query_map(params![season_id, user_id as i64], |row| {
            Ok(Registration {
                user_id: row.get::<_, i64>(0)? as u64,
                team_id: row.get(1)?,
                team_name: row.get(2)?,
            })
        })?;
        rows.collect()
    }
}
