use poise::serenity_prelude as serenity;
use rusqlite::Connection;

use crate::{
    api::Match,
    db::{
        EplMatchResult, EplPlayerGoalTotal, EplProcessedMatch, EplTiebreakerPick, Registration,
        Season, SeasonMeta, WcMatchResult, WcPlayerGoalTotal, WcProcessedMatch, WcTiebreakerPick,
    },
    epl,
    scoring::FinishedMatch,
    soccer::full_time_score,
    standings::{self, StandingRow},
    types::{Data, Error},
    wc,
};

/// Summary returned by [`League::poll`] for host logging.
#[derive(Debug, Clone)]
pub struct PollOutcome {
    pub finished_matches: usize,
    pub scored_matches: usize,
    pub seasons: usize,
    pub detail: String,
}

/// Compile-time league types that this binary can run.
///
/// Adding a league is a code change: new variant, league module, and `match` arms.
/// Runtime guild setup creates **seasons** for a compiled-in league; it does not
/// register new leagues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum League {
    Wc,
    Epl,
}

/// Team from a league's catalog (registration / unclaimed lists).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogTeam {
    pub id: i64,
    pub name: String,
    pub short_name: Option<String>,
    pub code: Option<String>,
}

impl CatalogTeam {
    pub fn from_api(team: crate::api::Team) -> Self {
        Self {
            id: team.id,
            name: team.name,
            short_name: team.short_name,
            code: team.tla,
        }
    }
}

