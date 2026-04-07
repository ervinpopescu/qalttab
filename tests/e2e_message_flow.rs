use qalttab::ipc::parse_ipc_message;
use qalttab::ui::{AppEvent, MessageType};
use serde_json::json;

#[test]
fn cycle_windows_event_carries_correct_data() {
    let payload = json!({
        "message_type": "cycle_windows",
        "windows": [
            { "id": "1", "name": "win1", "class": "test", "group_name": "g1", "group_label": "L1" },
            { "id": "2", "name": "win2", "class": "test", "group_name": "g1", "group_label": "L1" }
        ]
    });
    let (mt, wins, fi) = parse_ipc_message(&serde_json::to_vec(&payload).unwrap()).unwrap();
    let event = AppEvent::UnixSocketMsg(qalttab::ui::Response {
        message_type: mt,
        windows: wins,
        focus_index: fi,
    });

    if let AppEvent::UnixSocketMsg(r) = event {
        assert_eq!(r.message_type, MessageType::CycleWindows);
        assert_eq!(r.windows.len(), 2);
        assert_eq!(r.windows[0].get("id").unwrap(), "1");
    } else {
        panic!("expected UnixSocketMsg");
    }
}

#[test]
fn client_focus_event_carries_correct_data() {
    let payload = json!({
        "message_type": "client_focus",
        "windows": [
            { "id": "1", "name": "win1", "class": "test", "group_name": "g1", "group_label": "L1" }
        ]
    });
    let (mt, wins, fi) = parse_ipc_message(&serde_json::to_vec(&payload).unwrap()).unwrap();
    let event = AppEvent::UnixSocketMsg(qalttab::ui::Response {
        message_type: mt,
        windows: wins,
        focus_index: fi,
    });

    if let AppEvent::UnixSocketMsg(r) = event {
        assert_eq!(r.message_type, MessageType::ClientFocus);
        assert_eq!(r.windows.len(), 1);
    } else {
        panic!("expected UnixSocketMsg");
    }
}

#[test]
fn focus_index_is_propagated() {
    let payload = json!({
        "message_type": "cycle_windows",
        "windows": [
            { "id": "1", "name": "w1" },
            { "id": "2", "name": "w2" },
        ],
        "focus_index": 1
    });
    let (mt, wins, fi) = parse_ipc_message(&serde_json::to_vec(&payload).unwrap()).unwrap();
    assert_eq!(mt, MessageType::CycleWindows);
    assert_eq!(wins.len(), 2);
    assert_eq!(fi, Some(1));
}
