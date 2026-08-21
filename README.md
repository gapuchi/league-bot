# League Bot

Discord bot for sports prediction pools. Each member can claim one or more teams; when a claimed team's match finishes, the bot awards points and posts an announcement in a configured channel.

The bot can serve **multiple Discord servers** at once. Each server has its own team claims, standings, announcement channel, and league selection. Leagues are compiled into the bot; seasons are configured per server at runtime. World Cup and Premier League are fully supported today; NFL and NBA pools are coming soon.

## Setup

1. Create a [Discord application](https://discord.com/developers/applications) and bot token.
2. Get a free API token from [football-data.org](https://www.football-data.org/client/register).
3. Copy `.env.example` to `.env` and fill in the values.
4. Invite the bot to your server. In the [Discord Developer Portal](https://discord.com/developers/applications) → **OAuth2** → **URL Generator**, select:
   - **Scopes:** `bot`, `applications.commands`
   - **Bot permissions:** View Channels, Send Messages, Embed Links, Create Public Threads, Send Messages in Threads

   Open the generated URL to add the bot. It must be able to send messages (including embeds) in the channel you set with `/config channel`.

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
| `FOOTBALL_DATA_API_TOKEN` | yes | football-data.org API token |
| `DATABASE_PATH` | no | SQLite database path (default: `league_bot.db`) |

## Configuration

Each Discord server configures the bot independently. On a **new** server, an admin must create a season first — there is no default until `/config season` has been run in that server. Run `/help config` for subcommands.

Match announcements are only sent after `/config channel` has been set for that season. Each season keeps its own channel, registrations, scores, and tie-breaker picks. Gameplay commands always target the **command focus** season (default season) for the server where the command was run. The background poller processes seasons marked live for polling, which is separate from command focus. Admins can stop polling for the focused season with `/config season-end`.

Use `/season` to see which league and season commands currently target in this server.

### Team selection and pre-season draft

Teams are selected through `/draft pick`, with or without a snake draft:

1. Season roster phase starts as `open`; members may freely use `/draft pick`.
2. `/draft start` with the list of players — order is **randomized**; phase becomes `drafting`.
3. During the draft, `/draft pick` is restricted to the player on the clock; admins may `/assign` **only** for that player. `/unclaim` is blocked.
4. When every team is taken, the draft completes and the roster is **frozen** (no further claims/assigns/unclaims). Admins can also run `/draft end` to freeze the roster early without drafting every team.

Use `/draft status` anytime for order, whose turn, and remaining teams.

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

Coming soon.

### NBA (`nba`)

Coming soon.
