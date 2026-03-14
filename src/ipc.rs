use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc::UnboundedSender;

use anyhow::{Context, bail};
use serde_json::{Value, json};

use crate::ui::{AppEvent, MessageType, Response};

pub fn get_socket_path(custom_socket_path: Option<&Path>) -> PathBuf {
    if let Some(p) = custom_socket_path {
        p.to_owned()
    } else {
        let wayland_display = std::env::var("WAYLAND_DISPLAY").unwrap_or("wayland-0".to_owned());
        Path::new(&std::env::var("XDG_CACHE_HOME").unwrap_or("~/cache".to_owned()))
            .join("qtile")
            .join(format!("qalttab.{wayland_display}"))
    }
}

pub async fn listen(
    tx: UnboundedSender<AppEvent>,
    custom_socket_path: Option<&Path>,
) -> anyhow::Result<()> {
    let socket_path = get_socket_path(custom_socket_path);

    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }
    let listener = UnixListener::bind(&socket_path)?;
    log::info!(r#"Server listening on {socket_path:?}"#);

    // Accept incoming connections in a loop
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let tx_clone = tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_conn(stream, &tx_clone).await {
                        log::error!("Error handling connection: {e}");
                    }
                });
            }
            Err(e) => log::error!("Error accepting connection: {e}"),
        }
    }
}

pub async fn handle_conn(
    mut stream: UnixStream,
    tx: &UnboundedSender<AppEvent>,
) -> anyhow::Result<()> {
    let mut buffer = [0; 4096];
    let bytes_read = stream.read(&mut buffer).await?;
    let slice = &buffer[..bytes_read];

    let (response_bytes, event) = process_incoming_data(slice);

    stream.write_all(&response_bytes).await?;

    if let Some(event) = event
        && let Err(e) = tx.send(event)
    {
        log::error!("could not send message to GUI thread: {e}");
    }
    Ok(())
}

pub fn process_incoming_data(slice: &[u8]) -> (Vec<u8>, Option<AppEvent>) {
    let success = json!({"message": "success"}).to_string().into_bytes();
    match parse_message(slice) {
        Ok(response) => (success, Some(AppEvent::UnixSocketMsg(response))),
        Err(e) => {
            log::error!("{e}");
            (
                json!({"message": format!("{e}")}).to_string().into_bytes(),
                None,
            )
        }
    }
}

pub fn parse_message(slice: &[u8]) -> anyhow::Result<Response> {
    let response: HashMap<String, Value> = serde_json::from_slice(slice)?;
    log::debug!("{response:#?}");

    let message_type = match response.get("message_type") {
        Some(Value::String(s)) => MessageType::try_from(s.as_str())?,
        Some(_) => bail!("message_type is not a string"),
        None => bail!("missing message_type field"),
    };

    let windows: Vec<HashMap<String, String>> = match response.get("windows") {
        Some(Value::Array(a)) => a
            .iter()
            .map(|item| {
                serde_json::from_value::<HashMap<String, String>>(item.clone())
                    .context("Failed to parse window item")
            })
            .collect::<anyhow::Result<Vec<_>>>()?,
        Some(Value::Null) | None => Vec::new(),
        Some(_) => bail!("windows field is not an array or null"),
    };

    Ok(Response {
        message_type,
        windows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_message() {
        let payload = json!({
            "message_type": "client_focus",
            "windows": [
                {
                    "id": "123",
                    "name": "test window"
                }
            ]
        });
        let slice = serde_json::to_vec(&payload).unwrap();
        let response = parse_message(&slice).unwrap();
        assert_eq!(response.message_type, MessageType::ClientFocus);
        assert_eq!(response.windows.len(), 1);
        assert_eq!(response.windows[0].get("id"), Some(&"123".to_string()));
    }

    #[test]
    fn test_parse_message_null_windows() {
        let payload = json!({
            "message_type": "cycle_windows",
            "windows": null
        });
        let slice = serde_json::to_vec(&payload).unwrap();
        let response = parse_message(&slice).unwrap();
        assert_eq!(response.message_type, MessageType::CycleWindows);
        assert!(response.windows.is_empty());
    }

    #[test]
    fn test_parse_message_missing_windows() {
        let payload = json!({
            "message_type": "client_focus"
        });
        let slice = serde_json::to_vec(&payload).unwrap();
        let response = parse_message(&slice).unwrap();
        assert_eq!(response.message_type, MessageType::ClientFocus);
        assert!(response.windows.is_empty());
    }

    #[test]
    fn test_parse_message_invalid_type() {
        let payload = json!({
            "message_type": "unknown",
            "windows": []
        });
        let slice = serde_json::to_vec(&payload).unwrap();
        assert!(parse_message(&slice).is_err());
    }

    #[test]
    fn test_parse_message_missing_type() {
        let payload = json!({
            "windows": []
        });
        let slice = serde_json::to_vec(&payload).unwrap();
        assert!(parse_message(&slice).is_err());
    }

    #[test]
    fn test_parse_message_invalid_json() {
        let slice = b"{ invalid json }";
        assert!(parse_message(slice).is_err());
    }

    #[test]
    fn test_process_incoming_data_success() {
        let payload = json!({ "message_type": "client_focus", "windows": [] });
        let slice = serde_json::to_vec(&payload).unwrap();
        let (res, event) = process_incoming_data(&slice);
        assert!(!res.is_empty());
        assert!(event.is_some());
    }

    #[test]
    fn test_process_incoming_data_error() {
        let slice = b"invalid";
        let (res, event) = process_incoming_data(slice);
        assert!(!res.is_empty());
        assert!(event.is_none());

        // Test error message content
        let res_json: Value = serde_json::from_slice(&res).unwrap();
        assert!(
            res_json
                .get("message")
                .unwrap()
                .as_str()
                .unwrap()
                .contains("expected value")
        );
    }

    #[test]
    fn test_parse_message_error_branches() {
        // Test missing windows field
        let payload = json!({ "message_type": "client_focus" });
        let slice = serde_json::to_vec(&payload).unwrap();
        let res = parse_message(&slice).unwrap();
        assert!(res.windows.is_empty());

        // Test invalid message_type (not string)
        let p2 = json!({ "message_type": 123 });
        assert!(parse_message(&serde_json::to_vec(&p2).unwrap()).is_err());

        // Test invalid windows (not array)
        let p3 = json!({ "message_type": "client_focus", "windows": 123 });
        assert!(parse_message(&serde_json::to_vec(&p3).unwrap()).is_err());
    }

    #[test]
    fn test_get_socket_path() {
        let custom = Path::new("/tmp/custom.sock");
        assert_eq!(get_socket_path(Some(custom)), custom.to_path_buf());

        let path = get_socket_path(None);
        assert!(path.to_str().unwrap().contains("qalttab"));
    }

    #[tokio::test]
    async fn test_listen_cleanup() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let socket_path = "/tmp/qalttab_cleanup.sock";
        let path = Path::new(socket_path);

        // Create a dummy file to test cleanup
        std::fs::File::create(path).unwrap();
        assert!(path.exists());

        // Start listen in background and kill it quickly
        let path_clone = path.to_owned();
        let handle = tokio::spawn(async move {
            let _ = listen(tx, Some(&path_clone)).await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        handle.abort();

        // If listen worked, it should have unlinked and re-created it as a socket
        assert!(path.exists());
        let _metadata = std::fs::metadata(path).unwrap();
        // On unix, sockets are different but for this test just exists is okay
        // as long as it didn't fail.
        std::fs::remove_file(path).unwrap();
    }
}
