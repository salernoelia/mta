mod app;
mod config;
mod exporter;
mod history;
mod icon;
mod metadata;
mod updater;

use app::MtaApp;
use eframe::egui;
use icon::load_app_icon;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // CLI Quick Output Modes
    let mut format = None;
    let mut target_path: Option<PathBuf> = None;

    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("Mta - Pure Rust universal metadata inspector");
                println!();
                println!("USAGE:");
                println!("    mta [OPTIONS] [FILE]");
                println!();
                println!("OPTIONS:");
                println!("    --json             Output metadata as formatted JSON");
                println!("    --csv              Output metadata as CSV table");
                println!("    --md, --markdown   Output metadata as Markdown");
                println!("    -h, --help         Print help information");
                println!("    -V, --version      Print version information");
                println!();
                println!("If no export option is specified, Mta opens in GUI mode.");
                return;
            }
            "-V" | "--version" => {
                println!("Mta v{}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--json" => format = Some("json"),
            "--csv" => format = Some("csv"),
            "--md" | "--markdown" => format = Some("md"),
            other => {
                if !other.starts_with('-') && target_path.is_none() {
                    target_path = Some(PathBuf::from(other));
                }
            }
        }
    }

    if let (Some(fmt), Some(path)) = (format, &target_path) {
        match fmt {
            "csv" => match metadata::inspect_file(path, true) {
                Ok(rep) => match rep.to_csv() {
                    Ok(csv) => {
                        print!("{csv}");
                        return;
                    }
                    Err(e) => {
                        eprintln!("Error generating CSV: {e}");
                        std::process::exit(1);
                    }
                },
                Err(e) => {
                    eprintln!("Error inspecting file: {e}");
                    std::process::exit(1);
                }
            },
            "json" => match metadata::inspect_file(path, true) {
                Ok(rep) => match rep.to_json() {
                    Ok(json) => {
                        println!("{json}");
                        return;
                    }
                    Err(e) => {
                        eprintln!("Error generating JSON: {e}");
                        std::process::exit(1);
                    }
                },
                Err(e) => {
                    eprintln!("Error inspecting file: {e}");
                    std::process::exit(1);
                }
            },
            "md" => match metadata::inspect_file(path, true) {
                Ok(rep) => {
                    println!("{}", rep.to_markdown());
                    return;
                }
                Err(e) => {
                    eprintln!("Error inspecting file: {e}");
                    std::process::exit(1);
                }
            },
            _ => {}
        }
    }

    let initial_file = target_path.filter(|p| p.exists());

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Mta")
        .with_inner_size([640.0, 480.0])
        .with_min_inner_size([440.0, 300.0]);

    if let Some(icon) = load_app_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let run_result = eframe::run_native(
        "Mta",
        options,
        Box::new(move |_cc| Ok(Box::new(MtaApp::new(initial_file)))),
    );

    if let Err(err) = run_result {
        eprintln!("Failed to start Mta GUI: {err}");
    }
}
