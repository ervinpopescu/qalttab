use qalttab::ipc::parse_message;
use qalttab::ui::MessageType;
use serde_json::json;

#[test]
fn test_ipc_parsing_variants() {
    // Test a few representative cases instead of 100 identical ones
    let cases = vec![("0", "client_focus"), ("1", "cycle_windows")];

    for (id, m_type) in cases {
        let payload = json!({
            "message_type": m_type,
            "windows": [
                { "id": id, "name": format!("win{}", id) }
            ]
        });
        let slice = serde_json::to_vec(&payload).unwrap();
        let response = parse_message(&slice).unwrap();
        assert_eq!(response.windows.len(), 1);
        assert_eq!(response.windows[0].get("id").unwrap(), &id.to_string());
    }
}

#[test]
fn test_ipc_empty_payload() {
    let payload = json!({
        "message_type": "client_focus",
        "windows": []
    });
    let slice = serde_json::to_vec(&payload).unwrap();
    let response = parse_message(&slice).unwrap();
    assert_eq!(response.message_type, MessageType::ClientFocus);
    assert!(response.windows.is_empty());
}

#[test]
fn test_ipc_error_paths() {
    // Missing message_type
    let p1 = json!({ "windows": [] });
    assert!(parse_message(&serde_json::to_vec(&p1).unwrap()).is_err());

    // Invalid message_type type
    let p2 = json!({ "message_type": 123, "windows": [] });
    assert!(parse_message(&serde_json::to_vec(&p2).unwrap()).is_err());

    // Windows not an array
    let p3 = json!({ "message_type": "client_focus", "windows": "not array" });
    assert!(parse_message(&serde_json::to_vec(&p3).unwrap()).is_err());
}
