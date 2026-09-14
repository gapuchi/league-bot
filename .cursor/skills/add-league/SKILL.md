---
name: add-league
description: >-
  Add a new compile-time League enum variant and league module to league-bot.
  Use when the user wants to add a league (e.g. NFL, NBA, Euros), implement a new
  League::Variant, wire poll/standings/teams, or asks how to extend multi-league
  support. Invoke with /add-league.
---

# Add a league

Leagues are **compile-time**. Seasons are **runtime**. This skill adds a playable
league to the binary by extending the `League` enum and implementing a league module.

Follow `AGENTS.md` layer rules. Reference implementation: **World Cup** (`League::Wc`, `src/wc/`, `src/db/wc/`).

## Before coding — confirm with the user

Collect (or infer from the request):

1. **Slug** — lowercase catalog id (`nfl`, `nba`, `euros`). Must match `leagues.slug` / `from_slug`.
2. **Variant name** — Rust enum variant (`Nfl`, `Nba`).
3. **Display name** — user-facing (`NFL`, `NBA`).
4. **Data source** — API + env var name(s).
5. **Capabilities** — which of: teams/claim, poll + announce, standings, tie-breaker, league-only commands.
6. **Schema** — which per-league tables the league needs (add them to `migrate.rs` with the league; no stubs exist ahead of time).

Do not invent an API provider or scoring rules when unclear — ask briefly, then proceed.

## Terminology (do not blur)

| Term | Meaning |
|------|---------|
| **League** | Compile-time type (`League` enum + slug) |
| **Season** | Runtime guild instance (`season_id`); created via `/season start <league>` |
| **Target season** | `season::resolve(conn, guild_id, league)` — explicit `league` choice, else the guild's only live season |
| **Live season** | `polling_enabled` — background poller; one per league per guild |

## Workflow

Work bottom-up. Keep host files free of league-specific SQL/API types.

### 1. Catalog + schema

- Ensure `seed_catalog` in `src/db/migrate.rs` has the slug (nba already seeded).
- Add the league's tables to `CREATE_SCHEMA`, bump `SCHEMA_VERSION`, and add an `upgrade` step in `migrate.rs` if existing tables change; accessors under `src/db/<slug>/`; re-export from `src/db/mod.rs`.
- Shared tables only: `leagues`, `seasons`, `registrations`, `draft_sessions`, `draft_participants`.

### 2. League module (`src/<slug>/`)

For **football-data.org soccer**, shared teams/standings/tie-break/poll ingest live in host modules (`League`, `standings`, `tiebreaker`, `soccer_poll`) — do not copy per slug.

| Module | Responsibility |
|--------|----------------|
| `poll.rs` | Delegate match ingest to `soccer_poll`; league-only announce hooks (e.g. WC eliminations) |
| optional | `remaining.rs`, other league-only use cases |

For **non-soccer** leagues, follow **NFL** (`src/nfl/`, `src/api/espn.rs`, `src/db/nfl/`): API client in `api/`, a `season`/calendar module that maps provider games into `game_poll::GameReport`, `teams` → `CatalogTeam`, and a `poll` that calls `game_poll::process_game`. Standings, scoring, and announce are already shared. A league may opt out of tie-breakers (`tiebreaker_unit` → `None`, no-op tie-break arms) as NFL does; to opt in, supply rosters as `RosterPlayer` via `rosters_for_teams` and the pick-player flow is shared too.

Export via `src/<slug>/mod.rs`. Register `pub mod <slug>;` in `src/lib.rs`.

**Host must not import** this module’s DB types except through `League` match arms.

### 3. `League` enum (`src/league.rs`)

Add variant and update **every** exhaustive match:

- `ALL`
- `#[name = "<Display>"]` + `#[name = "<slug>"]` on the variant (`League` is the `league` slash choice)
- `from_slug` / `slug` / `display_name`
- `tiebreaker_unit` (`Option`; `None` = no tie-breaker) / `draw_label` / `finished_label` (user-facing sport words)
- `list_teams`, `team_not_found_message`
- `standings`, `user_points` (via `finished_matches`)
- `tiebreaker_for_standings`, `tiebreaker_pick_for_user`, `clear_picks_for_team`, `rosters_for_teams`, `pick_tiebreaker_player` upsert arm (no-op / empty `Ok` if unused)
- Result store surface used by `game_poll`: `upsert_match_result(GameReport)`, `stored_match_score`, `is/mark/unmark_match_processed`, `cache_tiebreaker_totals`
- `poll`

Extend unit tests: slug resolves; unknown still `None`; `ALL` includes the new variant.

`supports_season` stays `from_slug(slug).is_some()` — no separate list.

### 4. Commands

- **Shared** (already registered): claim/assign/unclaim, team(s), undrafted, standings, pick-player, draft, season — work via `League` once arms exist; the new variant appears in every `league` dropdown automatically.
- **League-only**: add adapters under `src/commands/<slug>/`, resolve the season with `season::resolve(&conn, guild_id, Some(League::X))` (see `wc/remaining.rs`), register in `commands_for` in `src/commands/mod.rs`:

```rust
fn commands_for(league: League) -> Vec<...> {
    match league {
        League::Wc => vec![remaining()],
        League::Epl | League::Nfl => vec![],
        League::Nba => vec![/* nba-only cmds, or vec![] */],
    }
}
```

### 5. Env + startup

- Document in `.env.example` and `README.md` (readme-sync).
- Fail-fast in `main` only if this league is compiled in and the token is required at boot (same pattern as `FOOTBALL_DATA_API_TOKEN` for wc). Keyless providers (ESPN for NFL) add nothing here.

### 6. Docs

- User-facing scoring/setup → `README.md` **Leagues** section.
- Agent checklist stays accurate in `AGENTS.md` (short); this skill is the full runbook.

### 7. Verify

```bash
cargo test
cargo clippy -- -D warnings
```

Manual smoke: `/season start <league>` → `/season channel` → `/claim` / `/standings` (with and without the `league` option while another season is live) → confirm poller logs for live seasons only.

## Done when

- [ ] `League::from_slug("<slug>")` is `Some`
- [ ] `/season start` offers the league in its dropdown
- [ ] Shared commands work with `league:<new>` while another league's season is live
- [ ] Poller calls `League::poll` for live seasons (or explicitly no-ops with a clear outcome)
- [ ] No new `Wc*` / `Nfl*` / league SQL types in `registration.rs`, host `standings.rs`, `game_poll.rs`, `types.rs`, `db/registration.rs`, host `poller.rs`
- [ ] Tests + clippy clean; README/env updated if user-visible

## Do not

- Add runtime “register league” plugins or DB-only playable leagues
- Add per-guild "current season" state; the `league` option plus `season::resolve` is the whole story
- Put HTTP/Discord in `src/db/`
- Copy WC tournament logic (`soccer::classify_teams`, `/remaining`) into leagues that are not WC-shaped
- Leave a `League` variant with missing match arms (won’t compile — fix all arms)

## Progressive detail

For file-by-file WC map while implementing, read [references/wc-template.md](references/wc-template.md).
