use std::{
    collections::HashMap,
    io::{Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::Path,
    sync::mpsc::Sender,
};

use anyhow::bail;
use serde_json::Value;

use crate::ui::{MessageType, Response};

pub fn listen(tx: Sender<Response>) -> anyhow::Result<()> {
    let socket_path: &Path =
        &Path::new(&std::env::var("XDG_CACHE_HOME").unwrap_or("~/cache".to_owned()))
            .join("qtile")
            .join("qalttab.wayland-0");

    if socket_path.exists() {
        std::fs::remove_file(socket_path)?;
    }
    let listener = UnixListener::bind(socket_path)?;
    log::info!("Server listening on {:?}", socket_path);

    // Accept incoming connections in a loop
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                handle_conn(stream, &tx)?;
            }
            Err(e) => log::error!("Error accepting connection: {}", e),
        }
    }
    std::fs::remove_file(socket_path).unwrap_or_else(|_| {
        log::error!("Could not remove the socket file");
    });
    Ok(())
}

pub fn handle_conn(mut stream: UnixStream, tx: &Sender<Response>) -> anyhow::Result<()> {
    let mut buffer = [0; 1024];
    // Read data from the client
    let bytes_read = stream.read(&mut buffer)?;
    // Write data back to the client
    let slice = &buffer[..bytes_read];
    stream.write_all(slice)?;
    let response: Result<HashMap<String, Value>, serde_json::Error> = serde_json::from_slice(slice);
    match response {
        Ok(response) => {
            // log::debug!("{response:#?}");
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
            match tx.send(Response {
                message_type,
                windows,
            }) {
                Ok(r) => r,
                Err(e) => log::error!("could not send message to GUI thread: {}", e),
            }
            // when a message is sent create window
            // detect when alt is released and close window
        }
        Err(e) => log::error!("{}", e),
    }
    Ok(())
}
