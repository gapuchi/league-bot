# Agent guide

## Related docs

- [`src/db/README.md`](src/db/README.md) — schema and entities
- [`docs/plans/multi-sport-framework/plan.md`](docs/plans/multi-sport-framework/plan.md) — multi-league framework plan
- [`docs/plans/season-draft/plan.md`](docs/plans/season-draft/plan.md) — pre-season snake draft plan
- [`.cursor/skills/add-league/SKILL.md`](.cursor/skills/add-league/SKILL.md) — **`/add-league`** runbook to add a `League` variant
- [`.cursor/rules/`](.cursor/rules/) — scoped reminders (`api-layer`, `db-layer`, `readme-sync`)

## Project overview

**League Bot** — Discord bot (Rust, poise + serenity) for sports prediction pools. World Cup is live; NFL planned. SQLite persistence; match data from [football-data.org](https://www.football-data.org/).

## Layer boundaries

Strict layers — details when editing matching paths are in `.cursor/rules/`:

| Layer | Role |
|-------|------|
| `src/api/` | HTTP clients + serde DTOs only; 1:1 with endpoints |
| `src/soccer.rs` | Soccer domain helpers on API data (used by the `wc` league module) |
| `src/db/` | Persistence accessors; shared entities at root; league tables under `db/<slug>/` |
| `src/league.rs` | Compile-time `League` enum — host dispatch face into league modules |
| League modules (`src/wc/`, …) | API client, poll, standings, tie-break, league-only use cases |
| Host use cases (`registration.rs`, `standings.rs` formatters, `poller.rs`) | Shared orchestration via `League` |
| `src/commands/` | Thin Discord adapters only |

**Command adapters vs use cases** — commands handle poise attrs, `guild_id`, defer/reply; use cases resolve season, dispatch via `League`, call `db/` / league modules, return strings.

| Concern | Adapter | Use case |
|---------|---------|----------|
| Registration | `commands/registration.rs` | `registration.rs` → `League` |
| Shared standings | `commands/standings.rs` | `League::standings` → league module (format helpers in `standings.rs`) |
| WC-only cmds | `commands/wc/` (`remaining`, `pick-player`) | `wc/` (registered via `commands_for(League)`) |
| Config | `commands/config.rs` | inline DB (small, admin-only) |

New behavior: use case first → thin handler in `commands/` → `commands::all()`.

```rust
// Adapter calls use case; use case owns API + DB + rules via League
let message = registration::pick_for_user(ctx.data(), guild_id, user_id, &team).await?;
ctx.say(message).await?;
```

**Poller** — `poller.rs` lists **live** seasons, groups by league slug, calls `League::poll`; not a command.

## Leagues vs seasons

| Term | Meaning |
|------|---------|
| **League** | Compile-time competition type (`League` enum in `src/league.rs`, slug e.g. `wc`). Adding a league is a code change. |
| **Season** | Runtime instance of a league for one guild (`season_id`). Created via `/season start`. |
| **Command focus** | Guild’s `default_season_id` — which season slash commands use. |
| **Live season** | Season with `polling_enabled` — which seasons the poller processes (independent of focus). |

Catalog rows may exist in `leagues` for future slugs; only variants on `League` can have seasons (`League::supports_season` / `from_slug`).

## Seasons (multi-guild, multi-league)

Tenancy is at **season** (`seasons.guild_id`).

- Gameplay commands: `Season::default_for_guild` / `League::for_guild` — pass invoking `ctx.guild_id()`
- Poller: `Season::list_live_with_meta()` then `League::from_slug` → `poll`
- Setup: `/season start` creates a season for a compiled-in league; fresh guilds have none until then
- `season_id` keys registrations, results, tie-breakers, announcements
- Do not hardcode guild or season ids
- `/config league` changes command focus; data per league stays separate

## Key patterns

- Resolve the focused season’s league with `League::for_guild` / `League::for_season`, then call enum methods (`list_teams`, `standings`, `poll`, …)
- `Data` holds `db` + shared `http`; soccer leagues use `FootballDataApi::from_env(data.http.clone())`
- Types from `crate::api`; soccer domain helpers from `crate::soccer` (wc module)
- Competition code from `league_competition_code()` via league slug
- League-specific slash commands: exhaustive `commands_for(League)` in `commands/mod.rs`

## Adding a league

Invoke **`/add-league`** (skill: `.cursor/skills/add-league/`). Short checklist:

1. `League` variant + match arms in `src/league.rs`
2. League module `src/<slug>/` (teams, standings, poll, …)
3. `db/<slug>/` accessors if needed (nfl/nba tables may already exist)
4. `commands_for(League)` for league-only commands
5. Env + README; `cargo test` / clippy

## Repo conventions

- **Tests** — `tests/migrate.rs`, `tests/standings.rs`, `tests/api.rs`
- **Errors** — `ApiError` in api; `types::Error` in commands
- **Releases** — `Cargo.toml`; `cargo release` or `just release`

## Common tasks

| Task | Where |
|------|-------|
| New command | Use case → `commands/` handler → `commands::all()` or `commands_for(League)` → docstring |
| New DB table | Extend greenfield `CREATE_SCHEMA` in `migrate.rs` (no upgrade path) + `db/` or `db/<league>/` → re-export in `db/mod.rs` |
| New API endpoint | `api/…` + league module helpers as needed |
| New league poller | `League::poll` arm + league module `poll`; soccer leagues delegate match ingestion to `soccer_poll` (host `poller.rs` stays generic) |
| Scoring / tie-breakers | league module + shared `scoring` helpers; update `README.md` if user-visible |
| Setup / config UX | `README.md` (see `readme-sync.mdc`) |

## What not to do

- Search, filtering, or orchestration in `api/`
- Business logic in `commands/`
- HTTP or Discord in `db/`
- Bypass `Season::default_for_guild()` / `League::for_guild` in gameplay commands
- Hard-wire `Wc*` types into shared host paths (`registration`, host `standings`, `types`, `db/registration`)
- Monolithic `db/mod.rs` with inline SQL
- Raw `reqwest::Client` + token in host code when `FootballDataApi::from_env` exists
- Assume command focus controls the poller (use `polling_enabled` / live seasons)

## Running checks

```bash
cargo test
cargo clippy -- -D warnings
```

## Cursor Cloud specific instructions

Toolchain and system deps are already provisioned in the VM snapshot; the startup update script only runs `cargo fetch`. Notes below are the non-obvious gotchas.

- **Rust edition 2024** (`Cargo.toml`) requires Rust ≥ 1.85. The base image ships an older `cargo`/`rustc` (1.83) that fails to build this repo; a newer `stable` toolchain is installed via `rustup` and set as default. If a build errors on `edition2024`, run `rustup default stable`.
- **System libraries**: `reqwest` uses `native-tls`, so `libssl-dev` + `pkg-config` must be present (missing `openssl.pc` breaks the build); `rusqlite` uses the `bundled` feature, so a C compiler (`gcc`) is required. These are already installed in the snapshot.
- **Running the bot** (`cargo run` / `./target/debug/league-bot`): it is a headless Discord gateway bot with **no local HTTP/UI**. It fail-fasts if `DISCORD_TOKEN` or `FOOTBALL_DATA_API_TOKEN` are unset. With placeholder tokens it still initializes the DB and reaches Discord auth, then exits with `Sent invalid authentication`. A real interactive end-to-end (slash commands like `/season start`, `/draft pick`, `/standings`) needs a real Discord bot token + the bot invited to a guild, plus a football-data.org token (deep data / squads + scorers need a paid tier). See `README.md` for setup and invite scopes.
- **SQLite is embedded** (no DB server). The file is created automatically at `DATABASE_PATH` (default `league_bot.db`) on first boot; there is no migration path — delete the file to reset (`src/db/migrate.rs`).
