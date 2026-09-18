# League Bot

Discord bot for sports prediction pools. Each member can claim one or more teams; when a claimed team's match finishes, the bot awards points and posts an announcement in a configured channel.

The bot can serve **multiple Discord servers** at once, and each server can run **several leagues side by side** (for example an NFL pool and a Premier League pool in the same server). Every season keeps its own claims, standings, and announcement channel. Leagues are compiled into the bot; seasons are configured per server at runtime. World Cup, Premier League, NFL, and NBA are fully supported today.

## Setup

1. Create a [Discord application](https://discord.com/developers/applications) and bot token.
2. Get a free API token from [football-data.org](https://www.football-data.org/client/register) (soccer leagues). NFL data comes from ESPN's public endpoints and needs no key.
3. Copy `.env.example` to `.env` and fill in the values.
4. Invite the bot to your server. In the [Discord Developer Portal](https://discord.com/developers/applications) → **OAuth2** → **URL Generator**, select:
   - **Scopes:** `bot`, `applications.commands`
   - **Bot permissions:** View Channels, Send Messages, Embed Links, Create Public Threads, Send Messages in Threads

   Open the generated URL to add the bot. It must be able to send messages (including embeds) in the channel you set with `/season channel`.

### Local development

```bash
cargo run
```

The bot loads environment variables from `.env` via dotenvy.

Slash commands are registered automatically in each guild the bot joins on startup. If commands don't appear, run `/register` in that server.

### Environment variables

| Variable | Required | Description |
|----------|----------|-------------|
| `DISCORD_TOKEN` | yes | Discord bot token |
| `FOOTBALL_DATA_API_TOKEN` | yes | football-data.org API token (World Cup, Premier League) |
| `DATABASE_PATH` | no | SQLite database path (default: `league_bot.db`) |

## Seasons

Each Discord server configures the bot independently. On a **new** server, an admin must start a season first — nothing works until `/season start` has been run there.

| Command | Who | What |
|---------|-----|------|
| `/season start <league> [name]` | admin | Create (or resume) a season. `name` defaults to the league plus the current year, e.g. `NFL 2026`. Starting a season **ends** that league's previous live season in the server, so a new year is one command. |
| `/season end [league]` | admin | Stop match polling. Data is kept; `/standings league:<league>` still works. |
| `/season channel <#channel> [league]` | admin | Where match announcements are posted. Nothing is announced until this is set. |
| `/season list` | anyone | Every season in the server with live/ended state, roster phase, and channel. |

### The `league` option

Every gameplay command (`/claim`, `/standings`, `/draft …`, `/team`, …) takes an optional `league` choice (World Cup, Premier League, NFL).

- **One live season in the server** — leave it out; the bot uses that season.
- **Several live seasons** — the bot asks which league; pick it from the dropdown.
- **An ended season** — pass `league` explicitly to look at its standings or roster.

There is no server-wide "current league" setting to switch.

### Team selection and pre-season draft

Teams can be claimed freely or through a snake draft:

1. A new season's roster is `open`; members use `/claim <team>`. Admins can `/assign` a team to someone, and members can `/unclaim`.
2. An admin runs `/draft start` mentioning every player (`@alice @bob @carol`). Order is **randomized** and the roster becomes `drafting`. `/claim` is blocked until the draft ends.
3. During the draft, `/draft pick <team>` is restricted to the player on the clock; admins may `/assign` **only** for that player. The last picker may `/draft unpick` to undo their pick until the next person picks.
4. The draft runs for full rounds only — each player gets the same number of teams (the pool size rounded down to a multiple of the player count). When that pick limit is reached, or when every team is taken, the draft completes and the roster is **frozen** (no further claims/assigns/unclaims/unpicks). Admins can also `/draft end` to freeze early, or `/draft cancel` to scrap the draft, clear its picks, and reopen the roster.

Use `/draft status` anytime for order, whose turn, and remaining teams; `/undrafted` lists teams nobody holds.

## Commands

Run `/help` to list all commands, or `/help <command>` for details (e.g. `/help claim`).

## Leagues

### World Cup (`wc`)

Each member claims one or more nations. Each nation can only be claimed by one person at a time; a person can claim multiple nations. When a claimed team's match finishes, the bot awards points and posts an announcement in the configured channel.

**Scoring** — points per match based on the result for each claimed team:

| Result | Points |
|--------|--------|
| Win    | 3      |
| Draw   | 1      |
| Loss   | 0      |

**Tie-breaker** — if two players finish with the same total points, the one whose designated player has scored more goals in the tournament ranks higher. Use `/pick-player` to choose one player from your claimed teams' squads. If you don't pick, tie-breaker goals count as 0. Tie-breaker goals do not add to your score — they only break ties on the leaderboard.

The background poller runs every 5 minutes, fetching finished matches and scorer totals from football-data.org (`WC` competition). Player squads and scorer data require a football-data.org plan that includes deep data (squads and goal scorers).

### Premier League (`epl`)

Each member claims one or more clubs. Each club can only be claimed by one person at a time; a person can claim multiple clubs. When a claimed team's match finishes, the bot awards points and posts an announcement in the configured channel.

**Scoring** — same win/draw/loss points as World Cup (3 / 1 / 0).

**Tie-breaker** — same as World Cup: designate a player with `/pick-player`; total goals from that player in the season break ties on the leaderboard.

The background poller fetches finished matches and scorer totals from football-data.org (`PL` competition). Player squads and scorer data require a football-data.org plan that includes deep data.

### NFL (`nfl`)

Each member claims one or more franchises. Each team can only be claimed by one person at a time; a person can claim multiple teams. Claim by full name, nickname, or abbreviation (e.g. `Eagles`, `PHI`). When a claimed team's game goes final, the bot awards points and posts an announcement in the configured channel.

**Scoring** — regular-season win 1, tie 0.5, loss 0 per game; a playoff (postseason) win is worth 3. Regular-season and playoff games count; preseason and the Pro Bowl do not.

**Tie-breaker** — none. Members level on points share a rank, and `/pick-player` is not used for NFL seasons.

The background poller fetches finished games from ESPN's public NFL API (no API key). Every live NFL season tracks the current NFL calendar season (March rolls over to the next season), so start one season per year.

### NBA (`nba`)

Each member claims one or more franchises. Each team can only be claimed by one person at a time; a person can claim multiple teams. Claim by full name, nickname, or abbreviation (e.g. `Lakers`, `LAL`). When a claimed team's game goes final, the bot awards points and posts an announcement in the configured channel.

**Scoring** — regular-season win 1, loss 0 per game; a playoff (postseason) win is worth 3. Regular-season and playoff games count; preseason and the All-Star Game do not. Basketball games never end level, so there is no tie score.

**Tie-breaker** — none. Members level on points share a rank, and `/pick-player` is not used for NBA seasons.

The background poller fetches finished games from ESPN's public NBA API (no API key). Every live NBA season tracks the current NBA calendar season (the offseason rolls over to the next season), so start one season per year.
