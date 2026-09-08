//! ESPN public NFL endpoints (no API key). Ids arrive as JSON strings and are
//! parsed to `i64` here so downstream code sees the same shape as other providers.

use serde::{Deserialize, Deserializer};

use super::{ApiError, check_response};

const SITE_BASE_URL: &str = "https://site.api.espn.com/apis/site/v2/sports/football/nfl";
const CORE_BASE_URL: &str = "https://sports.core.api.espn.com/v2/sports/football/leagues/nfl";
const TOUCHDOWN_CATEGORY: &str = "totalTouchdowns";
/// Large enough to return a whole season of games or every touchdown scorer in one page.
const PAGE_LIMIT: u32 = 1000;
/// ESPN's edge returns 403 to unidentified clients; a crawler-style `name/version (+url)`
/// agent is accepted. Keep the repository URL — bare product tokens are still refused.
const USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " (+",
    env!("CARGO_PKG_REPOSITORY"),
    ")"
);

#[derive(Deserialize)]
#[serde(untagged)]
enum StringOrNumber {
    Number(i64),
    Text(String),
}

fn string_i64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    match StringOrNumber::deserialize(deserializer)? {
        StringOrNumber::Number(value) => Ok(value),
        StringOrNumber::Text(text) => text.trim().parse().map_err(serde::de::Error::custom),
    }
}