impl League {
    pub const ALL: &[League] = &[League::Wc, League::Epl];

    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "wc" => Some(Self::Wc),
            "epl" => Some(Self::Epl),
            _ => None,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Wc => "wc",
            Self::Epl => "epl",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Wc => "FIFA World Cup",
            Self::Epl => "Premier League",
        }
    }

    /// Whether `/season start` (and related setup) may target this slug.
    pub fn supports_season(slug: &str) -> bool {
        Self::from_slug(slug).is_some()
    }

    pub fn for_season(conn: &Connection, season_id: i64) -> Result<Self, Error> {
        let slug = Season::league_slug_for(conn, season_id)?;
        Self::from_slug(&slug).ok_or_else(|| {
            format!("season {season_id} uses league \"{slug}\" which is not compiled into this bot")
                .into()
        })
    }

    pub fn for_guild(conn: &Connection, guild_id: u64) -> Result<(Season, Self), Error> {
        let season = Season::default_for_guild(conn, guild_id)?;
        let league = Self::for_season(conn, season.id)?;
        Ok((season, league))
    }

    pub async fn list_teams(self, data: &Data) -> Result<Vec<CatalogTeam>, Error> {
        let competition = crate::db::league_competition_code(self.slug());
        let teams = crate::api::FootballDataApi::from_env(data.http.clone())
            .fetch_teams(&competition)
            .await?;
        Ok(teams.into_iter().map(CatalogTeam::from_api).collect())
    }

    pub fn find_team<'a>(self, teams: &'a [CatalogTeam], query: &str) -> Option<&'a CatalogTeam> {
        let query = query.trim().to_lowercase();
        teams.iter().find(|team| {
            let name = team.name.to_lowercase();
            name == query
                || team
                    .short_name
                    .as_ref()
                    .is_some_and(|n| n.to_lowercase() == query)
                || team
                    .code
                    .as_ref()
                    .is_some_and(|c| c.to_lowercase() == query)
                || name.contains(&query)
        })
    }

    pub fn team_not_found_message(self, team_query: &str) -> String {
        match self {
            Self::Wc => format!(
                "Couldn't find a World Cup team matching \"{team_query}\". Try the full name or three-letter code (e.g. BRA)."
            ),
            Self::Epl => format!(
                "Couldn't find a Premier League club matching \"{team_query}\". Try the full name or three-letter code (e.g. LIV)."
            ),
        }
    }

    fn finished_matches(
        self,
        conn: &Connection,
        season_id: i64,
    ) -> rusqlite::Result<Vec<FinishedMatch>> {
        Ok(match self {
            Self::Wc => WcMatchResult::list_for_season(conn, season_id)?
                .iter()
                .map(WcMatchResult::as_finished_match)
                .collect(),
            Self::Epl => EplMatchResult::list_for_season(conn, season_id)?
                .iter()
                .map(EplMatchResult::as_finished_match)
                .collect(),
        })
    }

    fn tiebreaker_for_standings(
        self,
        conn: &Connection,
        season_id: i64,
        user_id: u64,
    ) -> rusqlite::Result<(i64, Option<String>)> {
        match self {
            Self::Wc => {
                let pick = WcTiebreakerPick::get_for_user(conn, season_id, user_id)?;
                let goals = match &pick {
                    Some(pick) => {
                        WcPlayerGoalTotal::goals_for_player(conn, season_id, pick.player_id)?
                    }
                    None => 0,
                };
                Ok((goals, pick.map(|p| p.player_name)))
            }
            Self::Epl => {
                let pick = EplTiebreakerPick::get_for_user(conn, season_id, user_id)?;
                let goals = match &pick {
                    Some(pick) => {
                        EplPlayerGoalTotal::goals_for_player(conn, season_id, pick.player_id)?
                    }
                    None => 0,
                };
                Ok((goals, pick.map(|p| p.player_name)))
            }
        }
    }

    pub fn standings(
        self,
        conn: &Connection,
        season_id: i64,
    ) -> rusqlite::Result<Vec<StandingRow>> {
        standings::build_rows(
            &self.finished_matches(conn, season_id)?,
            &Registration::list_for_season(conn, season_id)?,
            |user_id| self.tiebreaker_for_standings(conn, season_id, user_id),
        )
    }

    pub fn user_points(
        self,
        conn: &Connection,
        season_id: i64,
        user_id: u64,
    ) -> rusqlite::Result<i64> {
        Ok(standings::points_for_user_teams(
            &self.finished_matches(conn, season_id)?,
            &Registration::list_for_user(conn, season_id, user_id)?,
        ))
    }

    pub fn tiebreaker_value_for_user(
        self,
        conn: &Connection,
        season_id: i64,
        user_id: u64,
    ) -> rusqlite::Result<i64> {
        Ok(self.tiebreaker_for_standings(conn, season_id, user_id)?.0)
    }

    /// `(player_name, team_name)` when the user has a tie-breaker pick.
    pub fn tiebreaker_pick_for_user(
        self,
        conn: &Connection,
        season_id: i64,
        user_id: u64,
    ) -> rusqlite::Result<Option<(String, String)>> {
        Ok(match self {
            Self::Wc => WcTiebreakerPick::get_for_user(conn, season_id, user_id)?
                .map(|pick| (pick.player_name, pick.team_name)),
            Self::Epl => EplTiebreakerPick::get_for_user(conn, season_id, user_id)?
                .map(|pick| (pick.player_name, pick.team_name)),
        })
    }

    pub fn clear_picks_for_team(
        self,
        conn: &Connection,
        season_id: i64,
        user_id: u64,
        team_id: i64,
    ) -> rusqlite::Result<()> {
        match self {
            Self::Wc => WcTiebreakerPick::delete_for_team(conn, season_id, user_id, team_id),
            Self::Epl => EplTiebreakerPick::delete_for_team(conn, season_id, user_id, team_id),
        }
    }

    pub async fn pick_tiebreaker_player(
        self,
        data: &Data,
        guild_id: u64,
        user_id: u64,
        player: &str,
    ) -> Result<String, Error> {
        crate::tiebreaker::pick_tiebreaker_player(data, guild_id, user_id, player, |conn,
                                                                                     season_id,
                                                                                     user_id,
                                                                                     selected| {
            match self {
                Self::Wc => WcTiebreakerPick::upsert(
                    conn,
                    season_id,
                    user_id,
                    selected.player_id,
                    &selected.player_name,
                    selected.team_id,
                    &selected.team_name,
                ),
                Self::Epl => EplTiebreakerPick::upsert(
                    conn,
                    season_id,
                    user_id,
                    selected.player_id,
                    &selected.player_name,
                    selected.team_id,
                    &selected.team_name,
                ),
            }
        })
        .await
    }

    pub fn cache_player_goals(
        self,
        conn: &Connection,
        season_id: i64,
        totals: &[(i64, i64)],
        updated_at: &str,
    ) -> rusqlite::Result<()> {
        match self {
            Self::Wc => WcPlayerGoalTotal::upsert_batch(conn, season_id, totals, updated_at),
            Self::Epl => EplPlayerGoalTotal::upsert_batch(conn, season_id, totals, updated_at),
        }
    }

    pub fn stored_match_score(
        self,
        conn: &Connection,
        season_id: i64,
        match_id: i64,
    ) -> rusqlite::Result<Option<(i64, i64)>> {
        match self {
            Self::Wc => WcMatchResult::score(conn, season_id, match_id),
            Self::Epl => EplMatchResult::score(conn, season_id, match_id),
        }
    }

    pub fn is_match_processed(
        self,
        conn: &Connection,
        season_id: i64,
        match_id: i64,
    ) -> rusqlite::Result<bool> {
        match self {
            Self::Wc => WcProcessedMatch::is_processed(conn, season_id, match_id),
            Self::Epl => EplProcessedMatch::is_processed(conn, season_id, match_id),
        }
    }

    pub fn mark_match_processed(
        self,
        conn: &Connection,
        season_id: i64,
        match_id: i64,
    ) -> rusqlite::Result<()> {
        match self {
            Self::Wc => WcProcessedMatch::mark(conn, season_id, match_id),
            Self::Epl => EplProcessedMatch::mark(conn, season_id, match_id),
        }
    }

    pub fn unmark_match_processed(
        self,
        conn: &Connection,
        season_id: i64,
        match_id: i64,
    ) -> rusqlite::Result<()> {
        match self {
            Self::Wc => WcProcessedMatch::unmark(conn, season_id, match_id),
            Self::Epl => EplProcessedMatch::unmark(conn, season_id, match_id),
        }
    }

    /// Persist the finished-match score. No-op when the API row lacks team ids or a full-time score.
    pub fn upsert_match_result(
        self,
        conn: &Connection,
        season_id: i64,
        m: &Match,
    ) -> rusqlite::Result<()> {
        let Some((home_goals, away_goals)) = full_time_score(m) else {
            return Ok(());
        };
        let (Some(home_team_id), Some(away_team_id)) = (m.home_team.id, m.away_team.id) else {
            return Ok(());
        };
        match self {
            Self::Wc => WcMatchResult {
                season_id,
                match_id: m.id,
                home_team_id,
                away_team_id,
                home_goals,
                away_goals,
                stage: m.stage.clone(),
            }
            .upsert(conn),
            Self::Epl => EplMatchResult {
                season_id,
                match_id: m.id,
                home_team_id,
                away_team_id,
                home_goals,
                away_goals,
                matchday: m.matchday,
            }
            .upsert(conn),
        }
    }

    pub async fn poll(
        self,
        data: &Data,
        http: &serenity::Http,
        seasons: &[SeasonMeta],
    ) -> Result<PollOutcome, Error> {
        match self {
            Self::Wc => wc::poll::poll(data, http, seasons).await,
            Self::Epl => epl::poll::poll(data, http, seasons).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CatalogTeam, League};

    #[test]
    fn from_slug_resolves_compiled_leagues() {
        assert_eq!(League::from_slug("wc"), Some(League::Wc));
        assert_eq!(League::from_slug("wc").unwrap().slug(), "wc");
        assert_eq!(
            League::from_slug("wc").unwrap().display_name(),
            "FIFA World Cup"
        );
        assert_eq!(League::from_slug("epl"), Some(League::Epl));
        assert_eq!(League::from_slug("epl").unwrap().slug(), "epl");
        assert_eq!(
            League::from_slug("epl").unwrap().display_name(),
            "Premier League"
        );
    }

    #[test]
    fn from_slug_rejects_unknown_and_catalog_only_slugs() {
        assert_eq!(League::from_slug("nfl"), None);
        assert_eq!(League::from_slug("nba"), None);
        assert_eq!(League::from_slug("unknown"), None);
        assert!(!League::supports_season("nfl"));
        assert!(League::supports_season("wc"));
        assert!(League::supports_season("epl"));
    }

    #[test]
    fn all_lists_every_variant() {
        assert_eq!(League::ALL, &[League::Wc, League::Epl]);
    }

    #[test]
    fn find_team_matches_name_and_code() {
        let teams = vec![CatalogTeam {
            id: 1,
            name: "Brazil".into(),
            short_name: Some("Brazil".into()),
            code: Some("BRA".into()),
        }];
        assert_eq!(League::Wc.find_team(&teams, "bra").unwrap().id, 1);
        assert_eq!(League::Wc.find_team(&teams, "Brazil").unwrap().id, 1);
        assert!(League::Wc.find_team(&teams, "zzz").is_none());
    }
}
