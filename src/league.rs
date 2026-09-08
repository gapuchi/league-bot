use poise::serenity_prelude as serenity;
use rusqlite::Connection;

use crate::{
    db::{
        EplMatchResult, EplPlayerGoalTotal, EplProcessedMatch, EplTiebreakerPick, NflMatchResult,
        NflProcessedGame, Registration, Season, SeasonMeta, WcMatchResult, WcPlayerGoalTotal, WcProcessedMatch, WcTiebreakerPick,
    },
    epl,
    game_poll::GameReport,
    nfl,
    scoring::{self, FinishedMatch, ScoringRules},
    standings::{self, StandingRow},
    tiebreaker::{self, RosterPlayer},
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
    Nfl,
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
    pub const ALL: &[League] = &[League::Wc, League::Epl, League::Nfl];

    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "wc" => Some(Self::Wc),
            "epl" => Some(Self::Epl),
            "nfl" => Some(Self::Nfl),
            _ => None,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Wc => "wc",
            Self::Epl => "epl",
            Self::Nfl => "nfl",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Wc => "FIFA World Cup",
            Self::Epl => "Premier League",
            Self::Nfl => "NFL",
        }
    }

    /// Whether `/season start` (and related setup) may target this slug.
    pub fn supports_season(slug: &str) -> bool {
        Self::from_slug(slug).is_some()
    }

    /// Stat that breaks ties on the leaderboard, as shown to users. `None` when the
    /// league has no tie-breaker (ties on points share a rank).
    pub fn tiebreaker_unit(self) -> Option<&'static str> {
        match self {
            Self::Wc | Self::Epl => Some("goals"),
            Self::Nfl => None,
        }
    }

    /// Points awarded for win / draw / loss on this league's leaderboard.
    pub fn scoring(self) -> ScoringRules {
        match self {
            Self::Wc | Self::Epl => scoring::SOCCER,
            Self::Nfl => scoring::NFL,
        }
    }

    /// Word for a level result in announcements and footers.
    pub fn draw_label(self) -> &'static str {
        match self {
            Self::Wc | Self::Epl => "draw",
            Self::Nfl => "tie",
        }
    }

    /// Announcement suffix once a game is over.
    pub fn finished_label(self) -> &'static str {
        match self {
            Self::Wc | Self::Epl => "full time",
            Self::Nfl => "final",
        }
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
        match self {
            Self::Wc | Self::Epl => {
                let competition = crate::db::league_competition_code(self.slug());
                let teams = crate::api::FootballDataApi::from_env(data.http.clone())
                    .fetch_teams(&competition)
                    .await?;
                Ok(teams.into_iter().map(CatalogTeam::from_api).collect())
            }
            Self::Nfl => nfl::teams::list_teams(data).await,
        }
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
            Self::Nfl => format!(
                "Couldn't find an NFL team matching \"{team_query}\". Try the full name, nickname, or abbreviation (e.g. Eagles or PHI)."
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
            Self::Nfl => NflMatchResult::list_for_season(conn, season_id)?
                .iter()
                .map(NflMatchResult::as_finished_match)
                .collect(),
        })
    }

    /// `(tie-breaker total, player_name)` for standings; total is 0 without a pick.
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
            Self::Nfl => Ok((0, None)),
        }
    }

    pub fn standings(
        self,
        conn: &Connection,
        season_id: i64,
    ) -> rusqlite::Result<Vec<StandingRow>> {
        standings::build_rows(
            self.scoring(),
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
    ) -> rusqlite::Result<f64> {
        Ok(standings::points_for_user_teams(
            self.scoring(),
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
            Self::Nfl => None,
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
            Self::Nfl => Ok(()),
        }
    }

    fn no_tiebreaker_message(self) -> String {
        format!(
            "{} has no tie-breaker — members level on points share a rank.",
            self.display_name()
        )
    }

    /// Rosters of the given teams for `/pick-player`; `None` when the league has no tie-breaker.
    async fn rosters_for_teams(
        self,
        data: &Data,
        teams: &[(i64, String)],
    ) -> Result<Option<Vec<RosterPlayer>>, Error> {
        match self {
            Self::Wc | Self::Epl => {
                let api = crate::api::FootballDataApi::from_env(data.http.clone());
                Ok(Some(crate::soccer::fetch_squads_for_teams(&api, teams).await?))
            }
            Self::Nfl => Ok(None),
        }
    }

    pub async fn pick_tiebreaker_player(
        self,
        data: &Data,
        guild_id: u64,
        user_id: u64,
        player: &str,
    ) -> Result<String, Error> {
        if self.tiebreaker_unit().is_none() {
            return Ok(self.no_tiebreaker_message());
        }
        let teams = tiebreaker::claimed_teams(data, guild_id, user_id).await?;
        if teams.is_empty() {
            return Ok(tiebreaker::NO_TEAMS_MESSAGE.into());
        }
        let Some(players) = self.rosters_for_teams(data, &teams).await? else {
            return Ok(self.no_tiebreaker_message());
        };

        tiebreaker::resolve_pick(data, guild_id, user_id, player, &players, |conn,
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
                Self::Nfl => Ok(()),
            }
        })
        .await
    }

    /// Cache `(player_id, total)` tie-breaker stats for a season; no-op for leagues without one.
    pub fn cache_tiebreaker_totals(
        self,
        conn: &Connection,
        season_id: i64,
        totals: &[(i64, i64)],
        updated_at: &str,
    ) -> rusqlite::Result<()> {
        match self {
            Self::Wc => WcPlayerGoalTotal::upsert_batch(conn, season_id, totals, updated_at),
            Self::Epl => EplPlayerGoalTotal::upsert_batch(conn, season_id, totals, updated_at),
            Self::Nfl => Ok(()),
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
            Self::Nfl => NflMatchResult::score(conn, season_id, match_id),
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
            Self::Nfl => NflProcessedGame::is_processed(conn, season_id, match_id),
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
            Self::Nfl => NflProcessedGame::mark(conn, season_id, match_id),
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
            Self::Nfl => NflProcessedGame::unmark(conn, season_id, match_id),
        }
    }

    /// Persist a finished game's score in the league's result table.
    pub fn upsert_match_result(
        self,
        conn: &Connection,
        season_id: i64,
        report: &GameReport,
    ) -> rusqlite::Result<()> {
        match self {
            Self::Wc => WcMatchResult {
                season_id,
                match_id: report.game_id,
                home_team_id: report.home_team_id,
                away_team_id: report.away_team_id,
                home_goals: report.home_score,
                away_goals: report.away_score,
                stage: report.stage.clone(),
            }
            .upsert(conn),
            Self::Epl => EplMatchResult {
                season_id,
                match_id: report.game_id,
                home_team_id: report.home_team_id,
                away_team_id: report.away_team_id,
                home_goals: report.home_score,
                away_goals: report.away_score,
                matchday: report.round,
            }
            .upsert(conn),
            Self::Nfl => NflMatchResult {
                season_id,
                game_id: report.game_id,
                home_team_id: report.home_team_id,
                away_team_id: report.away_team_id,
                home_score: report.home_score,
                away_score: report.away_score,
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
            Self::Nfl => nfl::poll::poll(data, http, seasons).await,
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
        assert_eq!(League::from_slug("nfl"), Some(League::Nfl));
        assert_eq!(League::from_slug("nfl").unwrap().slug(), "nfl");
        assert_eq!(League::from_slug("nfl").unwrap().display_name(), "NFL");
        assert!(League::supports_season("nfl"));
    }

    #[test]
    fn from_slug_rejects_unknown_and_catalog_only_slugs() {
        assert_eq!(League::from_slug("nba"), None);
        assert_eq!(League::from_slug("unknown"), None);
        assert!(!League::supports_season("nba"));
        assert!(League::supports_season("wc"));
        assert!(League::supports_season("epl"));
    }

    #[test]
    fn all_lists_every_variant() {
        assert_eq!(League::ALL, &[League::Wc, League::Epl, League::Nfl]);
    }

    #[test]
    fn user_facing_labels_follow_the_sport() {
        assert_eq!(League::Wc.tiebreaker_unit(), Some("goals"));
        assert_eq!(League::Nfl.tiebreaker_unit(), None);
        assert_eq!(League::Nfl.draw_label(), "tie");
        assert_eq!(League::Nfl.finished_label(), "final");
        assert_eq!(League::Epl.finished_label(), "full time");
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

        let nfl = vec![CatalogTeam {
            id: 21,
            name: "Philadelphia Eagles".into(),
            short_name: Some("Eagles".into()),
            code: Some("PHI".into()),
        }];
        assert_eq!(League::Nfl.find_team(&nfl, "eagles").unwrap().id, 21);
        assert_eq!(League::Nfl.find_team(&nfl, "phi").unwrap().id, 21);
    }
}
