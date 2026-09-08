# WC reference map

Use while implementing a new league. Copy structure, not tournament rules.

## Enum + host dispatch

| File | Role |
|------|------|
| `src/league.rs` | `League::Wc` + method arms |
| `src/poller.rs` | Live seasons → `League::from_slug` → `poll` (no WC types) |
| `src/registration.rs` | `League::for_guild` / `list_teams` / `clear_picks_for_team` |
| `src/standings.rs` | Host-only `StandingRow` + formatting |
| `src/commands/mod.rs` | `commands_for(League::Wc)` → remaining, pick-player |
| `src/commands/helpers.rs` | `ensure_focused_league` |
| `src/types.rs` | `Data { db, http }` only |
| `src/main.rs` | Fail-fast `FOOTBALL_DATA_API_TOKEN` |

## League module

| File | Role |
|------|------|
| `src/wc/mod.rs` | Module tree (`poll`, `remaining`) |
| `src/wc/poll.rs` | WC elimination announces; delegates match ingest to `soccer_poll` |
| `src/game_poll.rs` | Sport-agnostic ingest: `GameReport` → persist via `League`, score, announce, tie-breaker cache |
| `src/soccer_poll.rs` | football-data.org `Match` → `GameReport`; scorer cache |
| `src/wc/remaining.rs` | Tournament remaining (WC-specific) |
| `src/league.rs` | Standings + pick-player dispatch (shared soccer logic) |
| `src/tiebreaker.rs` | Shared pick-player flow (`RosterPlayer`, `claimed_teams`, `resolve_pick`) |
| `src/db/league_macros.rs` | Macros for same-shape league table accessors |
| `src/soccer.rs` | Soccer helpers (WC API interpretation) |
| `src/api/football_data.rs` | HTTP + DTOs |

## DB

| Path | Role |
|------|------|
| `src/db/wc/` | `WcMatchResult`, `WcProcessedMatch`, `WcPlayerGoalTotal`, `WcTiebreakerPick`, `WcAnnouncedElimination` |
| `src/db/migrate.rs` | `WC_LEAGUE_SLUG`, seed row id `1` |
| `src/db/season.rs` | `polling_enabled`, `list_live_with_meta` |

## Commands

| Path | Shared vs WC-only |
|------|-------------------|
| `commands/registration.rs` | Shared |
| `commands/standings.rs` | Shared surface, dispatches via `League` |
| `commands/wc/remaining.rs` | WC-only + focus guard |

## Non-soccer reference: NFL

| File | Role |
|------|------|
| `src/api/espn.rs` | `EspnNflApi` (teams, scoreboard date range) |
| `src/nfl/season.rs` | Season-year rollover, `GameReport` from `NflGame`, preseason/Pro Bowl filter |
| `src/nfl/teams.rs` | `CatalogTeam` list |
| `src/nfl/poll.rs` | Fetch season games → `game_poll::process_game` |
| `src/db/nfl/` | `NflMatchResult`, `NflProcessedGame` (no tie-breaker: `tiebreaker_unit` is `None`) |

## Schema

Per-league tables are added to `CREATE_SCHEMA` together with the league's accessors. `nba` exists only as a `leagues` catalog row.
