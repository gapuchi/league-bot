use rusqlite::{Connection, OptionalExtension, params};

use crate::scoring::FinishedMatch;

pub struct EplMatchResult {
    pub season_id: i64,
    pub match_id: i64,
    pub home_team_id: i64,
    pub away_team_id: i64,
    pub home_goals: i64,
    pub away_goals: i64,
    pub matchday: Option<i64>,
}

impl EplMatchResult {
    pub fn upsert(&self, conn: &Connection) -> rusqlite::Result<()> {
        conn.execute(
            "
            INSERT INTO epl_match_results (
                season_id, match_id, home_team_id, away_team_id, home_goals, away_goals, matchday
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(season_id, match_id) DO UPDATE SET
                home_team_id = excluded.home_team_id,
                away_team_id = excluded.away_team_id,
                home_goals = excluded.home_goals,
                away_goals = excluded.away_goals,
                matchday = excluded.matchday
            ",
            params![
                self.season_id,
                self.match_id,
                self.home_team_id,
                self.away_team_id,
                self.home_goals,
                self.away_goals,
                self.matchday,
            ],
        )?;
        Ok(())
    }

    pub fn list_for_season(conn: &Connection, season_id: i64) -> rusqlite::Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "
            SELECT season_id, match_id, home_team_id, away_team_id, home_goals, away_goals, matchday
            FROM epl_match_results
            WHERE season_id = ?1
            ",
        )?;
        let rows = stmt.query_map(params![season_id], |row| {
            Ok(EplMatchResult {
                season_id: row.get(0)?,
                match_id: row.get(1)?,
                home_team_id: row.get(2)?,
                away_team_id: row.get(3)?,
                home_goals: row.get(4)?,
                away_goals: row.get(5)?,
                matchday: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    pub fn score(
        conn: &Connection,
        season_id: i64,
        match_id: i64,
    ) -> rusqlite::Result<Option<(i64, i64)>> {
        conn.query_row(
            "
            SELECT home_goals, away_goals
            FROM epl_match_results
            WHERE season_id = ?1 AND match_id = ?2
            ",
            params![season_id, match_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
    }

    pub fn as_finished_match(&self) -> FinishedMatch {
        FinishedMatch {
            home_team_id: self.home_team_id,
            away_team_id: self.away_team_id,
            home_goals: self.home_goals,
            away_goals: self.away_goals,
            playoff: false,
        }
    }
}
