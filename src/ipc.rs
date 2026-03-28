use std::{
    collections::HashMap,
    path::Path,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc::UnboundedSender;

use anyhow::Context;
use serde_json::{Value, json};

use crate::ui::{AppEvent, MessageType, Response};

pub async fn listen(tx: UnboundedSender<AppEvent>, ctx: egui::Context) -> anyhow::Result<()> {
    let wayland_display = std::env::var("WAYLAND_DISPLAY").unwrap_or("wayland-0".to_owned());
    let cache_home = std::env::var("XDG_CACHE_HOME").unwrap_or("~/.cache".to_owned());
    let expanded_cache_home = shellexpand::tilde(&cache_home).into_owned();
    let socket_path: &Path = &Path::new(&expanded_cache_home)
        .join("qtile")
        .join(format!("qalttab.{wayland_display}"));

    if socket_path.exists() {
        std::fs::remove_file(socket_path)?;
    }
    let listener = UnixListener::bind(socket_path)?;
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
    
    let response: Value = serde_json::from_slice(slice)
        .with_context(|| format!("Failed to parse JSON from socket ({} bytes): {}", bytes_read, String::from_utf8_lossy(slice)))?;

    log::debug!("Received IPC message: {response:#?}");

    let message_type_str = response.get("message_type")
        .and_then(|v| v.as_str())
        .context("message_type missing or not a string")?;

    let message_type = match message_type_str {
        "client_focus" => MessageType::ClientFocus,
        "cycle_windows" => MessageType::CycleWindows,
        s => {
            anyhow::bail!("MessageType {} not known", s);
        }
    };

    let windows_val = response.get("windows").context("windows field missing")?;
    let windows_array = windows_val.as_array().context("windows field is not an array")?;

    let mut windows = Vec::new();
    for item in windows_array {
        if let Ok(map) = serde_json::from_value::<HashMap<String, String>>(item.clone()) {
            windows.push(map);
        } else {
            log::warn!("Skipping malformed window entry: {item}");
        }
    }

    let focus_index = response.get("focus_index")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    tx.send(AppEvent::UnixSocketMsg(Response {
        message_type,
        windows,
        focus_index,
    })).map_err(|e| anyhow::anyhow!("could not send message to GUI thread: {e}"))?;

    ctx.request_repaint();

    let success = json!({"message": "success"});
    stream.write_all(success.to_string().as_bytes()).await?;
    
    Ok(())
}
