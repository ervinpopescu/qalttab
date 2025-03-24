use anyhow::bail;
use clap::Parser;
use qalttab::{ipc::listen, ui::AsyncApp};
use std::sync::mpsc::channel;
use sysinfo::{Pid, System};
use tokio::runtime::Runtime;
/// Qtile alttab window
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    let rt = Runtime::new().expect("Unable to create Runtime");

    let _enter = rt.enter();

    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .with_colors(true)
        .env()
        .init()
        .unwrap();
    let _args: Args = Args::parse();
    let (tx, rx) = channel();
    std::thread::spawn(move || rt.block_on(async { listen(tx.clone()) }));
    // Run the GUI in the main thread.

    let s = System::new_all();
    let qalttab_processes_parents = s
        .processes_by_exact_name("qalttab".as_ref())
        .map(|p| p.parent());
    let mut qalttab_processes_vec = qalttab_processes_parents.collect::<Vec<Option<Pid>>>();
    qalttab_processes_vec.sort();
    qalttab_processes_vec.dedup();
    if qalttab_processes_vec.len() >= 4 {
        bail!("qalttab already running");
    };
    match eframe::run_native(
        "qalttab",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder {
                title: Some("qalttab".to_owned()),
                app_id: Some("qalttab".to_owned()),
                // resizable: Some(false),
                // transparent: Some(true),
                decorations: Some(false),
                visible: Some(false),
                taskbar: Some(false),
                title_shown: Some(false),
                window_level: Some(egui::WindowLevel::AlwaysOnTop),
                ..egui::ViewportBuilder::default()
            },
            // event_loop_builder: Some(Box::new(|elb| {})),
            // window_builder: Some(Box::new(|vb| {})),
            ..eframe::NativeOptions::default()
        },
        Box::new(|cc| Ok(Box::<AsyncApp>::new(AsyncApp::new(cc, Some(rx))))),
    ) {
        Ok(()) => Ok(()),
        Err(e) => bail!("eframe crashed: {}", e),
    }
}
