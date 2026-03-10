use qalttab::ipc::listen;
use qalttab::ui::AppEvent;
use serde_json::json;
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::mpsc;

async fn run_single_server_test(socket_path: &str, m_type: &str) {
    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();
    let path = Path::new(socket_path);
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }

    let tx_clone = tx.clone();
    let path_clone = path.to_owned();
    tokio::spawn(async move {
        let _ = listen(tx_clone, Some(&path_clone)).await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    {
        let mut stream = UnixStream::connect(socket_path).await.unwrap();
        let payload = json!({
            "message_type": m_type,
            "windows": []
        });
        let slice = serde_json::to_vec(&payload).unwrap();
        stream.write_all(&slice).await.unwrap();
        let mut res_buf = [0; 1024];
        let _ = stream.read(&mut res_buf).await.unwrap();
    }

    let event = tokio::time::timeout(Duration::from_secs(1), rx.recv())
        .await
        .unwrap();
    assert!(matches!(event, Some(AppEvent::UnixSocketMsg(_))));
}

#[tokio::test]
async fn test_server_client_focus() {
    run_single_server_test("/tmp/q_srv_focus.sock", "client_focus").await;
}

#[tokio::test]
async fn test_server_cycle_windows() {
    run_single_server_test("/tmp/q_srv_cycle.sock", "cycle_windows").await;
}

#[tokio::test]
async fn test_server_multiple_sequential_messages() {
    let socket_path = "/tmp/q_srv_seq.sock";
    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();
    let path = Path::new(socket_path);
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }

    let tx_clone = tx.clone();
    let path_clone = path.to_owned();
    tokio::spawn(async move {
        let _ = listen(tx_clone, Some(&path_clone)).await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    for _ in 0..3 {
        let mut stream = UnixStream::connect(socket_path).await.unwrap();
        let payload = json!({ "message_type": "client_focus", "windows": [] });
        stream
            .write_all(&serde_json::to_vec(&payload).unwrap())
            .await
            .unwrap();
        let mut res_buf = [0; 1024];
        let _ = stream.read(&mut res_buf).await.unwrap();

        let event = tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .unwrap();
        assert!(matches!(event, Some(AppEvent::UnixSocketMsg(_))));
    }
}
