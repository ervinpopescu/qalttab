use qalttab::ipc::parse_message;
use qalttab::ui::{AppEvent, MessageType};
use serde_json::json;

#[test]
fn test_e2e_focus_cycle_flow() {
    // Simulate a full cycle: Message received -> Parsed -> Event generated
    let payload = json!({
        "message_type": "cycle_windows",
        "windows": [
            { "id": "1", "name": "win1", "class": "test", "group_name": "g1", "group_label": "L1" },
            { "id": "2", "name": "win2", "class": "test", "group_name": "g1", "group_label": "L1" }
        ]
    });
    let slice = serde_json::to_vec(&payload).unwrap();
    let response = parse_message(&slice).unwrap();

    // Simulate UI event handling
    let event = AppEvent::UnixSocketMsg(response);

    // Verify state
    if let AppEvent::UnixSocketMsg(r) = event {
        assert_eq!(r.message_type, MessageType::CycleWindows);
        assert_eq!(r.windows.len(), 2);
        assert_eq!(r.windows[0].get("id").unwrap(), "1");
    } else {
        panic!("Expected UnixSocketMsg");
    }
}

#[test]
fn test_e2e_client_focus_flow() {
    let payload = json!({
        "message_type": "client_focus",
        "windows": [
            { "id": "1", "name": "win1", "class": "test", "group_name": "g1", "group_label": "L1" }
        ]
    });
    let slice = serde_json::to_vec(&payload).unwrap();
    let response = parse_message(&slice).unwrap();
    let event = AppEvent::UnixSocketMsg(response);

    if let AppEvent::UnixSocketMsg(r) = event {
        assert_eq!(r.message_type, MessageType::ClientFocus);
        assert_eq!(r.windows.len(), 1);
    } else {
        panic!("Expected UnixSocketMsg");
    }
}
