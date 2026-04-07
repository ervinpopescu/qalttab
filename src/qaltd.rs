use crate::ui::AppEvent;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;

pub fn is_alt_release_event(line: &str) -> bool {
    (line.contains("KEY_LEFTALT") || line.contains("KEY_RIGHTALT")) && line.contains("released")
}

pub async fn listen_for_alt_release(
    tx: UnboundedSender<AppEvent>,
    ctx: egui::Context,
) -> anyhow::Result<()> {
    let mut child = Command::new("libinput")
        .args(["debug-events", "--show-keycodes"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to start libinput");

    let stdout = child.stdout.take().expect("Failed to capture stdout");
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();

    while let Some(line) = lines.next_line().await? {
        if is_alt_release_event(&line) {
            log::debug!("Alt released");
            tx.send(AppEvent::AltReleased).ok();
            ctx.request_repaint();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_alt_released_is_detected() {
        assert!(is_alt_release_event(
            "event5  KEYBOARD_KEY  +0.001s  KEY_LEFTALT (56) released"
        ));
    }

    #[test]
    fn right_alt_released_is_detected() {
        assert!(is_alt_release_event(
            "event5  KEYBOARD_KEY  +0.001s  KEY_RIGHTALT (100) released"
        ));
    }

    #[test]
    fn left_alt_pressed_is_not_an_alt_release() {
        assert!(!is_alt_release_event(
            "event5  KEYBOARD_KEY  +0.001s  KEY_LEFTALT (56) pressed"
        ));
    }

    #[test]
    fn unrelated_key_released_is_not_an_alt_release() {
        assert!(!is_alt_release_event(
            "event5  KEYBOARD_KEY  +0.001s  KEY_SPACE (57) released"
        ));
    }

    #[test]
    fn empty_line_is_not_an_alt_release() {
        assert!(!is_alt_release_event(""));
    }
}
