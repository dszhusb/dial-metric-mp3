use eframe::egui;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

mod analyze;
mod frequency_bands;
mod utils;
use analyze::analyze_directory_batch;

use crate::utils::AnalysisResult;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 700.0])
            .with_title("Audio Frequency Analyzer"),
        ..Default::default()
    };

    eframe::run_native(
        "Audio Frequency Analyzer",
        options,
        Box::new(|_cc| Ok(Box::new(AudioAnalyzerApp::default()))),
    )
}

#[derive(Default)]
struct AudioAnalyzerApp {
    selected_folder: Option<PathBuf>,
    results: Arc<Mutex<Vec<AnalysisResult>>>,
    is_analyzing: Arc<Mutex<bool>>,
    progress: Arc<Mutex<String>>,
    sort_column: SortColumn,
    sort_ascending: bool,
}

#[derive(PartialEq, Clone, Copy)]
enum SortColumn {
    Filename,
    Centroid,
    Spread,
    ZCR,
    Loudness,
    Duration,
}

impl Default for SortColumn {
    fn default() -> Self {
        SortColumn::Filename
    }
}

impl AudioAnalyzerApp {
    fn select_folder(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.selected_folder = Some(path);
        }
    }

    fn start_analysis(&mut self) {
        if let Some(folder) = &self.selected_folder {
            let folder = folder.clone();
            let results = self.results.clone();
            let is_analyzing = self.is_analyzing.clone();
            let progress = self.progress.clone();

            *is_analyzing.lock().unwrap() = true;
            *results.lock().unwrap() = Vec::new();
            *progress.lock().unwrap() = "Starting analysis...".to_string();

            thread::spawn(move || {
                match analyze_directory_batch(&folder, progress.clone()) {
                    Ok(analysis_results) => {
                        *results.lock().unwrap() = analysis_results;
                        *progress.lock().unwrap() = "Analysis complete!".to_string();
                    }
                    Err(e) => {
                        *progress.lock().unwrap() = format!("Error: {}", e);
                    }
                }
                *is_analyzing.lock().unwrap() = false;
            });
        }
    }

    fn export_csv(&self) {
        let results = self.results.lock().unwrap();
        if results.is_empty() {
            return;
        }

        if let Some(path) = rfd::FileDialog::new()
            .set_file_name("audio_analysis.csv")
            .add_filter("CSV", &["csv"])
            .save_file()
        {
            if let Err(e) = export_to_csv(&results, &path) {
                eprintln!("Failed to export CSV: {}", e);
            }
        }
    }

    fn sort_results(&mut self, column: SortColumn) {
        if self.sort_column == column {
            self.sort_ascending = !self.sort_ascending;
        } else {
            self.sort_column = column;
            self.sort_ascending = true;
        }

        let mut results = self.results.lock().unwrap();
        let ascending = self.sort_ascending;

        results.sort_by(|a, b| {
            let cmp = match self.sort_column {
                SortColumn::Filename => a.filename.cmp(&b.filename),
                SortColumn::Centroid => a.centroid.partial_cmp(&b.centroid).unwrap(),
                SortColumn::Spread => a.spread.partial_cmp(&b.spread).unwrap(),
                SortColumn::ZCR => a
                    .zero_crossing_rate
                    .partial_cmp(&b.zero_crossing_rate)
                    .unwrap(),
                SortColumn::Loudness => a.loudness.partial_cmp(&b.loudness).unwrap(),
                SortColumn::Duration => {
                    a.duration_seconds.partial_cmp(&b.duration_seconds).unwrap()
                }
            };
            if ascending { cmp } else { cmp.reverse() }
        });
    }
}

impl eframe::App for AudioAnalyzerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🎵 Audio Frequency Analyzer");
            ui.add_space(10.0);

            // Folder selection
            ui.horizontal(|ui| {
                if ui.button("📁 Select Folder").clicked() {
                    self.select_folder();
                }

                if let Some(folder) = &self.selected_folder {
                    ui.label(format!("Selected: {}", folder.display()));
                } else {
                    ui.label("No folder selected");
                }
            });

            ui.add_space(10.0);

            // Analyze button
            ui.horizontal(|ui| {
                let is_analyzing = *self.is_analyzing.lock().unwrap();

                if ui
                    .add_enabled(
                        !is_analyzing && self.selected_folder.is_some(),
                        egui::Button::new("▶ Analyze"),
                    )
                    .clicked()
                {
                    self.start_analysis();
                }

                let results = self.results.lock().unwrap();
                if !results.is_empty() {
                    if ui.button("💾 Export CSV").clicked() {
                        self.export_csv();
                    }
                }

                let progress = self.progress.lock().unwrap();
                if !progress.is_empty() {
                    ui.label(&*progress);
                }
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            let results_clone = {
                let results = self.results.lock().unwrap();
                results.clone()
            };

            // Results table
            if !results_clone.is_empty() {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new("results_grid")
                        .striped(true)
                        .spacing([10.0, 4.0])
                        .show(ui, |ui| {
                            // Headers
                            if ui.button("Filename").clicked() {
                                self.sort_results(SortColumn::Filename);
                            }
                            if ui.button("Centroid").clicked() {
                                self.sort_results(SortColumn::Centroid);
                            }
                            if ui.button("Spread").clicked() {
                                self.sort_results(SortColumn::Spread);
                            }
                            if ui.button("ZCR").clicked() {
                                self.sort_results(SortColumn::ZCR);
                            }
                            if ui.button("Loudness").clicked() {
                                self.sort_results(SortColumn::Loudness);
                            }
                            if ui.button("Duration").clicked() {
                                self.sort_results(SortColumn::Duration);
                            }
                            ui.label("Bands");
                            ui.end_row();

                            // Data rows
                            for result in results_clone.iter() {
                                ui.label(&result.filename);
                                ui.label(format!("{:.1}", result.centroid));
                                ui.label(format!("{:.1}", result.spread));
                                ui.label(format!("{:.1}", result.zero_crossing_rate));
                                ui.label(format!("{:.1} dB", result.loudness));

                                let mins = result.duration_seconds as u32 / 60;
                                let secs = result.duration_seconds as u32 % 60;
                                ui.label(format!("{}:{:02}", mins, secs));

                                let bands_str = result
                                    .band_percentages
                                    .iter()
                                    .map(|p| format!("{:.0}%", p))
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                ui.label(bands_str);

                                ui.end_row();
                            }
                        });
                });
            } else if !*self.is_analyzing.lock().unwrap() {
                ui.label("No results yet. Select a folder and click Analyze to begin.");
            }
        });

        // Request repaint if analyzing
        if *self.is_analyzing.lock().unwrap() {
            ctx.request_repaint();
        }
    }
}

fn export_to_csv(results: &[AnalysisResult], path: &std::path::Path) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;

    writeln!(
        file,
        "Filename,Centroid,Spread,ZCR,Loudness (dB),Duration (s),Band1,Band2,Band3,Band4,Band5,Band6,Band7"
    )?;

    for result in results {
        write!(
            file,
            "{},{:.2},{:.2},{:.2},{:.2},{:.2}",
            result.filename,
            result.centroid,
            result.spread,
            result.zero_crossing_rate,
            result.loudness,
            result.duration_seconds
        )?;

        for pct in &result.band_percentages {
            write!(file, ",{:.2}", pct)?;
        }
        writeln!(file)?;
    }

    Ok(())
}
