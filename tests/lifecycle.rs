use std::sync::Arc;

use rusqlite::Connection;
use tokio::sync::Mutex;

use league_bot::{
    db::{
        self, DraftOrderKind, DraftParticipant, DraftSession, DraftSessionStatus, Registration,
        RosterPhase, Season,
    },
    draft,
    league::League,
    season,
    types::{Data, UserError},
};

const GUILD: u64 = 111;

fn test_data(conn: Connection) -> Data {
    Data {
        db: Arc::new(Mutex::new(conn)),
        http: reqwest::Client::new(),
    }
}

fn fresh() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();
    conn
}

fn wc_season(conn: &Connection) -> Season {
    Season::get_or_create(conn, GUILD, "wc", "world-cup-2026", "World Cup 2026").unwrap()
}

fn seed_active_draft(conn: &Connection, season_id: i64, user_ids: &[u64]) {
    DraftSession::upsert(
        conn,
        season_id,
        DraftOrderKind::Snake,
        DraftSessionStatus::Active,
        "0",
    )
    .unwrap();
    DraftParticipant::replace_all(conn, season_id, user_ids).unwrap();
    Season::set_roster_phase(conn, season_id, RosterPhase::Drafting).unwrap();
}

fn user_message(error: league_bot::types::Error) -> String {
    error
        .downcast_ref::<UserError>()
        .unwrap_or_else(|| panic!("expected UserError, got {error}"))
        .to_string()
}

// --- season resolution ---

#[test]
fn resolve_infers_the_only_live_season() {
    let conn = fresh();
    let wc = wc_season(&conn);

    let (season, league) = season::resolve(&conn, GUILD, None).unwrap();
    assert_eq!(season.id, wc.id);
    assert_eq!(league, League::Wc);
}

#[test]
fn resolve_requires_league_when_several_seasons_are_live() {
    let conn = fresh();
    let wc = wc_season(&conn);
    let nfl = Season::get_or_create(&conn, GUILD, "nfl", "nfl-2026", "NFL 2026").unwrap();

    let message = user_message(season::resolve(&conn, GUILD, None).unwrap_err());
    assert!(message.contains("Which league?"), "{message}");
    assert!(message.contains("FIFA World Cup") && message.contains("NFL"), "{message}");

    assert_eq!(
        season::resolve(&conn, GUILD, Some(League::Nfl)).unwrap().0.id,
        nfl.id
    );
    assert_eq!(
        season::resolve(&conn, GUILD, Some(League::Wc)).unwrap().0.id,
        wc.id
    );
}

#[test]
fn resolve_reports_missing_seasons() {
    let conn = fresh();
    let message = user_message(season::resolve(&conn, GUILD, None).unwrap_err());
    assert!(message.contains("No live season"), "{message}");

    let message = user_message(season::resolve(&conn, GUILD, Some(League::Epl)).unwrap_err());
    assert!(message.contains("No Premier League season"), "{message}");
}

#[test]
fn resolve_by_league_prefers_live_then_newest() {
    let conn = fresh();
    let old = Season::get_or_create(&conn, GUILD, "nfl", "nfl-2025", "NFL 2025").unwrap();
    let new = Season::get_or_create(&conn, GUILD, "nfl", "nfl-2026", "NFL 2026").unwrap();

    Season::set_polling_enabled(&conn, new.id, false).unwrap();
    assert_eq!(
        season::resolve(&conn, GUILD, Some(League::Nfl)).unwrap().0.id,
        old.id
    );

    Season::set_polling_enabled(&conn, old.id, false).unwrap();
    assert_eq!(
        season::resolve(&conn, GUILD, Some(League::Nfl)).unwrap().0.id,
        new.id
    );
}

// --- season lifecycle ---

#[tokio::test]
async fn season_start_creates_named_season() {
    let data = test_data(fresh());
    let message = season::start_for_guild(&data, GUILD, League::Wc, Some("World Cup 2026"))
        .await
        .unwrap();
    assert!(message.contains("Started **World Cup 2026**"), "{message}");
    assert!(message.contains("Match polling enabled"));

    let conn = data.db.lock().await;
    let (season, league) = season::resolve(&conn, GUILD, None).unwrap();
    assert_eq!(league, League::Wc);
    assert!(season.polling_enabled);
    assert_eq!(season.slug, "world-cup-2026");
}

#[tokio::test]
async fn season_start_defaults_name_to_league_and_year() {
    let data = test_data(fresh());
    season::start_for_guild(&data, GUILD, League::Nfl, None)
        .await
        .unwrap();

    let conn = data.db.lock().await;
    let (season, _) = season::resolve(&conn, GUILD, Some(League::Nfl)).unwrap();
    assert!(season.name.starts_with("NFL 20"), "{}", season.name);
    assert!(season.slug.starts_with("nfl-20"), "{}", season.slug);
}

