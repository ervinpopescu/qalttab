use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc::UnboundedSender;

use anyhow::Context;
use serde_json::{Value, json};

use crate::ui::{AppEvent, MessageType, Response};

/// Returns the socket path to use.
/// If `custom` is `Some`, returns it unchanged (used in tests).
/// Otherwise derives the default from `$XDG_CACHE_HOME` and `$WAYLAND_DISPLAY`.
pub fn get_socket_path(custom: Option<&Path>) -> PathBuf {
    if let Some(p) = custom {
        return p.to_owned();
    }
    let wayland_display = std::env::var("WAYLAND_DISPLAY").unwrap_or("wayland-0".to_owned());
    let cache_home = std::env::var("XDG_CACHE_HOME").unwrap_or("~/.cache".to_owned());
    let expanded = shellexpand::tilde(&cache_home).into_owned();
    Path::new(&expanded)
        .join("qtile")
        .join(format!("qalttab.{wayland_display}"))
}

type ParsedMessage = (MessageType, Vec<HashMap<String, String>>, Option<usize>);

/// Parse a raw IPC message into its typed components.
///
/// Returns `(message_type, windows, focus_index)`.
pub fn parse_ipc_message(data: &[u8]) -> anyhow::Result<ParsedMessage> {
    let response: Value = serde_json::from_slice(data).with_context(|| {
        format!(
            "Failed to parse JSON ({} bytes): {}",
            data.len(),
            String::from_utf8_lossy(data)
        )
    })?;

    let message_type_str = response
        .get("message_type")
        .and_then(|v| v.as_str())
        .context("message_type missing or not a string")?;

    let message_type = match message_type_str {
        "client_focus" => MessageType::ClientFocus,
        "cycle_windows" => MessageType::CycleWindows,
        s => anyhow::bail!("MessageType {} not known", s),
    };

    let windows_val = response.get("windows").context("windows field missing")?;
    let windows_array = windows_val
        .as_array()
        .context("windows field is not an array")?;

    let mut windows = Vec::new();
    for item in windows_array {
        if let Ok(map) = serde_json::from_value::<HashMap<String, String>>(item.clone()) {
            windows.push(map);
        } else {
            log::warn!("Skipping malformed window entry: {item}");
        }
    }

    let focus_index = response
        .get("focus_index")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    Ok((message_type, windows, focus_index))
}

