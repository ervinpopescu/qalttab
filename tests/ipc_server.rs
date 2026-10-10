use qalttab::ipc::{handle_conn, listen};
use qalttab::ui::{AppEvent, MessageType};
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

// ── handle_conn unit-level tests using an in-process socket pair ──────────────

#[tokio::test]
async fn handle_conn_zero_bytes_returns_ok() {
    let (client, server) = tokio::net::UnixStream::pair().unwrap();
    drop(client); // EOF immediately → 0 bytes read
    let (tx, _rx) = mpsc::unbounded_channel::<AppEvent>();
    let ctx = egui::Context::default();
    assert!(handle_conn(server, tx, ctx).await.is_ok());
}

#[tokio::test]
async fn handle_conn_invalid_json_returns_error() {
    let (mut client, server) = tokio::net::UnixStream::pair().unwrap();
    let (tx, _rx) = mpsc::unbounded_channel::<AppEvent>();
    let ctx = egui::Context::default();
    client.write_all(b"not json at all").await.unwrap();
    drop(client);
    assert!(handle_conn(server, tx, ctx).await.is_err());
}

#[tokio::test]
async fn handle_conn_valid_cycle_windows_sends_event_and_replies_success() {
    let (mut client, server) = tokio::net::UnixStream::pair().unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();
    let ctx = egui::Context::default();

    let payload = json!({
        "message_type": "cycle_windows",
        "windows": [{"id": "1", "name": "Term", "class": "alacritty", "group_name": "coding", "group_label": ""}],
        "focus_index": 0
    });
    client
        .write_all(&serde_json::to_vec(&payload).unwrap())
        .await
        .unwrap();
    client.shutdown().await.unwrap(); // signal EOF after the payload

    assert!(handle_conn(server, tx, ctx).await.is_ok());

    // Server must have written {"message":"success"} back to the client half
    let mut buf = [0u8; 256];
    let n = client.read(&mut buf).await.unwrap();
    let reply: serde_json::Value = serde_json::from_slice(&buf[..n]).unwrap();
    assert_eq!(reply["message"], "success");

    // And the event must have been placed on the channel
    let event = rx.try_recv().expect("event should be present");
    if let AppEvent::UnixSocketMsg(r) = event {
        assert_eq!(r.message_type, MessageType::CycleWindows);
        assert_eq!(r.focus_index, Some(0));
        assert_eq!(r.windows.len(), 1);
        assert_eq!(r.windows[0]["class"], "alacritty");
    } else {
        panic!("expected UnixSocketMsg, got {:?}", event);
    }
}

#[tokio::test]
async fn handle_conn_valid_client_focus_sends_event() {
    let (mut client, server) = tokio::net::UnixStream::pair().unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();
    let ctx = egui::Context::default();

    let payload = json!({
        "message_type": "client_focus",
        "windows": [{"id": "5", "name": "Browser"}]
    });
    client
        .write_all(&serde_json::to_vec(&payload).unwrap())
        .await
        .unwrap();
    client.shutdown().await.unwrap();

    assert!(handle_conn(server, tx, ctx).await.is_ok());

    let event = rx.try_recv().expect("event should be present");
    if let AppEvent::UnixSocketMsg(r) = event {
        assert_eq!(r.message_type, MessageType::ClientFocus);
        assert_eq!(r.focus_index, None);
        assert_eq!(r.windows[0]["id"], "5");
    } else {
        panic!("expected UnixSocketMsg, got {:?}", event);
    }
}

#[tokio::test]
async fn handle_conn_unknown_message_type_returns_error() {
    let (mut client, server) = tokio::net::UnixStream::pair().unwrap();
    let (tx, _rx) = mpsc::unbounded_channel::<AppEvent>();
    let ctx = egui::Context::default();

    let payload = json!({ "message_type": "totally_unknown", "windows": [] });
    client
        .write_all(&serde_json::to_vec(&payload).unwrap())
        .await
        .unwrap();
    drop(client);

    assert!(handle_conn(server, tx, ctx).await.is_err());
}
