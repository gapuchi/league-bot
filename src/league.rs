use poise::serenity_prelude as serenity;
use rusqlite::Connection;

use crate::{
    db::{Season, SeasonMeta},
    epl,
    standings::StandingRow,
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
        match self {
            Self::Wc => wc::teams::list_teams(data).await,
            Self::Epl => epl::teams::list_teams(data).await,
        }
    }

    pub fn find_team<'a>(self, teams: &'a [CatalogTeam], query: &str) -> Option<&'a CatalogTeam> {
        let query = query.trim().to_lowercase();
        teams.iter().find(|team| {
            team.name.to_lowercase() == query
                || team
                    .short_name
                    .as_ref()
                    .is_some_and(|n| n.to_lowercase() == query)
                || team
                    .code
                    .as_ref()
                    .is_some_and(|c| c.to_lowercase() == query)
                || team.name.to_lowercase().contains(&query)
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

    pub fn standings(
        self,
        conn: &Connection,
        season_id: i64,
    ) -> rusqlite::Result<Vec<StandingRow>> {
        match self {
            Self::Wc => wc::standings::get_standings(conn, season_id),
            Self::Epl => epl::standings::get_standings(conn, season_id),
        }
    }

    pub fn user_points(
        self,
        conn: &Connection,
        season_id: i64,
        user_id: u64,
    ) -> rusqlite::Result<i64> {
        match self {
            Self::Wc => wc::standings::user_points(conn, season_id, user_id),
            Self::Epl => epl::standings::user_points(conn, season_id, user_id),
        }
    }

    pub fn tiebreaker_value_for_user(
        self,
        conn: &Connection,
        season_id: i64,
        user_id: u64,
    ) -> rusqlite::Result<i64> {
        match self {
            Self::Wc => wc::standings::tiebreaker_goals_for_user(conn, season_id, user_id),
            Self::Epl => epl::standings::tiebreaker_goals_for_user(conn, season_id, user_id),
        }
    }

    /// `(player_name, team_name)` when the user has a tie-breaker pick.
    pub fn tiebreaker_pick_for_user(
        self,
        conn: &Connection,
        season_id: i64,
        user_id: u64,
    ) -> rusqlite::Result<Option<(String, String)>> {
        match self {
            Self::Wc => wc::standings::tiebreaker_pick_for_user(conn, season_id, user_id),
            Self::Epl => epl::standings::tiebreaker_pick_for_user(conn, season_id, user_id),
        }
    }

    pub fn clear_picks_for_team(
        self,
        conn: &Connection,
        season_id: i64,
        user_id: u64,
        team_id: i64,
    ) -> rusqlite::Result<()> {
        match self {
            Self::Wc => {
                wc::standings::clear_picks_for_team(conn, season_id, user_id, team_id)
            }
            Self::Epl => {
                epl::standings::clear_picks_for_team(conn, season_id, user_id, team_id)
            }
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
