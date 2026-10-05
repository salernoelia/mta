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
    if args.len() >= 3 {
        let flag = &args[1];
        let path = PathBuf::from(&args[2]);

        match flag.as_str() {
            "--csv" => {
                match metadata::inspect_file(&path, true) {
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
                }
            }
            "--json" => {
                match metadata::inspect_file(&path, true) {
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
                }
            }
            "--md" | "--markdown" => {
                match metadata::inspect_file(&path, true) {
                    Ok(rep) => {
                        println!("{}", rep.to_markdown());
                        return;
                    }
                    Err(e) => {
                        eprintln!("Error inspecting file: {e}");
                        std::process::exit(1);
                    }
                }
            }
            _ => {}
        }
    }

    let initial_file = if args.len() >= 2 && !args[1].starts_with('-') {
        let p = PathBuf::from(&args[1]);
        if p.exists() {
            Some(p)
        } else {
            None
        }
    } else {
        None
    };

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
