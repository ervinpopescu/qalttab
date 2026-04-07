use qalttab::ipc::parse_ipc_message;
use qalttab::ui::MessageType;
use serde_json::json;

#[test]
fn parses_client_focus_with_windows() {
    for (id, m_type) in [("0", "client_focus"), ("1", "cycle_windows")] {
        let payload = json!({
            "message_type": m_type,
            "windows": [{ "id": id, "name": format!("win{id}") }]
        });
        let (mt, wins, _fi) = parse_ipc_message(&serde_json::to_vec(&payload).unwrap()).unwrap();
        assert_eq!(wins.len(), 1);
        assert_eq!(wins[0].get("id").unwrap(), id);
        let expected = if m_type == "client_focus" {
            MessageType::ClientFocus
        } else {
            MessageType::CycleWindows
        };
        assert_eq!(mt, expected);
    }
}

#[test]
fn parses_empty_windows() {
    let payload = json!({ "message_type": "client_focus", "windows": [] });
    let (mt, wins, _fi) = parse_ipc_message(&serde_json::to_vec(&payload).unwrap()).unwrap();
    assert_eq!(mt, MessageType::ClientFocus);
    assert!(wins.is_empty());
}

#[test]
fn error_on_missing_message_type() {
    let p = json!({ "windows": [] });
    assert!(parse_ipc_message(&serde_json::to_vec(&p).unwrap()).is_err());
}

#[test]
fn error_on_non_string_message_type() {
    let p = json!({ "message_type": 123, "windows": [] });
    assert!(parse_ipc_message(&serde_json::to_vec(&p).unwrap()).is_err());
}

#[test]
fn error_on_windows_not_array() {
    let p = json!({ "message_type": "client_focus", "windows": "not array" });
    assert!(parse_ipc_message(&serde_json::to_vec(&p).unwrap()).is_err());
}
