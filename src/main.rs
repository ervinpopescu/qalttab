use clap::Parser;
use qalttab::args::Args;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    simple_logger::SimpleLogger::new()
        .with_module_level("wgpu_hal", log::LevelFilter::Warn)
        .with_module_level("egui_wgpu", log::LevelFilter::Warn)
        .with_level(log::LevelFilter::Info)
        .with_colors(true)
        .env()
        .init()?;
    let _args: Args = Args::parse();

    // Unset DISPLAY to prevent arboard (used by eframe) from hanging on Xwayland connections
    unsafe {
        std::env::remove_var("DISPLAY");
    }

    qalttab::ui::run_ui()
}
