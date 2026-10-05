use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use crate::config::{AppConfig, AppTheme};
use crate::exporter;
use crate::history::InspectionHistory;
use crate::metadata::{inspect_file, FileMetadataReport};
use crate::updater::{spawn_self_update, spawn_update_check, UpdateEvent, UpdateStatus};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AppTab {
    Inspector,
    Batch,
    History,
    Settings,
}

pub struct MtaApp {
    config: AppConfig,
    active_tab: AppTab,
    history: InspectionHistory,
    history_search: String,

    // Single file inspector state
    current_report: Option<FileMetadataReport>,
    filter_query: String,
    selected_category: Option<String>,

    // Batch inspector state (async processing)
    batch_reports: Vec<FileMetadataReport>,
    is_batch_loading: bool,
    batch_total_expected: usize,
    batch_processed_count: usize,
    batch_rx: Receiver<FileMetadataReport>,
    batch_tx: Sender<FileMetadataReport>,

    // Status bar & notification
    status_line: String,

    // Updates
    update_status: UpdateStatus,
    update_event_rx: Receiver<UpdateEvent>,
    update_event_tx: Sender<UpdateEvent>,

    // Theme tracking
    applied_theme: Option<AppTheme>,
}

impl MtaApp {
    pub fn new(initial_file: Option<PathBuf>) -> Self {
        let config = AppConfig::load();
        let history = InspectionHistory::load();
        let (update_event_tx, update_event_rx) = mpsc::channel();
        let (batch_tx, batch_rx) = mpsc::channel();

        // Spawn background update check
        spawn_update_check(update_event_tx.clone());

        let mut app = Self {
            config,
            active_tab: AppTab::Inspector,
            history,
            history_search: String::new(),
            current_report: None,
            filter_query: String::new(),
            selected_category: None,
            batch_reports: Vec::new(),
            is_batch_loading: false,
            batch_total_expected: 0,
            batch_processed_count: 0,
            batch_rx,
            batch_tx,
            status_line: "Ready".to_string(),
            update_status: UpdateStatus::Checking,
            update_event_rx,
            update_event_tx,
            applied_theme: None,
        };

        if let Some(path) = initial_file {
            app.load_file(&path);
        }

        app
    }

    pub fn load_file(&mut self, path: &Path) {
        match inspect_file(path, self.config.calculate_hashes) {
            Ok(report) => {
                let count = report.total_entries_count();
                self.history.add(&report, self.config.max_recent_files);
                self.status_line = format!(
                    "Inspected {} ({} properties extracted)",
                    report.file_name, count
                );
                if self.config.auto_copy_csv_on_inspect {
                    if let Ok(csv) = report.to_csv() {
                        let _ = exporter::copy_to_clipboard(&csv);
                        self.status_line.push_str(" • CSV copied to clipboard");
                    }
                }
                self.current_report = Some(report);
                self.active_tab = AppTab::Inspector;
            }
            Err(err) => {
                self.status_line = format!("Error reading file: {err}");
            }
        }
    }

    pub fn load_batch_files(&mut self, paths: &[PathBuf]) {
        let valid_paths: Vec<PathBuf> = paths.iter().filter(|p| p.is_file()).cloned().collect();
        if valid_paths.is_empty() {
            return;
        }

        self.is_batch_loading = true;
        self.batch_total_expected += valid_paths.len();
        self.status_line = format!(
            "Inspecting batch: {}/{} files...",
            self.batch_processed_count, self.batch_total_expected
        );
        self.active_tab = AppTab::Batch;

        let calculate_hashes = self.config.calculate_hashes;
        let tx = self.batch_tx.clone();
        std::thread::spawn(move || {
            for p in valid_paths {
                if let Ok(rep) = inspect_file(&p, calculate_hashes) {
                    let _ = tx.send(rep);
                }
            }
        });
    }

    fn apply_batch_events(&mut self) {
        let mut received = 0;
        while let Ok(rep) = self.batch_rx.try_recv() {
            self.batch_reports.push(rep);
            self.batch_processed_count += 1;
            received += 1;
        }

        if received > 0 {
            if self.batch_processed_count >= self.batch_total_expected {
                self.is_batch_loading = false;
                self.status_line = format!(
                    "Batch inspection complete ({} files loaded)",
                    self.batch_reports.len()
                );
            } else {
                self.status_line = format!(
                    "Inspecting batch: {}/{} files...",
                    self.batch_processed_count, self.batch_total_expected
                );
            }
        }
    }

