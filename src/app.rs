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

    // Single file inspector state
    current_report: Option<FileMetadataReport>,
    filter_query: String,
    selected_category: Option<String>,

    // Batch inspector state
    batch_reports: Vec<FileMetadataReport>,
    is_batch_loading: bool,

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

        // Spawn background update check
        spawn_update_check(update_event_tx.clone());

        let mut app = Self {
            config,
            active_tab: AppTab::Inspector,
            history,
            current_report: None,
            filter_query: String::new(),
            selected_category: None,
            batch_reports: Vec::new(),
            is_batch_loading: false,
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
        self.is_batch_loading = true;
        let calculate_hashes = self.config.calculate_hashes;
        let mut loaded = Vec::new();

        for p in paths {
            if p.is_file() {
                if let Ok(rep) = inspect_file(p, calculate_hashes) {
                    loaded.push(rep);
                }
            }
        }

        let added_count = loaded.len();
        self.batch_reports.extend(loaded);
        self.is_batch_loading = false;
        self.status_line = format!("Loaded {added_count} files into batch inspector");
        self.active_tab = AppTab::Batch;
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
                        "📥 Drop file to inspect metadata",
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
            ui.selectable_value(&mut self.active_tab, AppTab::Inspector, "🔍 Inspector");
            ui.selectable_value(&mut self.active_tab, AppTab::Batch, "📚 Batch");
            ui.selectable_value(&mut self.active_tab, AppTab::History, "⏱ History");
            ui.selectable_value(&mut self.active_tab, AppTab::Settings, "⚙ Settings");

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                match &self.update_status {
                    UpdateStatus::Available { version, download_url, .. } => {
                        let btn = egui::Button::new(format!("⭐ Update to v{version}"))
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

                // Row 3: Action Buttons Toolbar (dedicated row)
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    if ui.button("💾 Export CSV").on_hover_text("Save metadata as CSV file").clicked() {
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

                    if ui.button("📋 Copy CSV").on_hover_text("Copy table as CSV to clipboard").clicked() {
                        if let Ok(csv) = report.to_csv() {
                            if exporter::copy_to_clipboard(&csv).is_ok() {
                                self.status_line = "Copied CSV to clipboard".to_string();
                            }
                        }
                    }

                    if ui.button("💾 Export JSON").on_hover_text("Save metadata as JSON file").clicked() {
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

                    if ui.button("📋 Copy Summary").on_hover_text("Copy markdown summary to clipboard").clicked() {
                        let md = report.to_markdown();
                        if exporter::copy_to_clipboard(&md).is_ok() {
                            self.status_line = "Copied summary to clipboard".to_string();
                        }
                    }

                    ui.separator();

                    if ui.button("📂 Open Another...").clicked() {
                        if let Some(p) = rfd::FileDialog::new().pick_file() {
                            self.load_file(&p);
                        }
                    }

                    if ui.button("✕ Close").clicked() {
                        self.current_report = None;
                    }
                });
            });

            ui.add_space(6.0);

            // Filter & Search bar
            ui.horizontal_wrapped(|ui| {
                ui.label("Filter:");
                let text_edit = ui.add(
                    egui::TextEdit::singleline(&mut self.filter_query)
                        .hint_text("Search properties...")
                        .desired_width(160.0),
                );
                if text_edit.changed() {
                    // Filter dynamically
                }
                if !self.filter_query.is_empty() && ui.small_button("✕").clicked() {
                    self.filter_query.clear();
                }

                // Category chips
                ui.separator();
                let is_all = self.selected_category.is_none();
                if ui.selectable_label(is_all, "All").clicked() {
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

                                        // Column 2: Value
                                        ui.allocate_ui_with_layout(
                                            egui::vec2(val_col_w, 0.0),
                                            egui::Layout::left_to_right(egui::Align::Center),
                                            |ui| {
                                                let is_link = entry.value.starts_with("http://")
                                                    || entry.value.starts_with("https://");

                                                if is_link {
                                                    ui.hyperlink(&entry.value);
                                                } else {
                                                    ui.add(egui::Label::new(&entry.value).wrap());
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

                    // Ample bottom space so bottom-most item is never obscured
                    ui.add_space(36.0);
                });
        } else {
            // Empty State: Drop zone
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);

                let drop_rect_size = egui::vec2(480.0, 240.0);
                let (rect, _response) = ui.allocate_exact_size(drop_rect_size, egui::Sense::hover());

                let painter = ui.painter();
                painter.rect_stroke(
                    rect,
                    12.0,
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(30, 80, 220)),
                    egui::StrokeKind::Middle,
                );

                painter.rect_filled(
                    rect,
                    12.0,
                    egui::Color32::from_rgba_unmultiplied(30, 80, 220, 18),
                );

                let center = rect.center();
                painter.text(
                    center - egui::vec2(0.0, 36.0),
                    egui::Align2::CENTER_CENTER,
                    "📂 Drag & Drop Any File Here",
                    egui::FontId::proportional(22.0),
                    egui::Color32::WHITE,
                );

                painter.text(
                    center - egui::vec2(0.0, 4.0),
                    egui::Align2::CENTER_CENTER,
                    "Images, PDFs, Documents, Audio, Video, Archives, Code, Fonts & more",
                    egui::FontId::proportional(13.0),
                    egui::Color32::from_gray(180),
                );

                ui.add_space(20.0);

                if ui.button(egui::RichText::new("Browse File...").size(16.0)).clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        self.load_file(&path);
                    }
                }

                ui.add_space(30.0);

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
            if ui.button("➕ Add Files...").clicked() {
                if let Some(files) = rfd::FileDialog::new().pick_files() {
                    self.load_batch_files(&files);
                }
            }

            if ui.button("📁 Add Folder...").clicked() {
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
                if ui.button("🗑 Clear Batch").clicked() {
                    self.batch_reports.clear();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("💾 Export Batch to CSV").clicked() {
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

                    if ui.button("📋 Copy Batch CSV").clicked() {
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
                ui.label(egui::RichText::new("No batch files loaded.").heading());
                ui.label("Drop multiple files or click 'Add Files' / 'Add Folder' to inspect them together.");
            });
        } else {
            ui.label(format!("Loaded {} files in batch", self.batch_reports.len()));
            ui.add_space(4.0);

            let mut inspect_idx = None;

            egui::ScrollArea::both()
                .id_salt("batch_table_scroll")
                .show(ui, |ui| {
                    egui::Grid::new("batch_grid")
                        .striped(true)
                        .spacing([18.0, 8.0])
                        .show(ui, |ui| {
                            // Header
                            ui.label(egui::RichText::new("Action").strong());
                            ui.label(egui::RichText::new("File Name").strong());
                            ui.label(egui::RichText::new("Format").strong());
                            ui.label(egui::RichText::new("Size").strong());
                            ui.label(egui::RichText::new("Properties").strong());
                            ui.label(egui::RichText::new("Path").strong());
                            ui.end_row();

                            for (idx, report) in self.batch_reports.iter().enumerate() {
                                if ui.small_button("🔍 View").clicked() {
                                    inspect_idx = Some(idx);
                                }
                                ui.label(&report.file_name);
                                ui.label(&report.file_type_label);
                                ui.label(&report.file_size_formatted);
                                ui.label(report.total_entries_count().to_string());
                                ui.label(
                                    egui::RichText::new(report.path.display().to_string())
                                        .small()
                                        .weak(),
                                );
                                ui.end_row();
                            }
                        });
                    ui.add_space(36.0);
                });

            if let Some(idx) = inspect_idx {
                self.current_report = Some(self.batch_reports[idx].clone());
                self.active_tab = AppTab::Inspector;
            }
        }
    }

    fn history_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Inspection History");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !self.history.records.is_empty() && ui.button("🗑 Clear History").clicked() {
                    self.history.clear();
                }
            });
        });
        ui.separator();

        if self.history.records.is_empty() {
            ui.label("No recent inspections.");
        } else {
            let mut file_to_load = None;
            let mut file_to_remove = None;

            egui::ScrollArea::vertical()
                .id_salt("history_scroll")
                .show(ui, |ui| {
                    egui::Grid::new("history_grid")
                        .striped(true)
                        .spacing([18.0, 8.0])
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Action").strong());
                            ui.label(egui::RichText::new("File Name").strong());
                            ui.label(egui::RichText::new("Format").strong());
                            ui.label(egui::RichText::new("Size").strong());
                            ui.label(egui::RichText::new("Properties").strong());
                            ui.label(egui::RichText::new("Time").strong());
                            ui.end_row();

                            for rec in &self.history.records {
                                ui.horizontal(|ui| {
                                    if ui.small_button("Inspect").clicked() {
                                        file_to_load = Some(PathBuf::from(&rec.path));
                                    }
                                    if ui.small_button("🗑").on_hover_text("Remove from history").clicked() {
                                        file_to_remove = Some(rec.path.clone());
                                    }
                                });
                                ui.label(&rec.file_name);
                                ui.label(&rec.file_type_label);
                                ui.label(&rec.file_size_formatted);
                                ui.label(rec.properties_count.to_string());
                                ui.label(&rec.timestamp);
                                ui.end_row();
                            }
                        });
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
        let prev_copy = self.config.auto_copy_csv_on_inspect;
        ui.checkbox(
            &mut self.config.auto_copy_csv_on_inspect,
            "Automatically copy CSV to clipboard on file inspection",
        );

        if self.config.calculate_hashes != prev_hashes || self.config.auto_copy_csv_on_inspect != prev_copy {
            let _ = self.config.save();
        }

        ui.add_space(14.0);
        ui.label(egui::RichText::new("Updates").strong());
        ui.horizontal(|ui| {
            ui.label(format!("Installed version: v{}", env!("CARGO_PKG_VERSION")));
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

        ctx.request_repaint_after(Duration::from_millis(100));
        self.apply_update_events();
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
