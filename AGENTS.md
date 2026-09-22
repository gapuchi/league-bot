# Agent guide

## Related docs

- **Cross-repo** — `coding-philosophy` rule (`gapuchi/ai`)
- [`src/db/README.md`](src/db/README.md) — schema and entities
- [`docs/plans/multi-sport-framework/plan.md`](docs/plans/multi-sport-framework/plan.md) — multi-league framework plan
- [`docs/plans/season-draft/plan.md`](docs/plans/season-draft/plan.md) — pre-season snake draft plan
- [`.cursor/skills/add-league/SKILL.md`](.cursor/skills/add-league/SKILL.md) — **`/add-league`** runbook to add a `League` variant
- [`.cursor/rules/`](.cursor/rules/) — scoped reminders (`api-layer`, `db-layer`, `readme-sync`)

## Project overview

**League Bot** — Discord bot (Rust, poise + serenity) for sports prediction pools. World Cup, Premier League, and NFL are live; NBA planned. SQLite persistence; soccer data from [football-data.org](https://www.football-data.org/), NFL data from ESPN's public API.

## Layer boundaries

Strict layers — details when editing matching paths are in `.cursor/rules/`:

| Layer | Role |
|-------|------|
| `src/api/` | HTTP clients + serde DTOs only; 1:1 with endpoints (`football_data`, `espn`) |
| `src/soccer.rs` | Soccer domain helpers on API data (used by the `wc` league module) |
| `src/db/` | Persistence accessors; shared entities at root; league tables under `db/<slug>/` |
| `src/league.rs` | Compile-time `League` enum — host dispatch face into league modules |
| League modules (`src/wc/`, `src/epl/`, `src/nfl/`) | League-only variation (WC eliminations; NFL calendar, teams, poll); not shared soccer logic |
| Host use cases (`registration.rs`, `standings.rs` formatters, `game_poll.rs`, `tiebreaker.rs`, `poller.rs`) | Shared sport-agnostic orchestration via `League` |
| `src/commands/` | Thin Discord adapters only |

**Command adapters vs use cases** — commands handle poise attrs, `guild_id`, defer/reply; use cases resolve season, dispatch via `League`, call `db/` / league modules, return strings.

| Concern | Adapter | Use case |
|---------|---------|----------|
| Registration | `commands/registration.rs` | `registration.rs` → `League` |
| Shared standings | `commands/standings.rs` | `League::standings` → league module (format helpers in `standings.rs`) |
| WC-only cmds | `commands/wc/` (`remaining`) | `wc/` (registered via `commands_for(League)`) |
| Season lifecycle | `commands/season.rs` | `season.rs` (`start` / `end` / `set_channel` / `list`, plus `resolve`) |

New behavior: use case first → thin handler in `commands/` → `commands::all()`.

```rust
// Adapter passes guild + optional league choice; use case resolves the season and owns rules
let message = registration::claim_for_user(ctx.data(), guild_id, league, user_id, &team).await?;
ctx.say(message).await?;
```

**Poller** — `poller.rs` lists **live** seasons, groups by league slug, calls `League::poll`; not a command. League polls map provider payloads into `game_poll::GameReport` and call `game_poll::process_game` (persist via `League`, score, announce).

## Leagues vs seasons

| Term | Meaning |
|------|---------|
| **League** | Compile-time competition type (`League` enum in `src/league.rs`, slug e.g. `wc`). Adding a league is a code change. |
| **Season** | Runtime instance of a league for one guild (`season_id`). Created via `/season start`. |
| **Live season** | Season with `polling_enabled`. At most one per league per guild (`/season start` ends the previous one). The poller processes exactly these. |
| **Target season** | The season a command acts on: `season::resolve(conn, guild_id, league)`. Explicit `league` → that league's live season (else its newest ended one). No `league` → the guild's only live season, or a `UserError` asking which. |

Catalog rows may exist in `leagues` for future slugs; only variants on `League` can have seasons (`League::supports_season` / `from_slug`).

## Seasons (multi-guild, multi-league)

Tenancy is at **season** (`seasons.guild_id`).

- Gameplay commands: every use case takes `guild_id` + `league: Option<League>` and calls `season::resolve` first. `League` derives `poise::ChoiceParameter`, so the slash option is a dropdown; it is always optional and always the last parameter.
- There is **no per-guild default season**. Single-season guilds never see the `league` option; multi-season guilds are told to pass it.
- Poller: `Season::list_live_with_meta()` then `League::from_slug` → `poll`
- Setup: `/season start <league> [name]` creates a season (slug from `season::slugify(name)`, default name `"<League> <year>"`) and ends the league's other live seasons in that guild; fresh guilds have none until then
- `season_id` keys registrations, results, tie-breakers, announcements, draft sessions
- Do not hardcode guild or season ids
- Resolution failures are `types::UserError`; poise's default `on_error` replies with its text, so use cases just `?` them

## Football-data.org soccer leagues

Shared poll, scoring, and tie-break logic lives in host modules (`game_poll`, `soccer_poll`, `standings`, `tiebreaker`, `League`). League dirs (`wc/`, `epl/`) hold variation only.

When adding behavior: does every league get this? Yes → `game_poll` / `standings` / `tiebreaker` or a `League` arm. Every football-data.org soccer league only? → `soccer_poll` / `soccer`. One league → league module or league DB type.

## NFL (ESPN)

`src/nfl/` owns the ESPN mapping: `season` (season-year rollover in March, `YYYYMMDD` scoreboard range, `GameReport` from an `NflGame`, preseason/Pro Bowl excluded), `teams`, `poll`. NFL has **no tie-breaker**: `League::tiebreaker_unit` is `None`, the tie-break arms are no-ops, and `/pick-player` replies that the league has none. `EspnNflApi` needs no token but must send a `User-Agent` (see `api/espn.rs`).

Procedure and file-level steps: **`/add-league` skill**. DB accessor rules when editing `src/db/**`: **`db-layer.mdc`**.

## Key patterns

- Resolve the target season with `season::resolve(&conn, guild_id, league)` (returns `(Season, League)`), then call enum methods (`list_teams`, `standings`, `poll`, …); `League::for_season` when you already hold a `season_id`
- `Data` holds `db` + shared `http`; soccer leagues use `FootballDataApi::from_env(data.http.clone())`, NFL uses `EspnNflApi::new(data.http.clone())`
- User-facing sport words come from `League` (`tiebreaker_unit` is `Option` — `None` hides tie-breaker lines, `draw_label`, `finished_label`) — do not hardcode "goals"/"draw" in host formatters
- Types from `crate::api`; soccer domain helpers from `crate::soccer`
- Competition code from `league_competition_code()` via league slug
- League-specific slash commands: exhaustive `commands_for(League)` in `commands/mod.rs`

## Adding a league

Invoke **`/add-league`** (skill: `.cursor/skills/add-league/`). Short checklist:

1. `League` variant + match arms in `src/league.rs`
2. League module `src/<slug>/` for variation only (soccer: thin `poll.rs` + optional league commands)
3. `db/<slug>/` accessors if needed (see `db-layer.mdc` for macro vs hand-written)
4. `commands_for(League)` for league-only commands
5. Env + README; `cargo test` / clippy

## Repo conventions

Generic coding standards → **`coding-philosophy`** (`gapuchi/ai`).

- **Tests** — `tests/migrate.rs`, `tests/standings.rs`, `tests/api.rs`
- **Errors** — `ApiError` in api; `types::Error` in commands; `types::UserError` for messages meant for the invoking user
- **Time** — `clock.rs` (`unix_timestamp_secs`, `civil_year_month`, `current_year`)
- **Releases** — `Cargo.toml`; `cargo release` or `just release`

## Common tasks

| Task | Where |
|------|-------|
| New command | Use case → `commands/` handler → `commands::all()` or `commands_for(League)` → docstring |
| New DB table / column | Add to `CREATE_SCHEMA` in `migrate.rs`, bump `SCHEMA_VERSION`, and add an `upgrade` step for existing databases (`CREATE TABLE IF NOT EXISTS` never alters existing tables) + `db/` or `db/<league>/` → re-export in `db/mod.rs` |
| New API endpoint | `api/…` + league module helpers as needed |
| New league | **`/add-league` skill** — enum arms, league module, DB, commands |
| New league poller | `League::poll` arm; build `GameReport`s and call `game_poll::process_game` (soccer leagues via `soccer_poll`) |
| Scoring / tie-breakers | Shared helpers + `League`; update `README.md` if user-visible |
| Setup / config UX | `README.md` (see `readme-sync.mdc`) |

## What not to do

- Search, filtering, or orchestration in `api/`
- Business logic in `commands/`
- HTTP or Discord in `db/`
- Bypass `season::resolve` in gameplay commands, or reintroduce a per-guild "current season"
- Hard-wire `Wc*` / `Nfl*` types into shared host paths (`registration`, host `standings`, `game_poll`, `types`, `db/registration`)
- Monolithic `db/mod.rs` with inline SQL
- Raw `reqwest::Client` + token in host code when `FootballDataApi::from_env` exists
- Let two seasons of the same league be live in one guild (they would both ingest the same games)

## Running checks

```bash
cargo test
cargo clippy -- -D warnings
```

## Cursor Cloud specific instructions

Toolchain and system deps are already provisioned in the VM snapshot; the startup update script only runs `cargo fetch`. Notes below are the non-obvious gotchas.

- **Rust edition 2024** (`Cargo.toml`) requires Rust ≥ 1.85. The base image ships an older `cargo`/`rustc` (1.83) that fails to build this repo; a newer `stable` toolchain is installed via `rustup` and set as default. If a build errors on `edition2024`, run `rustup default stable`.
- **System libraries**: `reqwest` uses `native-tls`, so `libssl-dev` + `pkg-config` must be present (missing `openssl.pc` breaks the build); `rusqlite` uses the `bundled` feature, so a C compiler (`gcc`) is required. These are already installed in the snapshot.
- **Running the bot** (`cargo run` / `./target/debug/league-bot`): it is a headless Discord gateway bot with **no local HTTP/UI**. It fail-fasts only if `DISCORD_TOKEN` is unset; `FOOTBALL_DATA_API_TOKEN` is optional at boot and errors lazily via `FootballDataApi::from_env` when a soccer command or poll actually needs it. With a placeholder `DISCORD_TOKEN` it still initializes the DB and reaches Discord auth, then exits with `Sent invalid authentication`. A real interactive end-to-end (slash commands like `/season start`, `/draft pick`, `/standings`) needs a real Discord bot token + the bot invited to a guild, plus a football-data.org token (deep data / squads + scorers need a paid tier). See `README.md` for setup and invite scopes.
- **SQLite is embedded** (no DB server). The file is created automatically at `DATABASE_PATH` (default `league_bot.db`) on first boot; there is no migration path — delete the file to reset (`src/db/migrate.rs`).
