use crate::ui::AppEvent;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;

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
        if (line.contains("KEY_LEFTALT") || line.contains("KEY_RIGHTALT"))
            && line.contains("released")
        {
            log::debug!("Alt released");
            tx.send(AppEvent::AltReleased).ok();
            ctx.request_repaint();
        }
    }

    Ok(())
}
