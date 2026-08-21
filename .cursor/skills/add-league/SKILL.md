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
6. **Schema** — new tables needed, or reuse existing `nfl_*` / `nba_*` stubs in `migrate.rs`.

Do not invent an API provider or scoring rules when unclear — ask briefly, then proceed.

## Terminology (do not blur)

| Term | Meaning |
|------|---------|
| **League** | Compile-time type (`League` enum + slug) |
| **Season** | Runtime guild instance (`season_id`); created via `/config season` |
| **Command focus** | `default_season_id` — slash commands |
| **Live season** | `polling_enabled` — background poller |

## Workflow

Work bottom-up. Keep host files free of league-specific SQL/API types.

### 1. Catalog + schema

- Ensure `seed_catalog` in `src/db/migrate.rs` has the slug (nfl/nba already seeded).
- If tables are missing: extend greenfield `CREATE_SCHEMA` (no upgrade migrations; wipe DB if stale); accessors under `src/db/<slug>/`; re-export from `src/db/mod.rs`.
- Shared tables only: `leagues`, `seasons`, `guild_config`, `teams`, `registrations`.

### 2. League module (`src/<slug>/`)

Mirror `src/wc/` as needed:

| Module | Responsibility |
|--------|----------------|
| `api.rs` (or equiv.) | Build HTTP client from `Data.http` + env token (`OnceLock` OK) |
| `teams.rs` | `list_teams(data) -> Vec<CatalogTeam>` |
| `standings.rs` | `get_standings`, `user_points`, tie-break helpers → `StandingRow` |
| `poll.rs` | Ingest finished games, idempotency, Discord announces → `PollOutcome` |
| optional | tie-breaker picks, remaining/eliminations, other use cases |

Export via `src/<slug>/mod.rs`. Register `pub mod <slug>;` in `src/lib.rs`.

**Host must not import** this module’s DB types except through `League` match arms.

### 3. `League` enum (`src/league.rs`)

Add variant and update **every** exhaustive match:

- `ALL`
- `from_slug` / `slug` / `display_name`
- `list_teams`, `team_not_found_message`
- `standings`, `user_points`
- `tiebreaker_value_for_user`, `tiebreaker_pick_for_user`, `clear_picks_for_team` (no-op / empty `Ok` if unused)
- `poll`

Extend unit tests: slug resolves; unknown still `None`; `ALL` includes the new variant.

`supports_season` stays `from_slug(slug).is_some()` — no separate list.

### 4. Commands

- **Shared** (already registered): claim/assign/unclaim, teams, standings, season, config — work via `League` once arms exist.
- **League-only**: add adapters under `src/commands/<slug>/`, guard with `ensure_focused_league(ctx, League::X)`, register in `commands_for` in `src/commands/mod.rs`:

```rust
fn commands_for(league: League) -> Vec<...> {
    match league {
        League::Wc => vec![remaining(), pick_player()],
        League::Nfl => vec![/* nfl-only cmds, or vec![] */],
    }
}
```

### 5. Env + startup

- Document in `.env.example` and `README.md` (readme-sync).
- Fail-fast in `main` only if this league is compiled in and the token is required at boot (same pattern as `FOOTBALL_DATA_API_TOKEN` for wc).

### 6. Docs

- User-facing scoring/setup → `README.md` **Leagues** section.
- Agent checklist stays accurate in `AGENTS.md` (short); this skill is the full runbook.

### 7. Verify

```bash
cargo test
cargo clippy -- -D warnings
```

Manual smoke: `/config season` with the new slug → `/config channel` → claim/standings → confirm poller logs for live seasons only.

## Done when

- [ ] `League::from_slug("<slug>")` is `Some`
- [ ] `/config season` accepts the slug; catalog-only slugs still rejected
- [ ] Shared commands work for a focused season of this league
- [ ] Poller calls `League::poll` for live seasons (or explicitly no-ops with a clear outcome)
- [ ] No new `Wc*` / league SQL types in `registration.rs`, host `standings.rs`, `types.rs`, `db/registration.rs`, host `poller.rs`
- [ ] Tests + clippy clean; README/env updated if user-visible

## Do not

- Add runtime “register league” plugins or DB-only playable leagues
- Key the poller off command focus (`default_season_id`)
- Put HTTP/Discord in `src/db/`
- Copy WC tournament logic (`soccer::classify_teams`, `/remaining`) into leagues that are not WC-shaped
- Leave a `League` variant with missing match arms (won’t compile — fix all arms)

## Progressive detail

For file-by-file WC map while implementing, read [references/wc-template.md](references/wc-template.md).
