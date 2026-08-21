use std::sync::Arc;

use rusqlite::Connection;
use tokio::sync::Mutex;

use league_bot::{
    db::{
        self, DraftOrderKind, DraftParticipant, DraftSession, DraftSessionStatus, GuildConfig,
        RosterPhase, Season,
    },
    draft,
    season,
    types::Data,
};

fn test_data(conn: Connection) -> Data {
    Data {
        db: Arc::new(Mutex::new(conn)),
        http: reqwest::Client::new(),
    }
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

#[tokio::test]
async fn draft_end_freezes_active_draft() {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();

    let guild_id = 111_u64;
    let season = Season::get_or_create(&conn, guild_id, "wc", "wc-2026", "World Cup 2026").unwrap();
    GuildConfig::set_default_season_id(&conn, guild_id, season.id).unwrap();

    let data = test_data(conn);
    {
        let conn = data.db.lock().await;
        seed_active_draft(&conn, season.id, &[1, 2, 3]);
    }

    let message = draft::freeze_for_guild(&data, guild_id).await.unwrap();
    assert!(message.contains("frozen"));

    let conn = data.db.lock().await;
    let season = Season::get(&conn, season.id).unwrap().unwrap();
    assert_eq!(season.roster_phase, RosterPhase::Frozen);
    let session = DraftSession::get(&conn, season.id).unwrap().unwrap();
    assert_eq!(session.status, DraftSessionStatus::Complete);
}

#[tokio::test]
async fn draft_end_is_idempotent_when_frozen() {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();

    let guild_id = 111_u64;
    let season = Season::get_or_create(&conn, guild_id, "wc", "wc-2026", "World Cup 2026").unwrap();
    GuildConfig::set_default_season_id(&conn, guild_id, season.id).unwrap();
    Season::set_roster_phase(&conn, season.id, RosterPhase::Frozen).unwrap();

    let data = test_data(conn);
    let message = draft::freeze_for_guild(&data, guild_id).await.unwrap();
    assert!(message.contains("already"));
}

#[tokio::test]
async fn season_start_creates_season_and_sets_focus() {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();

    let guild_id = 111_u64;
    let data = test_data(conn);
    let message = season::start_for_guild(&data, guild_id, "wc", "wc-2026", "World Cup 2026")
        .await
        .unwrap();
    assert!(message.contains("Started season"));
    assert!(message.contains("Match polling enabled"));

    let conn = data.db.lock().await;
    let season = Season::default_for_guild(&conn, guild_id).unwrap();
    assert!(season.polling_enabled);
    assert_eq!(season.slug, "wc-2026");
}

#[tokio::test]
async fn season_start_resumes_ended_season() {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();

    let guild_id = 111_u64;
    let season = Season::get_or_create(&conn, guild_id, "wc", "wc-2026", "World Cup 2026").unwrap();
    GuildConfig::set_default_season_id(&conn, guild_id, season.id).unwrap();
    Season::set_polling_enabled(&conn, season.id, false).unwrap();

    let data = test_data(conn);
    let message = season::start_for_guild(&data, guild_id, "wc", "wc-2026", "World Cup 2026")
        .await
        .unwrap();
    assert!(message.contains("Resumed season"));

    let conn = data.db.lock().await;
    let season = Season::get(&conn, season.id).unwrap().unwrap();
    assert!(season.polling_enabled);
}

#[tokio::test]
async fn season_start_is_idempotent_when_already_running() {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();

    let guild_id = 111_u64;
    let season = Season::get_or_create(&conn, guild_id, "wc", "wc-2026", "World Cup 2026").unwrap();
    GuildConfig::set_default_season_id(&conn, guild_id, season.id).unwrap();

    let data = test_data(conn);
    let message = season::start_for_guild(&data, guild_id, "wc", "wc-2026", "World Cup 2026")
        .await
        .unwrap();
    assert!(message.contains("already running"));
}

#[tokio::test]
async fn season_status_reports_polling_state() {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();

    let guild_id = 111_u64;
    let season = Season::get_or_create(&conn, guild_id, "wc", "wc-2026", "World Cup 2026").unwrap();
    GuildConfig::set_default_season_id(&conn, guild_id, season.id).unwrap();
    Season::set_polling_enabled(&conn, season.id, false).unwrap();

    let data = test_data(conn);
    let message = season::status_for_guild(&data, guild_id).await.unwrap();
    assert!(message.contains("match polling **off**"));
}

#[tokio::test]
async fn season_end_stops_polling_for_focused_season() {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();

    let guild_id = 111_u64;
    let season = Season::get_or_create(&conn, guild_id, "wc", "wc-2026", "World Cup 2026").unwrap();
    GuildConfig::set_default_season_id(&conn, guild_id, season.id).unwrap();
    assert!(season.polling_enabled);

    let data = test_data(conn);
    let message = season::end_for_guild(&data, guild_id).await.unwrap();
    assert!(message.contains("match polling stopped"));

    let conn = data.db.lock().await;
    let season = Season::get(&conn, season.id).unwrap().unwrap();
    assert!(!season.polling_enabled);
    assert!(Season::list_live_with_meta(&conn).unwrap().is_empty());
}

#[tokio::test]
async fn season_end_is_idempotent_when_already_ended() {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();

    let guild_id = 111_u64;
    let season = Season::get_or_create(&conn, guild_id, "wc", "wc-2026", "World Cup 2026").unwrap();
    GuildConfig::set_default_season_id(&conn, guild_id, season.id).unwrap();
    Season::set_polling_enabled(&conn, season.id, false).unwrap();

    let data = test_data(conn);
    let message = season::end_for_guild(&data, guild_id).await.unwrap();
    assert!(message.contains("already ended"));
}
