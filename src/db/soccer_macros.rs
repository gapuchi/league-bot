//! Macros for soccer league tables that only differ by table/type name.

macro_rules! define_processed_match {
    ($name:ident, $table:literal) => {
        pub struct $name;

        impl $name {
            pub fn is_processed(
                conn: &rusqlite::Connection,
                season_id: i64,
                match_id: i64,
            ) -> rusqlite::Result<bool> {
                use rusqlite::{OptionalExtension, params};
                conn.query_row(
                    concat!(
                        "SELECT 1 FROM ",
                        $table,
                        " WHERE season_id = ?1 AND match_id = ?2"
                    ),
                    params![season_id, match_id],
                    |_| Ok(()),
                )
                .optional()
                .map(|row| row.is_some())
            }

            pub fn mark(
                conn: &rusqlite::Connection,
                season_id: i64,
                match_id: i64,
            ) -> rusqlite::Result<()> {
                use rusqlite::params;
                conn.execute(
                    concat!(
                        "INSERT OR IGNORE INTO ",
                        $table,
                        " (season_id, match_id) VALUES (?1, ?2)"
                    ),
                    params![season_id, match_id],
                )?;
                Ok(())
            }

            pub fn unmark(
                conn: &rusqlite::Connection,
                season_id: i64,
                match_id: i64,
            ) -> rusqlite::Result<()> {
                use rusqlite::params;
                conn.execute(
                    concat!(
                        "DELETE FROM ",
                        $table,
                        " WHERE season_id = ?1 AND match_id = ?2"
                    ),
                    params![season_id, match_id],
                )?;
                Ok(())
            }
        }
    };
}

macro_rules! define_player_goal_total {
    ($name:ident, $table:literal) => {
        pub struct $name;

        impl $name {
            pub fn upsert_batch(
                conn: &rusqlite::Connection,
                season_id: i64,
                totals: &[(i64, i64)],
                updated_at: &str,
            ) -> rusqlite::Result<()> {
                use rusqlite::params;
                for (player_id, goals) in totals {
                    conn.execute(
                        concat!(
                            "INSERT INTO ",
                            $table,
                            " (season_id, player_id, goals, updated_at) VALUES (?1, ?2, ?3, ?4) \
                             ON CONFLICT(season_id, player_id) DO UPDATE SET \
                             goals = excluded.goals, updated_at = excluded.updated_at"
                        ),
                        params![season_id, player_id, goals, updated_at],
                    )?;
                }
                Ok(())
            }

            pub fn goals_for_player(
                conn: &rusqlite::Connection,
                season_id: i64,
                player_id: i64,
            ) -> rusqlite::Result<i64> {
                use rusqlite::{OptionalExtension, params};
                conn.query_row(
                    concat!(
                        "SELECT goals FROM ",
                        $table,
                        " WHERE season_id = ?1 AND player_id = ?2"
                    ),
                    params![season_id, player_id],
                    |row| row.get(0),
                )
                .optional()
                .map(|goals| goals.unwrap_or(0))
            }
        }
    };
}

macro_rules! define_tiebreaker_pick {
    ($name:ident, $table:literal) => {
        pub struct $name {
            pub player_id: i64,
            pub player_name: String,
            pub team_name: String,
        }

        impl $name {
            pub fn upsert(
                conn: &rusqlite::Connection,
                season_id: i64,
                user_id: u64,
                player_id: i64,
                player_name: &str,
                team_id: i64,
                team_name: &str,
            ) -> rusqlite::Result<()> {
                use rusqlite::params;
                conn.execute(
                    concat!(
                        "INSERT INTO ",
                        $table,
                        " (season_id, user_id, player_id, player_name, team_id, team_name) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                         ON CONFLICT(season_id, user_id) DO UPDATE SET \
                         player_id = excluded.player_id, \
                         player_name = excluded.player_name, \
                         team_id = excluded.team_id, \
                         team_name = excluded.team_name"
                    ),
                    params![
                        season_id,
                        user_id as i64,
                        player_id,
                        player_name,
                        team_id,
                        team_name,
                    ],
                )?;
                Ok(())
            }

            pub fn get_for_user(
                conn: &rusqlite::Connection,
                season_id: i64,
                user_id: u64,
            ) -> rusqlite::Result<Option<Self>> {
                use rusqlite::{OptionalExtension, params};
                conn.query_row(
                    concat!(
                        "SELECT player_id, player_name, team_name FROM ",
                        $table,
                        " WHERE season_id = ?1 AND user_id = ?2"
                    ),
                    params![season_id, user_id as i64],
                    |row| {
                        Ok($name {
                            player_id: row.get(0)?,
                            player_name: row.get(1)?,
                            team_name: row.get(2)?,
                        })
                    },
                )
                .optional()
            }

            pub fn delete_for_team(
                conn: &rusqlite::Connection,
                season_id: i64,
                user_id: u64,
                team_id: i64,
            ) -> rusqlite::Result<()> {
                use rusqlite::params;
                conn.execute(
                    concat!(
                        "DELETE FROM ",
                        $table,
                        " WHERE season_id = ?1 AND user_id = ?2 AND team_id = ?3"
                    ),
                    params![season_id, user_id as i64, team_id],
                )?;
                Ok(())
            }
        }
    };
}

pub(crate) use define_player_goal_total;
pub(crate) use define_processed_match;
pub(crate) use define_tiebreaker_pick;
