//! ESPN public NBA endpoints (no API key). Shares the string→id plumbing and crawler
//! `User-Agent` with the NFL client in `espn.rs`; the JSON shapes match ESPN's site API.

use serde::Deserialize;

use super::{ApiError, ESPN_USER_AGENT, check_response, opt_string_i64, string_i64};

const SITE_BASE_URL: &str = "https://site.api.espn.com/apis/site/v2/sports/basketball/nba";
/// Large enough to return a whole season of games in one page.
const PAGE_LIMIT: u32 = 1000;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NbaTeam {
    #[serde(deserialize_with = "string_i64")]
    pub id: i64,
    pub display_name: String,
    pub short_display_name: Option<String>,
    pub abbreviation: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NbaCompetitor {
    pub home_away: String,
    #[serde(default, deserialize_with = "opt_string_i64")]
    pub score: Option<i64>,
    pub team: NbaTeam,
}

/// One scheduled or finished NBA game. `season_type` is ESPN's numbering:
/// 1 preseason, 2 regular season, 3 postseason.
#[derive(Debug, Clone)]
pub struct NbaGame {
    pub id: i64,
    pub season_type: i64,
    pub completed: bool,
    pub competitors: Vec<NbaCompetitor>,
}

#[derive(Deserialize)]
struct TeamsResponse {
    sports: Vec<TeamsSport>,
}

#[derive(Deserialize)]
struct TeamsSport {
    leagues: Vec<TeamsLeague>,
}

#[derive(Deserialize)]
struct TeamsLeague {
    teams: Vec<TeamEntry>,
}

#[derive(Deserialize)]
struct TeamEntry {
    team: NbaTeam,
}

#[derive(Deserialize)]
struct ScoreboardResponse {
    #[serde(default)]
    events: Vec<Event>,
}

#[derive(Deserialize)]
struct Event {
    #[serde(deserialize_with = "string_i64")]
    id: i64,
    season: EventSeason,
    competitions: Vec<Competition>,
}

#[derive(Deserialize)]
struct EventSeason {
    #[serde(rename = "type")]
    season_type: i64,
}

#[derive(Deserialize)]
struct Competition {
    competitors: Vec<NbaCompetitor>,
    status: CompetitionStatus,
}

#[derive(Deserialize)]
struct CompetitionStatus {
    #[serde(rename = "type")]
    status_type: StatusType,
}

#[derive(Deserialize)]
struct StatusType {
    completed: bool,
}

#[derive(Clone)]
pub struct EspnNbaApi {
    client: reqwest::Client,
}

impl EspnNbaApi {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    async fn get(&self, url: String) -> Result<reqwest::Response, ApiError> {
        let response = self
            .client
            .get(url)
            .header(reqwest::header::USER_AGENT, ESPN_USER_AGENT)
            .send()
            .await
            .map_err(ApiError::Request)?;
        check_response(response)
    }

    pub async fn fetch_teams(&self) -> Result<Vec<NbaTeam>, ApiError> {
        let response = self.get(format!("{SITE_BASE_URL}/teams")).await?;
        let body: TeamsResponse = response.json().await.map_err(ApiError::Request)?;
        Ok(body
            .sports
            .into_iter()
            .flat_map(|sport| sport.leagues)
            .flat_map(|league| league.teams)
            .map(|entry| entry.team)
            .collect())
    }

    /// Games between two `YYYYMMDD` dates (inclusive), every season type.
    pub async fn fetch_games_between(
        &self,
        start: &str,
        end: &str,
    ) -> Result<Vec<NbaGame>, ApiError> {
        let response = self
            .get(format!(
                "{SITE_BASE_URL}/scoreboard?dates={start}-{end}&limit={PAGE_LIMIT}"
            ))
            .await?;
        let body: ScoreboardResponse = response.json().await.map_err(ApiError::Request)?;
        Ok(body
            .events
            .into_iter()
            .filter_map(|event| {
                let competition = event.competitions.into_iter().next()?;
                Some(NbaGame {
                    id: event.id,
                    season_type: event.season.season_type,
                    completed: competition.status.status_type.completed,
                    competitors: competition.competitors,
                })
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn competitor_score_accepts_string_or_missing() {
        let with_score: NbaCompetitor = serde_json::from_str(
            r#"{"homeAway":"home","score":"118","team":{"id":"13","displayName":"Los Angeles Lakers"}}"#,
        )
        .unwrap();
        assert_eq!(with_score.score, Some(118));
        assert_eq!(with_score.team.id, 13);

        let without_score: NbaCompetitor = serde_json::from_str(
            r#"{"homeAway":"away","team":{"id":2,"displayName":"Boston Celtics"}}"#,
        )
        .unwrap();
        assert_eq!(without_score.score, None);
        assert_eq!(without_score.team.id, 2);
    }
}