#[tokio::test]
async fn season_start_ends_the_previous_live_season_of_that_league() {
    let data = test_data(fresh());
    season::start_for_guild(&data, GUILD, League::Nfl, Some("NFL 2025"))
        .await
        .unwrap();
    season::start_for_guild(&data, GUILD, League::Wc, Some("World Cup 2026"))
        .await
        .unwrap();

    let message = season::start_for_guild(&data, GUILD, League::Nfl, Some("NFL 2026"))
        .await
        .unwrap();
    assert!(message.contains("Ended **NFL 2025**"), "{message}");

    let conn = data.db.lock().await;
    let live: Vec<String> = Season::list_live_for_guild(&conn, GUILD)
        .unwrap()
        .into_iter()
        .map(|meta| meta.season.name)
        .collect();
    assert_eq!(live, vec!["World Cup 2026", "NFL 2026"]);
}

#[tokio::test]
async fn season_start_resumes_ended_season() {
    let conn = fresh();
    let season = wc_season(&conn);
    Season::set_polling_enabled(&conn, season.id, false).unwrap();

    let data = test_data(conn);
    let message = season::start_for_guild(&data, GUILD, League::Wc, Some("World Cup 2026"))
        .await
        .unwrap();
    assert!(message.contains("Resumed"), "{message}");

    let conn = data.db.lock().await;
    assert!(Season::get(&conn, season.id).unwrap().unwrap().polling_enabled);
}

#[tokio::test]
async fn season_start_is_idempotent_when_already_running() {
    let conn = fresh();
    wc_season(&conn);

    let data = test_data(conn);
    let message = season::start_for_guild(&data, GUILD, League::Wc, Some("World Cup 2026"))
        .await
        .unwrap();
    assert!(message.contains("already running"), "{message}");
}

#[tokio::test]
async fn season_end_stops_polling_and_is_idempotent() {
    let conn = fresh();
    let season = wc_season(&conn);

    let data = test_data(conn);
    let message = season::end_for_guild(&data, GUILD, None).await.unwrap();
    assert!(message.contains("match polling stopped"), "{message}");
    {
        let conn = data.db.lock().await;
        assert!(!Season::get(&conn, season.id).unwrap().unwrap().polling_enabled);
        assert!(Season::list_live_with_meta(&conn).unwrap().is_empty());
    }

    // Nothing is live now, so the league must be named to reach the ended season.
    let message = season::end_for_guild(&data, GUILD, Some(League::Wc))
        .await
        .unwrap();
    assert!(message.contains("already ended"), "{message}");
}

#[tokio::test]
async fn season_channel_and_list_report_state() {
    let conn = fresh();
    wc_season(&conn);
    let data = test_data(conn);

    let message = season::set_channel_for_guild(&data, GUILD, None, 4242)
        .await
        .unwrap();
    assert!(message.contains("<#4242>"), "{message}");

    let listing = season::list_for_guild(&data, GUILD).await.unwrap();
    assert!(listing.contains("World Cup 2026"), "{listing}");
    assert!(listing.contains("live"), "{listing}");
    assert!(listing.contains("<#4242>"), "{listing}");
}

// --- draft lifecycle ---

#[tokio::test]
async fn draft_end_freezes_active_draft() {
    let conn = fresh();
    let season = wc_season(&conn);
    seed_active_draft(&conn, season.id, &[1, 2, 3]);

    let data = test_data(conn);
    let message = draft::freeze_for_guild(&data, GUILD, None).await.unwrap();
    assert!(message.contains("frozen"));

    let conn = data.db.lock().await;
    let season = Season::get(&conn, season.id).unwrap().unwrap();
    assert_eq!(season.roster_phase, RosterPhase::Frozen);
    let session = DraftSession::get(&conn, season.id).unwrap().unwrap();
    assert_eq!(session.status, DraftSessionStatus::Complete);
}

#[tokio::test]
async fn draft_end_is_idempotent_when_frozen() {
    let conn = fresh();
    let season = wc_season(&conn);
    Season::set_roster_phase(&conn, season.id, RosterPhase::Frozen).unwrap();

    let data = test_data(conn);
    let message = draft::freeze_for_guild(&data, GUILD, None).await.unwrap();
    assert!(message.contains("already"));
}

