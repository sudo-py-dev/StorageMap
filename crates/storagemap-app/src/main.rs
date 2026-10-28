//! StorageMap: a fast, multilingual disk space analyzer.

// Release builds are GUI-subsystem on Windows (no console window); the CLI
// re-attaches to the parent console instead.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod cli;
mod devshot;
mod filetype;
mod fmt;
mod i18n;
mod platform;
mod scan_job;
mod settings;
mod theme;
mod update;
mod views;
mod widgets;

use std::process::ExitCode;

const MIN_WINDOW: [f32; 2] = [760.0, 480.0];
const DEFAULT_WINDOW: [f32; 2] = [1280.0, 820.0];

fn window_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/icon/storagemap-256.png"))
        .unwrap_or_default()
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("STORAGEMAP_LOG")
                .unwrap_or_else(|_| "warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let args = match cli::Args::parse(std::env::args_os().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            platform::attach_console();
            eprintln!("error: {e}\n\n{}", cli::USAGE);
            return ExitCode::from(2);
        }
    };
    if args.help || args.version || args.cli {
        platform::attach_console();
        if args.help {
            println!("{}", cli::USAGE);
            return ExitCode::SUCCESS;
        }
        if args.version {
            println!("storagemap {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        return cli::run(&args);
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("StorageMap")
            .with_app_id("storagemap")
            .with_inner_size(DEFAULT_WINDOW)
            .with_min_inner_size(MIN_WINDOW)
            .with_drag_and_drop(true)
            .with_icon(window_icon()),
        persist_window: true,
        ..Default::default()
    };
    let initial = args.path;
    match eframe::run_native(
        "StorageMap",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, initial)))),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(%e, "cannot start the window");
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
