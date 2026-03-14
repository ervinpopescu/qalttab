use clap::Parser;
use qalttab::ui::AppEvent;
use tokio::sync::mpsc::unbounded_channel;

use qalttab::args::Args;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run(true).await
}

pub async fn run(should_run_ui: bool) -> anyhow::Result<()> {
    simple_logger::SimpleLogger::new()
        .with_module_level("wgpu_hal", log::LevelFilter::Warn)
        .with_module_level("egui_wgpu", log::LevelFilter::Warn)
        .with_level(log::LevelFilter::Info)
        .with_colors(true)
        .env()
        .init()
        .ok();

    // In actual run we parse args, in tests we might skip or use dummy
    let _args = Args::try_parse_from(["qalttab"]).unwrap_or_default();

    let (tx, rx) = unbounded_channel::<AppEvent>();

    // Spawn the UNIX socket listener
    let tx_socket = tx.clone();
    tokio::spawn(async move {
        if let Err(e) = qalttab::ipc::listen(tx_socket, None).await {
            eprintln!("Unix socket listener error: {e:?}");
        }
    });

    // Spawn the Alt key release listener
    let tx_alt = tx.clone();
    tokio::spawn(async move {
        if let Err(e) = qalttab::qaltd::listen_for_alt_release(tx_alt).await {
            eprintln!("qaltd listener error: {e:?}");
        }
    });

    if should_run_ui {
        qalttab::ui::run_ui(rx)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_init_no_ui() {
        let res = run(false).await;
        assert!(res.is_ok());
    }
}