    fn check_for_updates(&mut self) {
        self.update_status = UpdateStatus::Checking;
        spawn_update_check(self.update_event_tx.clone());
    }

    fn perform_update(&mut self, download_url: String) {
        self.update_status = UpdateStatus::Downloading;
        self.status_line = "Downloading and applying update...".to_string();
        spawn_self_update(download_url, self.update_event_tx.clone());
    }

    fn handle_drag_and_drop(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped.is_empty() {
            let valid_paths: Vec<PathBuf> = dropped
                .into_iter()
                .filter_map(|f| f.path)
                .filter(|p| p.is_file())
                .collect();

            if valid_paths.len() == 1 {
                self.load_file(&valid_paths[0]);
            } else if valid_paths.len() > 1 {
                self.load_batch_files(&valid_paths);
            }
        }
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        // Cmd/Ctrl + O: Open file
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            if let Some(path) = rfd::FileDialog::new().pick_file() {
                self.load_file(&path);
            }
        }
        // Cmd/Ctrl + Shift + O: Open batch files
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT), egui::Key::O)) {
            if let Some(files) = rfd::FileDialog::new().pick_files() {
                self.load_batch_files(&files);
            }
        }
        // Cmd/Ctrl + W: Close current report
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::W)) {
            if self.current_report.is_some() {
                self.current_report = None;
                self.status_line = "Closed report".to_string();
            }
        }
        // Cmd/Ctrl + 1-4: Switch tabs
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Num1)) {
            self.active_tab = AppTab::Inspector;
        } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Num2)) {
            self.active_tab = AppTab::Batch;
        } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Num3)) {
            self.active_tab = AppTab::History;
        } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Num4)) {
            self.active_tab = AppTab::Settings;
        }
        // Escape: clear filter or search
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            if !self.filter_query.is_empty() {
                self.filter_query.clear();
            }
            if !self.history_search.is_empty() {
                self.history_search.clear();
            }
        }
    }

    fn draw_drag_overlay(&self, ctx: &egui::Context) {
        let hovered = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if hovered {
            egui::Area::new(egui::Id::new("drag_drop_overlay"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(0.0, 0.0))
                .show(ctx, |ui| {
                    let screen_rect = ctx.screen_rect();
                    let painter = ui.painter();
                    painter.rect_filled(
                        screen_rect,
                        0.0,
                        egui::Color32::from_black_alpha(190),
                    );
                    let center = screen_rect.center();
                    painter.text(
                        center,
                        egui::Align2::CENTER_CENTER,
                        "Drop files to inspect metadata",
                        egui::FontId::proportional(22.0),
                        egui::Color32::WHITE,
                    );
                });
        }
    }

    fn apply_update_events(&mut self) {
        while let Ok(event) = self.update_event_rx.try_recv() {
            match event {
                UpdateEvent::UpdateAvailable {
                    version,
                    html_url,
                    download_url,
                } => {
                    self.update_status = UpdateStatus::Available {
                        version,
                        html_url,
                        download_url,
                    };
                }
                UpdateEvent::UpToDate => {
                    self.update_status = UpdateStatus::UpToDate;
                }
                UpdateEvent::CheckFailed(err) => {
                    self.update_status = UpdateStatus::Failed(err);
                }
                UpdateEvent::UpdateApplied => {
                    self.update_status = UpdateStatus::Applied;
                    self.status_line = "Update installed successfully. Restart Mta to use the new version.".to_string();
                }
                UpdateEvent::InstallFailed(err) => {
                    self.update_status = UpdateStatus::Failed(err.clone());
                    self.status_line = format!("Update failed: {err}");
                }
            }
        }
    }

    fn tabs_bar_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            ui.selectable_value(&mut self.active_tab, AppTab::Inspector, "Inspector");
            ui.selectable_value(&mut self.active_tab, AppTab::Batch, "Batch");
            ui.selectable_value(&mut self.active_tab, AppTab::History, "History");
            ui.selectable_value(&mut self.active_tab, AppTab::Settings, "Settings");

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                match &self.update_status {
                    UpdateStatus::Available { version, download_url, .. } => {
                        let btn = egui::Button::new(format!("Update to v{version}"))
                            .fill(egui::Color32::from_rgb(30, 80, 255));
                        if ui.add(btn).clicked() {
                            let url = download_url.clone();
                            self.perform_update(url);
                        }
                    }
                    UpdateStatus::Downloading => {
                        ui.label(egui::RichText::new("Updating...").italics());
                    }
                    UpdateStatus::Applied => {
                        ui.label(egui::RichText::new("✓ Updated").color(egui::Color32::GREEN));
                    }
                    _ => {
                        ui.label(
                            egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                                .weak()
                                .small(),
                        );
                    }
                }
            });
        });
        ui.separator();
    }

    fn inspector_ui(&mut self, ui: &mut egui::Ui) {
        if let Some(ref report) = self.current_report.clone() {
            // Header Card
            ui.group(|ui| {
                ui.set_width(ui.available_width());

                // Row 1: File name & badges
                ui.horizontal_wrapped(|ui| {
                    ui.heading(
                        egui::RichText::new(&report.file_name)
                            .strong(),
                    );
                    let badge = egui::RichText::new(format!(" {} ", report.file_type_label))
                        .background_color(egui::Color32::from_rgb(30, 80, 220))
                        .color(egui::Color32::WHITE)
                        .strong();
                    ui.label(badge);
                    ui.label(egui::RichText::new(&report.file_size_formatted).weak());
                });

                // Row 2: Full Path
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(report.path.display().to_string())
                        .small()
                        .weak(),
                );

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);

                // Row 3: Action Buttons Toolbar (dedicated row, no separators, clean text)
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    if ui.button("Export CSV").on_hover_text("Save metadata as CSV spreadsheet").clicked() {
                        match exporter::save_single_csv(report, &self.config.export_dir) {
                            Ok(Some(saved)) => {
                                self.status_line = format!("Exported CSV to {saved}");
                            }
                            Ok(None) => {}
                            Err(err) => {
                                self.status_line = format!("Export error: {err}");
                            }
                        }
                    }

                    if ui.button("Copy CSV").on_hover_text("Copy CSV table to clipboard").clicked() {
                        if let Ok(csv) = report.to_csv() {
                            if exporter::copy_to_clipboard(&csv).is_ok() {
                                self.status_line = "Copied CSV to clipboard".to_string();
                            }
                        }
                    }

                    if ui.button("Export JSON").on_hover_text("Save metadata as JSON document").clicked() {
                        match exporter::save_single_json(report, &self.config.export_dir) {
                            Ok(Some(saved)) => {
                                self.status_line = format!("Exported JSON to {saved}");
                            }
                            Ok(None) => {}
                            Err(err) => {
                                self.status_line = format!("Export error: {err}");
                            }
                        }
                    }

                    if ui.button("Copy Summary").on_hover_text("Copy Markdown summary to clipboard").clicked() {
                        let md = report.to_markdown();
                        if exporter::copy_to_clipboard(&md).is_ok() {
                            self.status_line = "Copied summary to clipboard".to_string();
                        }
                    }

                    #[cfg(target_os = "macos")]
                    let reveal_btn_text = "Reveal in Finder";
                    #[cfg(target_os = "windows")]
                    let reveal_btn_text = "Show in Explorer";
                    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                    let reveal_btn_text = "Show in File Manager";

                    if ui.button(reveal_btn_text).on_hover_text("Reveal and select file in system file manager").clicked() {
                        let _ = exporter::reveal_file_in_finder(&report.path);
                    }

                    if ui.button("Open File").on_hover_text("Open file with standard application").clicked() {
                        let _ = exporter::open_file_in_default_app(&report.path);
                    }

                    if ui.button("Open Another...").on_hover_text("Select another file to inspect (Cmd+O)").clicked() {
                        if let Some(p) = rfd::FileDialog::new().pick_file() {
                            self.load_file(&p);
                        }
                    }

                    if ui.button("Close").on_hover_text("Close current report (Cmd+W)").clicked() {
                        self.current_report = None;
                        self.status_line = "Closed report".to_string();
                    }
                });
            });

            ui.add_space(6.0);

            // Filter & Search bar
            ui.horizontal_wrapped(|ui| {
                ui.label("Filter:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.filter_query)
                        .hint_text("Search properties...")
                        .desired_width(160.0),
                );
                if !self.filter_query.is_empty() && ui.small_button("✕").on_hover_text("Clear filter (Esc)").clicked() {
                    self.filter_query.clear();
                }

                // Category chips
                ui.separator();
                let is_all = self.selected_category.is_none();
                if ui.selectable_label(is_all, format!("All ({})", report.total_entries_count())).clicked() {
                    self.selected_category = None;
                }

                for sec in &report.sections {
                    let is_sel = self.selected_category.as_deref() == Some(&sec.name);
                    let label = format!("{} ({})", sec.name, sec.entries.len());
                    if ui.selectable_label(is_sel, label).clicked() {
                        self.selected_category = if is_sel { None } else { Some(sec.name.clone()) };
                    }
                }
            });

            ui.add_space(6.0);

            // Metadata Table
            let filter = self.filter_query.to_lowercase();
            egui::ScrollArea::vertical()
                .id_salt("metadata_table_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let full_width = ui.available_width();
                    let mut total_matches = 0;

                    for section in &report.sections {
                        if let Some(ref cat) = self.selected_category {
                            if &section.name != cat {
                                continue;
                            }
                        }

                        let matching_entries: Vec<_> = section
                            .entries
                            .iter()
                            .filter(|e| {
                                filter.is_empty()
                                    || e.key.to_lowercase().contains(&filter)
                                    || e.value.to_lowercase().contains(&filter)
                                    || e.category.to_lowercase().contains(&filter)
                            })
                            .collect();

                        if matching_entries.is_empty() {
                            continue;
                        }

                        total_matches += matching_entries.len();

                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&section.name).strong().heading());
                            ui.label(
                                egui::RichText::new(format!("({} properties)", matching_entries.len()))
                                    .small()
                                    .weak(),
                            );
                        });
                        ui.separator();

                        let key_col_w = (full_width * 0.28).clamp(130.0, 220.0);
                        let action_col_w = 32.0;
                        let val_col_w = (full_width - key_col_w - action_col_w - 32.0).max(100.0);

                        for (idx, entry) in matching_entries.iter().enumerate() {
                            let row_bg = if idx % 2 == 0 {
                                ui.visuals().faint_bg_color
                            } else {
                                egui::Color32::TRANSPARENT
                            };

                            egui::Frame::NONE
                                .fill(row_bg)
                                .inner_margin(egui::Margin::symmetric(6, 4))
                                .corner_radius(4.0)
                                .show(ui, |ui| {
                                    ui.set_width(full_width - 12.0);
                                    ui.horizontal(|ui| {
                                        // Column 1: Key
                                        ui.allocate_ui_with_layout(
                                             egui::vec2(key_col_w, 0.0),
                                            egui::Layout::left_to_right(egui::Align::Center),
                                            |ui| {
                                                ui.add(
                                                    egui::Label::new(
                                                        egui::RichText::new(&entry.key).strong(),
                                                    )
                                                    .wrap(),
                                                );
                                            },
                                        );

                                        // Column 2: Value (selectable text)
                                        ui.allocate_ui_with_layout(
                                            egui::vec2(val_col_w, 0.0),
                                            egui::Layout::left_to_right(egui::Align::Center),
                                            |ui| {
                                                let is_link = entry.value.starts_with("http://")
                                                    || entry.value.starts_with("https://");

                                                if is_link {
                                                    ui.hyperlink(&entry.value);
                                                } else {
                                                    ui.add(egui::Label::new(&entry.value).wrap().selectable(true));
                                                }
                                            },
                                        );

                                        // Column 3: Copy Action Button
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .small_button("📋")
                                                    .on_hover_text("Copy value to clipboard")
                                                    .clicked()
                                                {
                                                    if exporter::copy_to_clipboard(&entry.value).is_ok() {
                                                        self.status_line = format!(
                                                            "Copied '{}' to clipboard",
                                                            entry.key
                                                        );
                                                    }
                                                }
                                            },
                                        );
                                    });
                                });
                        }
                    }

                    if total_matches == 0 && (!filter.is_empty() || self.selected_category.is_some()) {
                        ui.add_space(30.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new(format!("No properties matching \"{}\"", self.filter_query))
                                    .weak(),
                            );
                            ui.add_space(8.0);
                            if ui.button("Clear Filter").clicked() {
                                self.filter_query.clear();
                                self.selected_category = None;
                            }
                        });
                    }

                    // Ample bottom space so bottom-most item is never obscured
                    ui.add_space(36.0);
                });
        } else {
            // Empty State: Minimalist text only
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);

                ui.label(
                    egui::RichText::new("Drag & drop any file here")
                        .size(18.0)
                        .strong(),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("or browse to inspect full metadata")
                        .small()
                        .weak(),
                );
                ui.add_space(10.0);

                if ui.button(egui::RichText::new("Browse File...").size(14.0)).clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        self.load_file(&path);
                    }
                }

                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new("Supports Images, PDFs, Documents, Audio, Video, Archives, Code, Fonts & more")
                        .small()
                        .weak(),
                );

                ui.add_space(24.0);

                // Quick Recent Files
                if !self.history.records.is_empty() {
                    ui.label(egui::RichText::new("Recent Files").strong().weak());
                    ui.add_space(6.0);

                    let mut clicked_recent = None;
                    for rec in self.history.records.iter().take(5) {
                        let path = PathBuf::from(&rec.path);
                        if path.exists() {
                            let label = format!("{} ({})", rec.file_name, rec.file_type_label);
                            if ui.button(label).clicked() {
                                clicked_recent = Some(path);
                            }
                        }
                    }

                    if let Some(p) = clicked_recent {
                        self.load_file(&p);
                    }
                }
            });
        }
    }

    fn batch_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Add Files...").on_hover_text("Select multiple files to inspect (Cmd+Shift+O)").clicked() {
                if let Some(files) = rfd::FileDialog::new().pick_files() {
                    self.load_batch_files(&files);
                }
            }

            if ui.button("Add Folder...").on_hover_text("Select an entire directory to inspect").clicked() {
                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                    if let Ok(entries) = std::fs::read_dir(folder) {
                        let files: Vec<PathBuf> = entries
                            .flatten()
                            .map(|e| e.path())
                            .filter(|p| p.is_file())
                            .collect();
                        self.load_batch_files(&files);
                    }
                }
            }

            if !self.batch_reports.is_empty() {
                if ui.button("Clear All").on_hover_text("Clear current batch list").clicked() {
                    self.batch_reports.clear();
                    self.batch_total_expected = 0;
                    self.batch_processed_count = 0;
                    self.is_batch_loading = false;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Export CSV").on_hover_text("Export entire batch as CSV spreadsheet").clicked() {
                        match exporter::save_batch_csv(&self.batch_reports, &self.config.export_dir) {
                            Ok(Some(path)) => {
                                self.status_line = format!("Exported batch CSV to {path}");
                            }
                            Ok(None) => {}
                            Err(err) => {
                                self.status_line = format!("Export error: {err}");
                            }
                        }
                    }

                    if ui.button("Copy CSV").on_hover_text("Copy batch CSV table to clipboard").clicked() {
                        if let Ok(csv) = exporter::generate_batch_csv(&self.batch_reports) {
                            if exporter::copy_to_clipboard(&csv).is_ok() {
                                self.status_line = "Copied batch CSV to clipboard".to_string();
                            }
                        }
                    }
                });
            }
        });

        ui.separator();

        if self.batch_reports.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                ui.label(egui::RichText::new("No batch files loaded").heading());
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Drop multiple files or click 'Add Files' / 'Add Folder' to inspect them together.")
                        .weak(),
                );
            });
        } else {
            let total_props: usize = self.batch_reports.iter().map(|r| r.total_entries_count()).sum();
            ui.horizontal(|ui| {
                if self.is_batch_loading {
                    ui.label(
                        egui::RichText::new(format!(
                            "Analyzing {} of {} files...",
                            self.batch_processed_count, self.batch_total_expected
                        ))
                        .italics(),
                    );
                } else {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} files in batch ({} total properties extracted)",
                            self.batch_reports.len(),
                            total_props
                        ))
                        .weak(),
                    );
                }
            });
            ui.add_space(4.0);

            let mut inspect_idx = None;
            let mut remove_idx = None;

            egui::ScrollArea::vertical()
                .id_salt("batch_table_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let full_width = ui.available_width();
                    let action_w = 76.0;
                    let format_w = (full_width * 0.16).clamp(80.0, 130.0);
                    let size_w = (full_width * 0.14).clamp(70.0, 110.0);
                    let props_w = (full_width * 0.10).clamp(50.0, 80.0);
                    let name_w = (full_width - action_w - format_w - size_w - props_w - 40.0).max(100.0);

                    // Header
                    egui::Frame::NONE
                        .fill(ui.visuals().faint_bg_color)
                        .inner_margin(egui::Margin::symmetric(6, 6))
                        .corner_radius(4.0)
                        .show(ui, |ui| {
                            ui.set_width(full_width - 12.0);
                            ui.horizontal(|ui| {
                                ui.allocate_ui(egui::vec2(action_w, 0.0), |ui| {
                                    ui.label(egui::RichText::new("Action").strong());
                                });
                                ui.allocate_ui(egui::vec2(name_w, 0.0), |ui| {
                                    ui.label(egui::RichText::new("File Name").strong());
                                });
                                ui.allocate_ui(egui::vec2(format_w, 0.0), |ui| {
                                    ui.label(egui::RichText::new("Format").strong());
                                });
                                ui.allocate_ui(egui::vec2(size_w, 0.0), |ui| {
                                    ui.label(egui::RichText::new("Size").strong());
                                });
                                ui.allocate_ui(egui::vec2(props_w, 0.0), |ui| {
                                    ui.label(egui::RichText::new("Properties").strong());
                                });
                            });
                        });

                    for (idx, report) in self.batch_reports.iter().enumerate() {
                        let row_bg = if idx % 2 == 0 {
                            ui.visuals().faint_bg_color
                        } else {
                            egui::Color32::TRANSPARENT
                        };

                        egui::Frame::NONE
                            .fill(row_bg)
                            .inner_margin(egui::Margin::symmetric(6, 5))
                            .corner_radius(4.0)
                            .show(ui, |ui| {
                                ui.set_width(full_width - 12.0);
                                ui.horizontal(|ui| {
                                    ui.allocate_ui(egui::vec2(action_w, 0.0), |ui| {
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 4.0;
                                            if ui.small_button("Inspect").clicked() {
                                                inspect_idx = Some(idx);
                                            }
                                            if ui.small_button("✕").on_hover_text("Remove from batch").clicked() {
                                                remove_idx = Some(idx);
                                            }
                                        });
                                    });
                                    ui.allocate_ui(egui::vec2(name_w, 0.0), |ui| {
                                        ui.add(egui::Label::new(&report.file_name).truncate())
                                            .on_hover_text(report.path.display().to_string());
                                    });
                                    ui.allocate_ui(egui::vec2(format_w, 0.0), |ui| {
                                        ui.label(&report.file_type_label);
                                    });
                                    ui.allocate_ui(egui::vec2(size_w, 0.0), |ui| {
                                        ui.label(&report.file_size_formatted);
                                    });
                                    ui.allocate_ui(egui::vec2(props_w, 0.0), |ui| {
                                        ui.label(report.total_entries_count().to_string());
                                    });
                                });
                            });
                    }

                    ui.add_space(36.0);
                });

            if let Some(idx) = remove_idx {
                self.batch_reports.remove(idx);
            }
            if let Some(idx) = inspect_idx {
                self.current_report = Some(self.batch_reports[idx].clone());
                self.active_tab = AppTab::Inspector;
            }
        }
    }

    fn history_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Inspection History");

            ui.add_space(16.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.history_search)
                    .hint_text("Search history...")
                    .desired_width(180.0),
            );
            if !self.history_search.is_empty() && ui.small_button("✕").on_hover_text("Clear search (Esc)").clicked() {
                self.history_search.clear();
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !self.history.records.is_empty() && ui.button("Clear History").clicked() {
                    self.history.clear();
                }
            });
        });
        ui.separator();

        let search = self.history_search.to_lowercase();
        let filtered_records: Vec<(usize, &crate::history::HistoryRecord)> = self
            .history
            .records
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                search.is_empty()
                    || r.file_name.to_lowercase().contains(&search)
                    || r.file_type_label.to_lowercase().contains(&search)
                    || r.path.to_lowercase().contains(&search)
            })
            .collect();

        if self.history.records.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                ui.label(egui::RichText::new("No inspection history yet").heading());
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Files you inspect will appear here for fast re-inspection.")
                        .weak(),
                );
            });
        } else if filtered_records.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    egui::RichText::new(format!("No history entries matching \"{}\"", self.history_search))
                        .weak(),
                );
                ui.add_space(8.0);
                if ui.button("Clear Search").clicked() {
                    self.history_search.clear();
                }
            });
        } else {
            let mut file_to_load = None;
            let mut file_to_remove = None;

            egui::ScrollArea::vertical()
                .id_salt("history_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let full_width = ui.available_width();
                    let action_w = 78.0;
                    let format_w = (full_width * 0.16).clamp(80.0, 130.0);
                    let size_w = (full_width * 0.14).clamp(70.0, 110.0);
                    let props_w = (full_width * 0.10).clamp(50.0, 80.0);
                    let time_w = (full_width * 0.20).clamp(110.0, 160.0);
                    let name_w = (full_width - action_w - format_w - size_w - props_w - time_w - 40.0).max(100.0);

                    // Header
                    egui::Frame::NONE
                        .fill(ui.visuals().faint_bg_color)
                        .inner_margin(egui::Margin::symmetric(6, 6))
                        .corner_radius(4.0)
                        .show(ui, |ui| {
                            ui.set_width(full_width - 12.0);
                            ui.horizontal(|ui| {
                                ui.allocate_ui(egui::vec2(action_w, 0.0), |ui| {
                                    ui.label(egui::RichText::new("Action").strong());
                                });
                                ui.allocate_ui(egui::vec2(name_w, 0.0), |ui| {
                                    ui.label(egui::RichText::new("File Name").strong());
                                });
                                ui.allocate_ui(egui::vec2(format_w, 0.0), |ui| {
                                    ui.label(egui::RichText::new("Format").strong());
                                });
                                ui.allocate_ui(egui::vec2(size_w, 0.0), |ui| {
                                    ui.label(egui::RichText::new("Size").strong());
                                });
                                ui.allocate_ui(egui::vec2(props_w, 0.0), |ui| {
                                    ui.label(egui::RichText::new("Props").strong());
                                });
                                ui.allocate_ui(egui::vec2(time_w, 0.0), |ui| {
                                    ui.label(egui::RichText::new("Time").strong());
                                });
                            });
                        });

                    for (row_num, (_orig_idx, rec)) in filtered_records.iter().enumerate() {
                        let exists = Path::new(&rec.path).exists();
                        let row_bg = if row_num % 2 == 0 {
                            ui.visuals().faint_bg_color
                        } else {
                            egui::Color32::TRANSPARENT
                        };

                        egui::Frame::NONE
                            .fill(row_bg)
                            .inner_margin(egui::Margin::symmetric(6, 5))
                            .corner_radius(4.0)
                            .show(ui, |ui| {
                                ui.set_width(full_width - 12.0);
                                ui.horizontal(|ui| {
                                    ui.allocate_ui(egui::vec2(action_w, 0.0), |ui| {
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 4.0;
                                            if ui.add_enabled(exists, egui::Button::new("Inspect")).clicked() {
                                                file_to_load = Some(PathBuf::from(&rec.path));
                                            }
                                            if ui.small_button("🗑").on_hover_text("Remove from history").clicked() {
                                                file_to_remove = Some(rec.path.clone());
                                            }
                                        });
                                    });
                                    ui.allocate_ui(egui::vec2(name_w, 0.0), |ui| {
                                        let mut label = egui::RichText::new(&rec.file_name);
                                        if !exists {
                                            label = label.weak().strikethrough();
                                        }
                                        let tooltip = if exists {
                                            rec.path.clone()
                                        } else {
                                            format!("{} (File no longer found on disk)", rec.path)
                                        };
                                        ui.add(egui::Label::new(label).truncate())
                                            .on_hover_text(tooltip);
                                    });
                                    ui.allocate_ui(egui::vec2(format_w, 0.0), |ui| {
                                        ui.label(&rec.file_type_label);
                                    });
                                    ui.allocate_ui(egui::vec2(size_w, 0.0), |ui| {
                                        ui.label(&rec.file_size_formatted);
                                    });
                                    ui.allocate_ui(egui::vec2(props_w, 0.0), |ui| {
                                        ui.label(rec.properties_count.to_string());
                                    });
                                    ui.allocate_ui(egui::vec2(time_w, 0.0), |ui| {
                                        ui.label(egui::RichText::new(&rec.timestamp).small().weak());
                                    });
                                });
                            });
                    }

                    ui.add_space(36.0);
                });

            if let Some(path) = file_to_load {
                self.load_file(&path);
            }
            if let Some(path) = file_to_remove {
                self.history.remove(&path);
            }
        }
    }

    fn settings_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings");
        ui.separator();

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Appearance").strong());
        ui.horizontal(|ui| {
            ui.label("Theme:");
            let prev_theme = self.config.theme;
            ui.selectable_value(&mut self.config.theme, AppTheme::System, "System");
            ui.selectable_value(&mut self.config.theme, AppTheme::Dark, "Dark");
            ui.selectable_value(&mut self.config.theme, AppTheme::Light, "Light");

            if self.config.theme != prev_theme {
                let _ = self.config.save();
            }
        });

        ui.add_space(14.0);
        ui.label(egui::RichText::new("Metadata Inspection").strong());
        let prev_hashes = self.config.calculate_hashes;
        ui.checkbox(
            &mut self.config.calculate_hashes,
            "Compute cryptographic hashes (MD5, SHA-1, SHA-256) & Shannon entropy",
        );
        ui.label(
            egui::RichText::new("  (Automatically skipped for files > 100 MB to preserve instant responsiveness)")
                .small()
                .weak(),
        );

        let prev_copy = self.config.auto_copy_csv_on_inspect;
        ui.checkbox(
            &mut self.config.auto_copy_csv_on_inspect,
            "Automatically copy CSV to clipboard on file inspection",
        );

        if self.config.calculate_hashes != prev_hashes || self.config.auto_copy_csv_on_inspect != prev_copy {
            let _ = self.config.save();
        }

        ui.add_space(14.0);
        ui.label(egui::RichText::new("Data & Storage").strong());
        let data_dir = crate::config::config_dir();
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("Storage directory: {}", data_dir.display()))
                    .small()
                    .weak(),
            );

            #[cfg(target_os = "macos")]
            let reveal_dir_text = "Reveal in Finder";
            #[cfg(target_os = "windows")]
            let reveal_dir_text = "Show in Explorer";
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            let reveal_dir_text = "Show in File Manager";

            if ui.small_button(reveal_dir_text).clicked() {
                let _ = exporter::open_file_in_default_app(&data_dir);
            }
        });

        ui.add_space(14.0);
        ui.label(egui::RichText::new("Updates & About").strong());
        ui.horizontal(|ui| {
            ui.label(format!("Mta v{}", env!("CARGO_PKG_VERSION")));
            if ui.button("Check for Updates").clicked() {
                self.check_for_updates();
            }
        });

        let mut update_to_run = None;
        match &self.update_status {
            UpdateStatus::Checking => {
                ui.label(egui::RichText::new("Checking for updates...").italics());
            }
            UpdateStatus::Available {
                version,
                download_url,
                ..
            } => {
                let d_url = download_url.clone();
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("Version v{version} is available!"))
                            .color(egui::Color32::from_rgb(30, 140, 255)),
                    );
                    if ui.button("Install Update").clicked() {
                        update_to_run = Some(d_url);
                    }
                });
            }
            UpdateStatus::UpToDate => {
                ui.label(egui::RichText::new("✓ Mta is up to date").color(egui::Color32::GREEN));
            }
            UpdateStatus::Downloading => {
                ui.label("Downloading and applying update package...");
            }
            UpdateStatus::Applied => {
                ui.label(
                    egui::RichText::new("Update applied successfully! Restart Mta to use the new version.")
                        .color(egui::Color32::GREEN),
                );
            }
            UpdateStatus::Failed(err) => {
                ui.label(
                    egui::RichText::new(format!("Update check failed: {err}"))
                        .color(egui::Color32::from_rgb(240, 70, 70)),
                );
            }
            UpdateStatus::Idle => {}
        }

        if let Some(url) = update_to_run {
            self.perform_update(url);
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Pure Rust • Zero C Dependencies • MIT License").small().weak());
            ui.hyperlink_to("GitHub Repository", "https://github.com/salernoelia/mta");
        });
    }
}

