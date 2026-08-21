use std::fmt;
use std::sync::OnceLock;

use reqwest::StatusCode;
use serde::Deserialize;

const BASE_URL: &str = "https://api.football-data.org/v4";
const RATE_LIMIT_MESSAGE: &str =
    "The football data API is rate-limited right now. Please try again later.";

#[derive(Debug)]
pub enum ApiError {
    RateLimited,
    Request(reqwest::Error),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::RateLimited => write!(f, "{RATE_LIMIT_MESSAGE}"),
            ApiError::Request(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ApiError::RateLimited => None,
            ApiError::Request(error) => Some(error),
        }
    }
}

fn check_response(response: reqwest::Response) -> Result<reqwest::Response, ApiError> {
    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        return Err(ApiError::RateLimited);
    }
    response.error_for_status().map_err(ApiError::Request)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    pub id: i64,
    pub name: String,
    pub short_name: Option<String>,
    pub tla: Option<String>,
}

/// Team slot on a match fixture. Knockout placeholders may have null id/name until teams are known.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchTeam {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub short_name: Option<String>,
    pub tla: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreDetail {
    pub home: Option<i64>,
    pub away: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Score {
    pub full_time: ScoreDetail,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    pub id: i64,
    pub home_team: MatchTeam,
    pub away_team: MatchTeam,
    pub score: Score,
    pub status: Option<String>,
    pub stage: Option<String>,
    pub group: Option<String>,
    pub matchday: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SquadPlayer {
    pub id: i64,
    pub name: String,
    pub role: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scorer {
    pub player_id: i64,
    pub goals: i64,
}

#[derive(Debug, Deserialize)]
struct MatchesResponse {
    matches: Vec<Match>,
}

#[derive(Debug, Deserialize)]
struct TeamsResponse {
    teams: Vec<Team>,
}

#[derive(Debug, Deserialize)]
struct TeamDetailResponse {
    squad: Vec<SquadPlayer>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScorerEntry {
    player: ScorerPlayer,
    goals: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScorerPlayer {
    id: i64,
}

#[derive(Debug, Deserialize)]
struct ScorersResponse {
    scorers: Vec<ScorerEntry>,
}

#[derive(Clone)]
pub struct FootballDataApi {
    client: reqwest::Client,
    token: String,
}

impl FootballDataApi {
    pub fn new(client: reqwest::Client, token: impl Into<String>) -> Self {
        Self {
            client,
            token: token.into(),
        }
    }

    /// Shared football-data.org token used by soccer league modules (wc, epl).
    pub fn from_env(client: reqwest::Client) -> Self {
        static TOKEN: OnceLock<String> = OnceLock::new();
        let token = TOKEN.get_or_init(|| {
            std::env::var("FOOTBALL_DATA_API_TOKEN").expect(
                "FOOTBALL_DATA_API_TOKEN must be set (required while soccer leagues are compiled in)",
            )
        });
        Self::new(client, token.clone())
    }

    async fn get(&self, path: &str) -> Result<reqwest::Response, ApiError> {
        let url = format!("{BASE_URL}{path}");
        let response = self
            .client
            .get(url)
            .header("X-Auth-Token", &self.token)
            .send()
            .await
            .map_err(ApiError::Request)?;
        check_response(response)
    }

    pub async fn fetch_teams(&self, competition: &str) -> Result<Vec<Team>, ApiError> {
        let path = format!("/competitions/{competition}/teams");
        let response = self.get(&path).await?;
        let body: TeamsResponse = response.json().await.map_err(ApiError::Request)?;
        Ok(body.teams)
    }

    pub async fn fetch_competition_matches(
        &self,
        competition: &str,
    ) -> Result<Vec<Match>, ApiError> {
        let path = format!("/competitions/{competition}/matches");
        let response = self.get(&path).await?;
        let body: MatchesResponse = response.json().await.map_err(ApiError::Request)?;
        Ok(body.matches)
    }

    pub async fn fetch_team_squad(&self, team_id: i64) -> Result<Vec<SquadPlayer>, ApiError> {
        let path = format!("/teams/{team_id}");
        let response = self.get(&path).await?;
        let body: TeamDetailResponse = response.json().await.map_err(ApiError::Request)?;
        Ok(body.squad)
    }

    pub async fn fetch_scorers(&self, competition: &str) -> Result<Vec<Scorer>, ApiError> {
        // football-data.org defaults to limit=10; tie-breaker picks may be outside the top ten.
        let path = format!("/competitions/{competition}/scorers?limit=500");
        let response = self.get(&path).await?;
        let body: ScorersResponse = response.json().await.map_err(ApiError::Request)?;
        Ok(body
            .scorers
            .into_iter()
            .map(|entry| Scorer {
                player_id: entry.player.id,
                goals: entry.goals,
            })
            .collect())
    }
}
