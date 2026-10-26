# League Bot

Discord bot for sports prediction pools. Each member claims one or more teams; when a claimed team's match finishes, the bot awards points and posts an announcement in the channel where the season was started.

The bot can serve **multiple Discord servers** at once, and each server can run **several leagues side by side** (for example an NFL pool and a Premier League pool in the same server). Every season keeps its own claims, standings, and announcement channel. Leagues are compiled into the bot; seasons are configured per server at runtime. World Cup, Premier League, NFL, and NBA are supported.

## Setup

1. Create a Discord application, add a bot, and invite it to your server. [Discord's bot docs](https://docs.discord.com/developers/bots/overview) cover the application, token, and invite. The bot needs to send messages (including embeds) in the channel where you run `/season start`.
2. For soccer leagues, get an API token from [football-data.org](https://www.football-data.org/client/register). Squads and scorer totals need a plan that includes deep data. NFL and NBA use ESPN's public API and need no key.
3. Provide credentials using either:
   - **`league-bot setup`** — prompts for tokens and saves them to a config file (see below), or
   - **`.env`** — copy `.env.example` to `.env` and fill in the values (handy for local development), or
   - **Environment variables** — for systemd, Docker, or Nix (recommended for servers).

On a new server, an admin runs `/season start` in the announcement channel before anything else works. Run `/help` for commands, or `/help <command>` for one command (for example `/help claim`).

### Local development

```bash
cargo run
# or, after building: league-bot
```

Configuration is read in this order (later sources only fill in values not already set): **process environment** → **`.env` in the current directory** → **config file** from `league-bot setup` (typically `~/.config/league-bot/config.toml` on Linux).

First-time interactive setup (prompts for tokens and writes the config file):

```bash
cargo run -- setup
```

Running the bot never prompts for input. If required variables are missing, it exits with a short message.

Slash commands are registered automatically in each guild the bot joins on startup. If commands don't appear, run `/register` in that server.

### Environment variables

| Variable | Required | Description |
|----------|----------|-------------|
| `DISCORD_TOKEN` | yes | Discord bot token |
| `FOOTBALL_DATA_API_TOKEN` | for soccer | football-data.org API token. Only needed for soccer leagues (World Cup, Premier League); the bot boots without it and errors only when a soccer command or poll runs. |
| `DATABASE_PATH` | no | SQLite database path (default: `league_bot.db`) |
| `HEALTH_CHECK_ADDR` | no | Address for the health check HTTP server (default: `0.0.0.0:8080`) |

### Health check

The bot runs a lightweight HTTP server for liveness/readiness probes alongside the Discord gateway. `GET /health` (also `/healthz`) returns `200` when both the database connection is serving queries and the most recent background poll succeeded; if either check fails it returns `503`. The JSON body reports each check, e.g. `{"status":"ok","database":"ok","poller":"ok"}`. Configure the bind address with `HEALTH_CHECK_ADDR` (default `0.0.0.0:8080`).

## Leagues

A team belongs to one person at a time; one person can hold several teams. NFL and NBA teams can be claimed by full name, nickname, or abbreviation (for example `Eagles` or `PHI`). Finished games are polled every 5 minutes; each poll batches a league's new results into a single embed (one game per field), titled with the league's emoji (🌍 World Cup, ⚽ Premier League, 🏈 NFL, 🏀 NBA) so sports stay distinguishable when several leagues share a server. Soccer tie-breaker goals come from `/pick-player` and do not add to the score.

| League | Win / draw / loss | Playoff win | Tie-breaker | Counts | Data |
|--------|-------------------|-------------|-------------|--------|------|
| World Cup (`wc`) | 3 / 1 / 0 | 3 | Player goals | Finished matches | [football-data.org](https://www.football-data.org/) `WC` |
| Premier League (`epl`) | 3 / 1 / 0 | 3 | Player goals | Finished matches | [football-data.org](https://www.football-data.org/) `PL` |
| NFL (`nfl`) | 1 / 0.5 / 0 | 3 | None | Regular season and playoffs | [ESPN](https://www.espn.com/) |
| NBA (`nba`) | 1 / — / 0 | 2 | None | Regular season and playoffs | [ESPN](https://www.espn.com/) |

NFL also skips the Pro Bowl. A live NFL season follows the calendar season that rolls over in March. A live NBA season rolls over in September and is labeled by the year it ends (the 2025–26 season is `2026`).
