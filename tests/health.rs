use std::sync::Arc;
use std::time::Duration;

use league_bot::{db, health, health::PollerHealth, types::Data};
use rusqlite::Connection;

async fn spawn_server(poller: PollerHealth) -> String {
    let conn = Connection::open_in_memory().expect("open in-memory db");
    db::init(&conn).expect("init db");
    let data = Data {
        db: Arc::new(tokio::sync::Mutex::new(conn)),
        http: reqwest::Client::new(),
    };

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let addr = listener.local_addr().unwrap().to_string();
    drop(listener);

    health::start_health_server(Arc::new(data), addr.clone(), poller);
    tokio::time::sleep(Duration::from_millis(100)).await;
    addr
}

#[tokio::test]
async fn health_endpoint_reports_ok() {
    let addr = spawn_server(PollerHealth::default()).await;
    let response = reqwest::get(format!("http://{addr}/health"))
        .await
        .expect("request /health");

    assert_eq!(response.status(), 200);
    assert_eq!(
        response.text().await.unwrap(),
        "{\"status\":\"ok\",\"database\":\"ok\",\"poller\":\"ok\"}"
    );
}

#[tokio::test]
async fn failing_poller_fails_health() {
    let poller = PollerHealth::default();
    poller.mark_failed();
    let addr = spawn_server(poller).await;

    let response = reqwest::get(format!("http://{addr}/health"))
        .await
        .expect("request /health");

    assert_eq!(response.status(), 503);
    assert_eq!(
        response.text().await.unwrap(),
        "{\"status\":\"unavailable\",\"database\":\"ok\",\"poller\":\"failing\"}"
    );
}

#[tokio::test]
async fn unknown_path_is_not_found() {
    let addr = spawn_server(PollerHealth::default()).await;
    let response = reqwest::get(format!("http://{addr}/nope"))
        .await
        .expect("request /nope");

    assert_eq!(response.status(), 404);
}