#[tokio::test]
async fn draft_cancel_clears_picks_and_reopens_roster() {
    let conn = fresh();
    let season = wc_season(&conn);
    seed_active_draft(&conn, season.id, &[10, 20]);
    Registration::upsert(&conn, season.id, 10, 1, "Team One").unwrap();

    let data = test_data(conn);
    let message = draft::cancel_for_guild(&data, GUILD, None).await.unwrap();
    assert!(message.contains("cancelled"), "{message}");

    let conn = data.db.lock().await;
    assert!(Registration::list_for_season(&conn, season.id).unwrap().is_empty());
    assert!(DraftSession::get(&conn, season.id).unwrap().is_none());
    assert_eq!(
        Season::get(&conn, season.id).unwrap().unwrap().roster_phase,
        RosterPhase::Open
    );
}

#[tokio::test]
async fn draft_cancel_without_session_is_a_notice() {
    let conn = fresh();
    wc_season(&conn);
    let data = test_data(conn);
    let message = draft::cancel_for_guild(&data, GUILD, None).await.unwrap();
    assert!(message.contains("No draft to cancel"), "{message}");
}

#[tokio::test]
async fn draft_unpick_allows_only_last_picker() {
    let conn = fresh();
    let season = wc_season(&conn);
    seed_active_draft(&conn, season.id, &[10, 20]);
    let order = DraftParticipant::user_ids_ordered(&conn, season.id).unwrap();
    let (first, second) = (order[0], order[1]);
    Registration::upsert(&conn, season.id, first, 1, "Team One").unwrap();
    Registration::upsert(&conn, season.id, second, 2, "Team Two").unwrap();

    let data = test_data(conn);
    let denied = draft::unpick_for_user(&data, GUILD, None, first).await.unwrap();
    assert!(denied.contains("Only the last picker"), "{denied}");
    {
        let conn = data.db.lock().await;
        assert_eq!(Registration::list_for_season(&conn, season.id).unwrap().len(), 2);
    }

    let allowed = draft::unpick_for_user(&data, GUILD, None, second).await.unwrap();
    assert!(allowed.contains("unpicked"), "{allowed}");
    assert!(allowed.contains("Team Two"));
    {
        let conn = data.db.lock().await;
        let remaining = Registration::list_for_season(&conn, season.id).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].team_id, 1);
        assert_eq!(remaining[0].user_id, first);
    }

    let after_unpick = draft::unpick_for_user(&data, GUILD, None, first).await.unwrap();
    assert!(after_unpick.contains("Team One"));
    {
        let conn = data.db.lock().await;
        assert!(Registration::list_for_season(&conn, season.id).unwrap().is_empty());
    }
}

#[tokio::test]
async fn draft_unpick_rejects_when_no_picks_or_frozen() {
    let conn = fresh();
    let season = wc_season(&conn);
    seed_active_draft(&conn, season.id, &[10, 20]);

    let data = test_data(conn);
    let empty = draft::unpick_for_user(&data, GUILD, None, 10).await.unwrap();
    assert!(empty.contains("No picks"));

    draft::freeze_for_guild(&data, GUILD, None).await.unwrap();
    let frozen = draft::unpick_for_user(&data, GUILD, None, 10).await.unwrap();
    assert!(frozen.contains("frozen"));
}

#[tokio::test]
async fn draft_commands_are_scoped_to_the_named_league() {
    let conn = fresh();
    let wc = wc_season(&conn);
    let nfl = Season::get_or_create(&conn, GUILD, "nfl", "nfl-2026", "NFL 2026").unwrap();
    seed_active_draft(&conn, nfl.id, &[10, 20]);

    let data = test_data(conn);
    let message = draft::freeze_for_guild(&data, GUILD, Some(League::Nfl))
        .await
        .unwrap();
    assert!(message.contains("frozen"), "{message}");

    let conn = data.db.lock().await;
    assert_eq!(
        Season::get(&conn, nfl.id).unwrap().unwrap().roster_phase,
        RosterPhase::Frozen
    );
    assert_eq!(
        Season::get(&conn, wc.id).unwrap().unwrap().roster_phase,
        RosterPhase::Open
    );
}

#[test]
fn registration_latest_follows_insert_order() {
    let conn = fresh();
    let season = wc_season(&conn);
    assert!(Registration::latest_for_season(&conn, season.id).unwrap().is_none());

    Registration::upsert(&conn, season.id, 10, 1, "Alpha").unwrap();
    Registration::upsert(&conn, season.id, 20, 2, "Zulu").unwrap();

    let latest = Registration::latest_for_season(&conn, season.id).unwrap().unwrap();
    assert_eq!(latest.user_id, 20);
    assert_eq!(latest.team_id, 2);
}