pub async fn listen(
    tx: UnboundedSender<AppEvent>,
    ctx: egui::Context,
    custom_socket_path: Option<&Path>,
) -> anyhow::Result<()> {
    let socket_path = get_socket_path(custom_socket_path);

    if let Err(e) = std::fs::remove_file(&socket_path)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        return Err(e.into());
    }
    let listener = UnixListener::bind(&socket_path)?;
    log::info!(r#"Server listening on {socket_path:?}"#);

    // Accept incoming connections in a loop
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                log::debug!("Accepted new IPC connection");
                let tx_clone = tx.clone();
                let ctx_clone = ctx.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_conn(stream, tx_clone, ctx_clone).await {
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
    tx: UnboundedSender<AppEvent>,
    ctx: egui::Context,
) -> anyhow::Result<()> {
    let mut buffer = [0; 8192];
    let bytes_read = stream.read(&mut buffer).await?;
    log::debug!("Read {} bytes from IPC stream", bytes_read);
    if bytes_read == 0 {
        return Ok(());
    }
    let slice = &buffer[..bytes_read];
    log::debug!(
        "Received IPC message ({} bytes): {}",
        bytes_read,
        String::from_utf8_lossy(slice)
    );

    let (message_type, windows, focus_index) = parse_ipc_message(slice)?;

    tx.send(AppEvent::UnixSocketMsg(Response {
        message_type,
        windows,
        focus_index,
    }))
    .map_err(|e| anyhow::anyhow!("could not send message to GUI thread: {e}"))?;

    ctx.request_repaint();

    let success = json!({"message": "success"});
    stream.write_all(success.to_string().as_bytes()).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(json: &str) -> ParsedMessage {
        parse_ipc_message(json.as_bytes()).expect("parse failed")
    }

    fn err(json: &str) -> String {
        parse_ipc_message(json.as_bytes()).unwrap_err().to_string()
    }

    #[test]
    fn parses_cycle_windows_message() {
        let json = r#"{"message_type":"cycle_windows","windows":[{"id":"1","name":"Firefox","class":"firefox","group_name":"www","group_label":""}]}"#;
        let (mt, wins, fi) = msg(json);
        assert_eq!(mt, MessageType::CycleWindows);
        assert_eq!(wins.len(), 1);
        assert_eq!(wins[0]["name"], "Firefox");
        assert_eq!(fi, None);
    }

    #[test]
    fn parses_client_focus_message() {
        let json = r#"{"message_type":"client_focus","windows":[]}"#;
        let (mt, wins, _) = msg(json);
        assert_eq!(mt, MessageType::ClientFocus);
        assert!(wins.is_empty());
    }

    #[test]
    fn parses_focus_index() {
        let json = r#"{"message_type":"cycle_windows","windows":[],"focus_index":2}"#;
        let (_, _, fi) = msg(json);
        assert_eq!(fi, Some(2));
    }

    #[test]
    fn missing_message_type_returns_error() {
        let e = err(r#"{"windows":[]}"#);
        assert!(e.contains("message_type"), "got: {e}");
    }

    #[test]
    fn unknown_message_type_returns_error() {
        let e = err(r#"{"message_type":"something_else","windows":[]}"#);
        assert!(e.contains("not known"), "got: {e}");
    }

    #[test]
    fn missing_windows_field_returns_error() {
        let e = err(r#"{"message_type":"cycle_windows"}"#);
        assert!(e.contains("windows"), "got: {e}");
    }

    #[test]
    fn malformed_window_entries_are_skipped() {
        // One valid entry, one where a value is not a string — the bad one is skipped.
        let json =
            r#"{"message_type":"cycle_windows","windows":[{"id":"1","name":"Term"},{"id":99}]}"#;
        let (_, wins, _) = msg(json);
        assert_eq!(wins.len(), 1);
        assert_eq!(wins[0]["id"], "1");
    }

    #[test]
    fn parses_multiple_window_entries() {
        let json = r#"{"message_type":"cycle_windows","windows":[
            {"id":"1","name":"A","class":"a","group_name":"www","group_label":"l1"},
            {"id":"2","name":"B","class":"b","group_name":"coding","group_label":"l2"},
            {"id":"3","name":"C","class":"c","group_name":"media","group_label":"l3"}
        ]}"#;
        let (_, wins, _) = msg(json);
        assert_eq!(wins.len(), 3);
        assert_eq!(wins[1]["class"], "b");
    }

    #[test]
    fn focus_index_zero_is_some_zero() {
        let json = r#"{"message_type":"cycle_windows","windows":[],"focus_index":0}"#;
        let (_, _, fi) = msg(json);
        assert_eq!(fi, Some(0));
    }

    #[test]
    fn windows_field_not_array_returns_error() {
        let e = err(r#"{"message_type":"cycle_windows","windows":"oops"}"#);
        assert!(e.contains("not an array"), "got: {e}");
    }

    #[test]
    fn empty_windows_array_is_valid() {
        let json = r#"{"message_type":"cycle_windows","windows":[]}"#;
        let (mt, wins, _) = msg(json);
        assert_eq!(mt, MessageType::CycleWindows);
        assert!(wins.is_empty());
    }

    #[test]
    fn all_window_fields_present() {
        let json = r#"{"message_type":"cycle_windows","windows":[
            {"id":"42","name":"Term","class":"alacritty","group_name":"coding","group_label":"\ue795"}
        ]}"#;
        let (_, wins, _) = msg(json);
        let w = &wins[0];
        assert_eq!(w["id"], "42");
        assert_eq!(w["name"], "Term");
        assert_eq!(w["class"], "alacritty");
        assert_eq!(w["group_name"], "coding");
        assert_eq!(w["group_label"], "\u{e795}");
    }

    #[test]
    fn invalid_json_returns_error() {
        assert!(parse_ipc_message(b"not json at all").is_err());
    }

    #[test]
    fn empty_input_returns_error() {
        assert!(parse_ipc_message(b"").is_err());
    }

    #[test]
    fn message_type_not_string_returns_error() {
        let e = err(r#"{"message_type":42,"windows":[]}"#);
        assert!(e.contains("message_type"), "got: {e}");
    }

    #[test]
    fn extra_unknown_top_level_fields_ignored() {
        let json =
            r#"{"message_type":"client_focus","windows":[],"extra":"ignored","another":123}"#;
        let (mt, wins, _) = msg(json);
        assert_eq!(mt, MessageType::ClientFocus);
        assert!(wins.is_empty());
    }

    #[test]
    fn extra_fields_inside_window_entry_preserved() {
        let json =
            r#"{"message_type":"cycle_windows","windows":[{"id":"7","name":"x","extra":"y"}]}"#;
        let (_, wins, _) = msg(json);
        assert_eq!(wins.len(), 1);
        assert_eq!(wins[0]["extra"], "y");
    }

    #[test]
    fn focus_index_invalid_type_becomes_none() {
        let json = r#"{"message_type":"cycle_windows","windows":[],"focus_index":"nope"}"#;
        let (_, _, fi) = msg(json);
        assert_eq!(fi, None);
    }

    #[test]
    fn all_window_entries_malformed_yields_empty_vec() {
        let json = r#"{"message_type":"cycle_windows","windows":[1,2,3]}"#;
        let (_, wins, _) = msg(json);
        assert!(wins.is_empty());
    }

    #[test]
    fn empty_message_type_string_returns_error() {
        let e = err(r#"{"message_type":"","windows":[]}"#);
        assert!(e.contains("not known"), "got: {e}");
    }

    #[test]
    fn message_type_null_returns_error() {
        let e = err(r#"{"message_type":null,"windows":[]}"#);
        assert!(e.contains("message_type"), "got: {e}");
    }

    #[test]
    fn whitespace_only_input_returns_error() {
        assert!(parse_ipc_message(b"   \n\t  ").is_err());
    }

    #[test]
    fn top_level_array_returns_error() {
        let e = err(r#"[1,2,3]"#);
        assert!(e.contains("message_type"), "got: {e}");
    }
}
