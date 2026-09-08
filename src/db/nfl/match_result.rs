use rusqlite::{Connection, OptionalExtension, params};

use crate::scoring::FinishedMatch;

pub struct NflMatchResult {
    pub season_id: i64,
    pub game_id: i64,
    pub home_team_id: i64,
    pub away_team_id: i64,
    pub home_score: i64,
    pub away_score: i64,
    pub week: Option<i64>,
}

impl NflMatchResult {
    pub fn upsert(&self, conn: &Connection) -> rusqlite::Result<()> {
        conn.execute(
            "
            INSERT INTO nfl_match_results (
                season_id, game_id, home_team_id, away_team_id, home_score, away_score, week
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(season_id, game_id) DO UPDATE SET
                home_team_id = excluded.home_team_id,
                away_team_id = excluded.away_team_id,
                home_score = excluded.home_score,
                away_score = excluded.away_score,
                week = excluded.week
            ",
            params![
                self.season_id,
                self.game_id,
                self.home_team_id,
                self.away_team_id,
                self.home_score,
                self.away_score,
                self.week,
            ],
        )?;
        Ok(())
    }

    pub fn list_for_season(conn: &Connection, season_id: i64) -> rusqlite::Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "
            SELECT season_id, game_id, home_team_id, away_team_id, home_score, away_score, week
            FROM nfl_match_results
            WHERE season_id = ?1
            ",
        )?;
        let rows = stmt.query_map(params![season_id], |row| {
            Ok(NflMatchResult {
                season_id: row.get(0)?,
                game_id: row.get(1)?,
                home_team_id: row.get(2)?,
                away_team_id: row.get(3)?,
                home_score: row.get(4)?,
                away_score: row.get(5)?,
                week: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    pub fn score(
        conn: &Connection,
        season_id: i64,
        game_id: i64,
    ) -> rusqlite::Result<Option<(i64, i64)>> {
        conn.query_row(
            "
            SELECT home_score, away_score
            FROM nfl_match_results
            WHERE season_id = ?1 AND game_id = ?2
            ",
            params![season_id, game_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
    }

    pub fn as_finished_match(&self) -> FinishedMatch {
        FinishedMatch {
            home_team_id: self.home_team_id,
            away_team_id: self.away_team_id,
            home_goals: self.home_score,
            away_goals: self.away_score,
        }
    }
}
