use qalttab::ipc::listen;
use qalttab::ui::AppEvent;
use serde_json::json;
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::mpsc;

/// Spin up a server on `socket_path`, send one message of type `m_type`, and
/// assert that exactly one `UnixSocketMsg` event is received.
async fn run_single_server_test(socket_path: &str, m_type: &str) {
    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();
    let path = Path::new(socket_path);
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }

    let ctx = egui::Context::default();
    let tx_clone = tx.clone();
    let path_clone = path.to_owned();
    let ctx_clone = ctx.clone();
    tokio::spawn(async move {
        let _ = listen(tx_clone, ctx_clone, Some(&path_clone)).await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    {
        let mut stream = UnixStream::connect(socket_path).await.unwrap();
        let payload = json!({ "message_type": m_type, "windows": [] });
        stream
            .write_all(&serde_json::to_vec(&payload).unwrap())
            .await
            .unwrap();
        let mut buf = [0; 256];
        let _ = stream.read(&mut buf).await.unwrap();
    }

    let event = tokio::time::timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("timed out waiting for event");
    assert!(matches!(event, Some(AppEvent::UnixSocketMsg(_))));
}

#[tokio::test]
async fn server_handles_client_focus() {
    run_single_server_test("/tmp/q_srv_focus.sock", "client_focus").await;
}

#[tokio::test]
async fn server_handles_cycle_windows() {
    run_single_server_test("/tmp/q_srv_cycle.sock", "cycle_windows").await;
}

#[tokio::test]
async fn server_handles_sequential_messages() {
    let socket_path = "/tmp/q_srv_seq.sock";
    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();
    let path = Path::new(socket_path);
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }

    let ctx = egui::Context::default();
    let tx_clone = tx.clone();
    let path_clone = path.to_owned();
    let ctx_clone = ctx.clone();
    tokio::spawn(async move {
        let _ = listen(tx_clone, ctx_clone, Some(&path_clone)).await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    for _ in 0..3 {
        let mut stream = UnixStream::connect(socket_path).await.unwrap();
        let payload = json!({ "message_type": "client_focus", "windows": [] });
        stream
            .write_all(&serde_json::to_vec(&payload).unwrap())
            .await
            .unwrap();
        let mut buf = [0; 256];
        let _ = stream.read(&mut buf).await.unwrap();

        let event = tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("timed out waiting for event");
        assert!(matches!(event, Some(AppEvent::UnixSocketMsg(_))));
    }
}
