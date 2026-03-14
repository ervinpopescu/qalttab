use crate::ui::AppEvent;
use qtile_client_lib::utils::client::{CommandQuery, QtileClient};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;

pub fn is_alt_release(line: &str) -> bool {
    (line.contains("KEY_LEFTALT") || line.contains("KEY_RIGHTALT")) && line.contains("released")
}

pub async fn listen_for_alt_release(tx: UnboundedSender<AppEvent>) -> anyhow::Result<()> {
    let child = Command::new("libinput")
        .args(["debug-events", "--show-keycodes"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to start libinput");

    let stdout = child.stdout.expect("Failed to capture stdout");
    let reader = BufReader::new(stdout);
    process_libinput_events(reader, tx).await
}

pub async fn process_libinput_events<R: AsyncBufReadExt + Unpin>(
    reader: R,
    tx: UnboundedSender<AppEvent>,
) -> anyhow::Result<()> {
    let mut lines = reader.lines();

    while let Some(line) = lines.next_line().await? {
        if is_alt_release(&line) {
            log::debug!("Alt released");
            tx.send(AppEvent::AltReleased).ok();

            // Still notify Qtile
            let _ = QtileClient::new(false).call(
                CommandQuery::new()
                    .function("fire_user_hook".into())
                    .args(vec!["alt_release".into()]),
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    #[test]
    fn test_is_alt_release() {
        assert!(is_alt_release(
            "event5   KEYBOARD_KEY     +2.34s	*** (KEY_LEFTALT) released"
        ));
        assert!(is_alt_release(
            "event5   KEYBOARD_KEY     +2.34s	*** (KEY_RIGHTALT) released"
        ));
        assert!(!is_alt_release(
            "event5   KEYBOARD_KEY     +2.34s	*** (KEY_LEFTALT) pressed"
        ));
        assert!(!is_alt_release(
            "event5   KEYBOARD_KEY     +2.34s	*** (KEY_SPACE) released"
        ));
        assert!(!is_alt_release("some random line"));
    }

    #[tokio::test]
    async fn test_process_libinput_events() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let input = "KEY_LEFTALT released\nSOME_OTHER_KEY released\nKEY_RIGHTALT released\nKEY_LEFTALT pressed\n";
        let reader = BufReader::new(input.as_bytes());

        process_libinput_events(reader, tx).await.unwrap();

        // Should get two AltReleased events
        assert_eq!(rx.recv().await.unwrap(), AppEvent::AltReleased);
        assert_eq!(rx.recv().await.unwrap(), AppEvent::AltReleased);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_is_alt_release_edge_cases() {
        assert!(!is_alt_release(""));
        assert!(!is_alt_release("KEY_LEFTALT"));
        assert!(!is_alt_release("released"));
    }
}
