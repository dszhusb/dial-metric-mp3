use std::fs::{self};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::frequency_bands::{
    calculate_band_energies, calculate_band_positions, calculate_loudness,
    calculate_zero_crossing_rate, get_bands,
};
use crate::utils::{AnalysisResult, get_samples, load_cache, save_cache, should_analyze};

pub fn analyze_directory_batch(
    dir_path: &Path,
    progress: Arc<Mutex<String>>,
) -> Result<Vec<AnalysisResult>, Box<dyn std::error::Error>> {
    let cache_file = dir_path.join(".frequency_analysis_cache.json");
    let mut cache = load_cache(&cache_file);

    let entries = fs::read_dir(dir_path)?;

    let mut mp3_files: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("mp3"))
                .unwrap_or(false)
        })
        .collect();

    if mp3_files.is_empty() {
        return Err("No MP3 files found in directory".into());
    }

    mp3_files.sort();

    let mut results: Vec<AnalysisResult> = Vec::new();
    let total = mp3_files.len();
    let mut updated = false;

    for (idx, file_path) in mp3_files.iter().enumerate() {
        let filename = file_path.file_name().unwrap().to_string_lossy().to_string();

        *progress.lock().unwrap() = format!("Analyzing {} of {}: {}", idx + 1, total, filename);

        let needs_analysis = should_analyze(&file_path, &cache, &filename);

        if needs_analysis {
            match analyze_file(&file_path) {
                Ok(metrics) => {
                    cache.insert(
                        filename.clone(),
                        AnalysisResult {
                            filename: filename.clone(),
                            centroid: metrics.centroid,
                            spread: metrics.spread,
                            zero_crossing_rate: metrics.zero_crossing_rate,
                            loudness: metrics.loudness,
                            duration_seconds: metrics.duration_seconds,
                            band_percentages: metrics.band_percentages.clone(),
                            file_size: metrics.file_size,
                            modified_time: metrics.modified_time,
                        },
                    );
                    updated = true;

                    results.push(metrics);
                }
                Err(e) => {
                    eprintln!("Error analyzing {}: {}", filename, e);
                }
            }
        } else {
            if let Some(cached) = cache.get(&filename) {
                results.push(AnalysisResult {
                    filename: filename.clone(),
                    centroid: cached.centroid,
                    spread: cached.spread,
                    zero_crossing_rate: cached.zero_crossing_rate,
                    loudness: cached.loudness,
                    duration_seconds: cached.duration_seconds,
                    band_percentages: cached.band_percentages.clone(),
                    file_size: cached.file_size,
                    modified_time: cached.modified_time,
                });
            }
        }
    }

    if updated {
        save_cache(&cache_file, &cache);
    }

    Ok(results)
}

fn analyze_file(path: &Path) -> Result<AnalysisResult, Box<dyn std::error::Error>> {
    let (all_samples, sample_rate) = get_samples(path)?;

    if all_samples.is_empty() {
        return Err("No audio data found".into());
    }

    let duration_seconds = all_samples.len() as f32 / sample_rate as f32;

    let loudness = calculate_loudness(&all_samples);

    let bands = get_bands(sample_rate);

    let band_energies = calculate_band_energies(&all_samples, sample_rate, &bands)?;

    let zcr = calculate_zero_crossing_rate(&all_samples);

    let total_energy: f64 = band_energies.iter().sum();

    let band_percentages: Vec<f32> = band_energies
        .iter()
        .map(|&energy| {
            if total_energy > 0.0 {
                (energy / total_energy * 100.0) as f32
            } else {
                0.0
            }
        })
        .collect();

    let band_positions = calculate_band_positions(&bands, sample_rate);

    let centroid = band_percentages
        .iter()
        .zip(band_positions.iter())
        .map(|(pct, pos)| pct * pos)
        .sum::<f32>()
        / 100.0;

    let variance = band_percentages
        .iter()
        .zip(band_positions.iter())
        .map(|(pct, pos)| {
            let diff = pos - centroid;
            pct * diff * diff
        })
        .sum::<f32>()
        / 100.0;

    let spread = variance.sqrt();
    let normalized_spread = (spread / 35.0 * 100.0).min(100.0);

    let metadata = fs::metadata(&path).ok();
    let file_size = metadata.as_ref().and_then(|m| Some(m.len()));
    let modified_time = metadata.as_ref().and_then(|m| {
        m.modified().ok().and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs())
        })
    });

    Ok(AnalysisResult {
        filename: path.file_name().unwrap().to_string_lossy().to_string(),
        centroid,
        spread: normalized_spread,
        zero_crossing_rate: zcr,
        loudness,
        duration_seconds,
        band_percentages,
        file_size,
        modified_time,
    })
}