fn opt_string_i64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<i64>, D::Error> {
    match Option::<StringOrNumber>::deserialize(deserializer)? {
        None => Ok(None),
        Some(StringOrNumber::Number(value)) => Ok(Some(value)),
        Some(StringOrNumber::Text(text)) => {
            let text = text.trim();
            if text.is_empty() {
                Ok(None)
            } else {
                text.parse().map(Some).map_err(serde::de::Error::custom)
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NflTeam {
    #[serde(deserialize_with = "string_i64")]
    pub id: i64,
    pub display_name: String,
    pub short_display_name: Option<String>,
    pub abbreviation: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NflCompetitor {
    pub home_away: String,
    #[serde(default, deserialize_with = "opt_string_i64")]
    pub score: Option<i64>,
    pub team: NflTeam,
}

/// One scheduled or finished NFL game. `season_type` is ESPN's numbering:
/// 1 preseason, 2 regular season, 3 postseason.
#[derive(Debug, Clone)]
pub struct NflGame {
    pub id: i64,
    pub name: String,
    pub date: String,
    pub season_year: i64,
    pub season_type: i64,
    pub week: Option<i64>,
    pub completed: bool,
    pub competitors: Vec<NflCompetitor>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NflRosterPlayer {
    #[serde(deserialize_with = "string_i64")]
    pub id: i64,
    pub full_name: String,
    pub position: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NflTouchdownLeader {
    pub player_id: i64,
    pub touchdowns: i64,
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
    team: NflTeam,
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
    name: String,
    date: String,
    season: EventSeason,
    week: Option<EventWeek>,
    competitions: Vec<Competition>,
}

#[derive(Deserialize)]
struct EventSeason {
    year: i64,
    #[serde(rename = "type")]
    season_type: i64,
}

#[derive(Deserialize)]
struct EventWeek {
    number: i64,
}

#[derive(Deserialize)]
struct Competition {
    competitors: Vec<NflCompetitor>,
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

#[derive(Deserialize)]
struct RosterResponse {
    #[serde(default)]
    athletes: Vec<RosterGroup>,
}

#[derive(Deserialize)]
struct RosterGroup {
    #[serde(default)]
    items: Vec<RosterItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RosterItem {
    #[serde(deserialize_with = "string_i64")]
    id: i64,
    full_name: String,
    position: Option<RosterPosition>,
}

#[derive(Deserialize)]
struct RosterPosition {
    abbreviation: Option<String>,
}

#[derive(Deserialize)]
struct LeadersResponse {
    #[serde(default)]
    categories: Vec<LeaderCategory>,
}

#[derive(Deserialize)]
struct LeaderCategory {
    name: String,
    #[serde(default)]
    leaders: Vec<LeaderEntry>,
}

#[derive(Deserialize)]
struct LeaderEntry {
    value: f64,
    athlete: RefLink,
}

#[derive(Deserialize)]
struct RefLink {
    #[serde(rename = "$ref")]
    href: String,
}

/// Extracts `{id}` from `.../athletes/{id}?lang=en`.
fn athlete_id_from_ref(href: &str) -> Option<i64> {
    let (_, tail) = href.rsplit_once("/athletes/")?;
    let id = tail.split(['?', '/']).next()?;
    id.parse().ok()
}

#[derive(Clone)]
pub struct EspnNflApi {
    client: reqwest::Client,
}

impl EspnNflApi {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    async fn get(&self, url: String) -> Result<reqwest::Response, ApiError> {
        let response = self
            .client
            .get(url)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await
            .map_err(ApiError::Request)?;
        check_response(response)
    }

    pub async fn fetch_teams(&self) -> Result<Vec<NflTeam>, ApiError> {
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
    ) -> Result<Vec<NflGame>, ApiError> {
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
                Some(NflGame {
                    id: event.id,
                    name: event.name,
                    date: event.date,
                    season_year: event.season.year,
                    season_type: event.season.season_type,
                    week: event.week.map(|week| week.number),
                    completed: competition.status.status_type.completed,
                    competitors: competition.competitors,
                })
            })
            .collect())
    }

    pub async fn fetch_team_roster(&self, team_id: i64) -> Result<Vec<NflRosterPlayer>, ApiError> {
        let response = self
            .get(format!("{SITE_BASE_URL}/teams/{team_id}/roster"))
            .await?;
        let body: RosterResponse = response.json().await.map_err(ApiError::Request)?;
        Ok(body
            .athletes
            .into_iter()
            .flat_map(|group| group.items)
            .map(|item| NflRosterPlayer {
                id: item.id,
                full_name: item.full_name,
                position: item.position.and_then(|position| position.abbreviation),
            })
            .collect())
    }

    /// Season-long total touchdowns scored per player (rushing, receiving, returns).
    pub async fn fetch_touchdown_leaders(
        &self,
        season_year: i64,
        season_type: i64,
    ) -> Result<Vec<NflTouchdownLeader>, ApiError> {
        let response = self
            .get(format!(
                "{CORE_BASE_URL}/seasons/{season_year}/types/{season_type}/leaders?limit={PAGE_LIMIT}"
            ))
            .await?;
        let body: LeadersResponse = response.json().await.map_err(ApiError::Request)?;
        Ok(body
            .categories
            .into_iter()
            .filter(|category| category.name == TOUCHDOWN_CATEGORY)
            .flat_map(|category| category.leaders)
            .filter_map(|entry| {
                Some(NflTouchdownLeader {
                    player_id: athlete_id_from_ref(&entry.athlete.href)?,
                    touchdowns: entry.value.round() as i64,
                })
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn athlete_id_parses_core_ref() {
        assert_eq!(
            athlete_id_from_ref(
                "http://sports.core.api.espn.com/v2/sports/football/leagues/nfl/seasons/2025/athletes/12483?lang=en&region=us"
            ),
            Some(12483)
        );
        assert_eq!(athlete_id_from_ref("http://example.com/teams/1"), None);
    }

    #[test]
    fn competitor_score_accepts_string_or_missing() {
        let with_score: NflCompetitor = serde_json::from_str(
            r#"{"homeAway":"home","score":"24","team":{"id":"21","displayName":"Philadelphia Eagles"}}"#,
        )
        .unwrap();
        assert_eq!(with_score.score, Some(24));
        assert_eq!(with_score.team.id, 21);

        let without_score: NflCompetitor = serde_json::from_str(
            r#"{"homeAway":"away","team":{"id":6,"displayName":"Dallas Cowboys"}}"#,
        )
        .unwrap();
        assert_eq!(without_score.score, None);
        assert_eq!(without_score.team.id, 6);
    }
}
