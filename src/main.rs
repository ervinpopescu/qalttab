use clap::Parser;
use qalttab::ui::AppEvent;
use tokio::sync::mpsc::unbounded_channel;

use qalttab::args::Args;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    simple_logger::SimpleLogger::new()
        .with_module_level("wgpu_hal", log::LevelFilter::Warn)
        .with_module_level("egui_wgpu", log::LevelFilter::Warn)
        .with_level(log::LevelFilter::Info)
        .with_colors(true)
        .env()
        .init()
        .unwrap();
    let _args: Args = Args::parse();
    let (tx, rx) = unbounded_channel::<AppEvent>();
    // Spawn the UNIX socket listener
    let tx_socket = tx.clone();
    tokio::spawn(async move {
        if let Err(e) = qalttab::ipc::listen(tx_socket).await {
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
    qalttab::ui::run_ui(rx)
}