impl eframe::App for MtaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.applied_theme != Some(self.config.theme) {
            match self.config.theme {
                AppTheme::System => ctx.set_theme(egui::ThemePreference::System),
                AppTheme::Dark => ctx.set_theme(egui::ThemePreference::Dark),
                AppTheme::Light => ctx.set_theme(egui::ThemePreference::Light),
            }
            self.applied_theme = Some(self.config.theme);
        }

        // Optimized reactive repaints: only request timer when processing background work
        if self.is_batch_loading {
            ctx.request_repaint_after(Duration::from_millis(50));
        } else if self.update_status == UpdateStatus::Downloading {
            ctx.request_repaint_after(Duration::from_millis(100));
        } else if self.update_status == UpdateStatus::Checking {
            ctx.request_repaint_after(Duration::from_millis(500));
        }

        self.apply_update_events();
        self.apply_batch_events();
        self.handle_keyboard_shortcuts(ctx);
        self.handle_drag_and_drop(ctx);
        self.draw_drag_overlay(ctx);

        // Status bar bottom panel
        egui::TopBottomPanel::bottom("status_bar")
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(&self.status_line);
                    if let Some(ref report) = self.current_report {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} properties • {}",
                                    report.total_entries_count(),
                                    report.mime_type
                                ))
                                .small()
                                .weak(),
                            );
                        });
                    }
                });
            });

        // Central Panel
        egui::CentralPanel::default().show(ctx, |ui| {
            self.tabs_bar_ui(ui);

            match self.active_tab {
                AppTab::Inspector => self.inspector_ui(ui),
                AppTab::Batch => self.batch_ui(ui),
                AppTab::History => self.history_ui(ui),
                AppTab::Settings => self.settings_ui(ui),
            }
        });
    }
}
