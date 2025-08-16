use std::{
    collections::HashMap,
    io::{Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::Path,
};
use tokio::sync::mpsc::UnboundedSender;

use anyhow::bail;
use serde_json::{Value, json};

use crate::ui::{AppEvent, MessageType, Response};

pub async fn listen(tx: UnboundedSender<AppEvent>) -> anyhow::Result<()> {
    let wayland_display = std::env::var("WAYLAND_DISPLAY").unwrap_or("wayland-0".to_owned());
    let socket_path: &Path =
        &Path::new(&std::env::var("XDG_CACHE_HOME").unwrap_or("~/cache".to_owned()))
            .join("qtile")
            .join(format!("qalttab.{wayland_display}"));

    if socket_path.exists() {
        std::fs::remove_file(socket_path)?;
    }
    let listener = UnixListener::bind(socket_path)?;
    log::info!(r#"Server listening on {socket_path:?}"#);

    // Accept incoming connections in a loop
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                handle_conn(stream, &tx).await?;
            }
            Err(e) => log::error!("Error accepting connection: {e}"),
        }
    }
    std::fs::remove_file(socket_path).unwrap_or_else(|_| {
        log::error!("Could not remove the socket file");
    });
    Ok(())
}

pub async fn handle_conn(
    mut stream: UnixStream,
    tx: &UnboundedSender<AppEvent>,
) -> anyhow::Result<()> {
    let mut buffer = [0; 4096];
    let bytes_read = stream.read(&mut buffer)?;
    let slice = &buffer[..bytes_read];
    let success = json!({"message": "success"});
    stream.write_fmt(format_args!("{success}"))?;
    let response: Result<HashMap<String, Value>, serde_json::Error> = serde_json::from_slice(slice);
    match response {
        Ok(response) => {
            log::debug!("{response:#?}");
            let message_type = match response.get("message_type").expect("correct message") {
                Value::String(s) => match s.as_str() {
                    "client_focus" => MessageType::ClientFocus,
                    "cycle_windows" => MessageType::CycleWindows,
                    s => {
                        bail!("MessageType {} not known", s);
                    }
                },
                Value::Null
                | Value::Bool(_)
                | Value::Number(_)
                | Value::Array(_)
                | Value::Object(_) => todo!(),
            };
            let windows: Vec<HashMap<String, String>> =
                match response.get("windows").expect("correct message") {
                    Value::Null => todo!(),
                    Value::Bool(_) => todo!(),
                    Value::Number(_) => todo!(),
                    Value::String(_) => todo!(),
                    Value::Array(a) => a
                        .iter()
                        .map(|item| {
                            serde_json::from_value::<HashMap<String, String>>(item.clone())
                                .expect("qtile sends correct message")
                        })
                        .collect(),
                    Value::Object(_) => todo!(),
                };
            match tx.send(AppEvent::UnixSocketMsg(Response {
                message_type,
                windows,
            })) {
                Ok(r) => r,
                Err(e) => log::error!("could not send message to GUI thread: {e}"),
            }
        }
        Err(e) => log::error!("{e}"),
    }
    Ok(())
}
