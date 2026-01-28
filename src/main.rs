mod frequency_bands;
mod utils;

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use frequency_bands::{SpectrumMetrics, get_bands};
use utils::get_samples;

use crate::{
    frequency_bands::{
        calculate_band_energies, calculate_zero_crossing_rate, print_histogram_bar,
        print_spectrum_position, print_spread_bar,
    },
    utils::{CachedMetrics, load_cache, save_cache, should_analyze, truncate_filename},
};

fn main() {
    let args: Vec<String> = env::args().collect();

    let target_path = if args.len() == 2 {
        PathBuf::from(&args[1])
    } else {
        env::current_dir().expect("Failed to get current directory")
    };

    if !target_path.is_dir() {
        eprintln!("Usage: {} [directory]", args[0]);
        eprintln!("If no path is provided, analyzes all MP3s in the current directory");
        std::process::exit(1);
    }

    analyze_directory(&target_path);
}

fn analyze_directory(dir_path: &Path) {
    let cache_file = dir_path.join("file_calc_cache.json");

    let mut cache = load_cache(&cache_file);

    // Read all entries in the directory
    let entries = match fs::read_dir(dir_path) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Error reading directory: {}", e);
            return;
        }
    };

    // Collect all MP3 files
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
        println!("No MP3 files found in directory: {}", dir_path.display());
        return;
    }

    mp3_files.sort();

    println!(
        "\nFound {} MP3 file(s) in {}\n",
        mp3_files.len(),
        dir_path.display()
    );
    println!("{}", "=".repeat(80));

    let mut updated = false;

    for file_path in mp3_files.iter() {
        let filename = file_path.file_name().unwrap().to_string_lossy().to_string();

        // Check if we need to analyze this file
        let needs_analysis = should_analyze(&file_path, &cache, &filename);

        if needs_analysis {
            if let Ok(metrics) = analyze_frequency_distribution(&file_path) {
                // Get file metadata
                let metadata = fs::metadata(&file_path).ok();
                let file_size = metadata.as_ref().and_then(|m| Some(m.len()));
                let modified_time = metadata.as_ref().and_then(|m| {
                    m.modified().ok().and_then(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .ok()
                            .map(|d| d.as_secs())
                    })
                });

                // Update cache
                cache.insert(
                    filename.clone(),
                    CachedMetrics {
                        filename: filename.clone(),
                        centroid: metrics.centroid,
                        spread: metrics.spread,
                        zero_crossing_rate: metrics.zero_crossing_rate,
                        band_percentages: metrics.band_percentages.clone(),
                        file_size,
                        modified_time,
                    },
                );
                updated = true;

                display_metrics(&filename, &metrics);
            } else {
                println!(
                    "\n{:<40}  ERROR: Failed to analyze",
                    truncate_filename(&filename, 40)
                );
            }
        } else {
            // Use cached data
            if let Some(cached) = cache.get(&filename) {
                let metrics = SpectrumMetrics {
                    centroid: cached.centroid,
                    spread: cached.spread,
                    zero_crossing_rate: cached.zero_crossing_rate,
                    band_percentages: cached.band_percentages.clone(),
                };
                display_metrics(&filename, &metrics);
            }
        }
    }

    // Save cache if updated
    if updated {
        save_cache(&cache_file, &cache);
    }
}

fn display_metrics(filename: &str, metrics: &SpectrumMetrics) {
    print!("\n{:<40}", truncate_filename(filename, 40));

    // Display individual band percentages as histogram
    for pct in &metrics.band_percentages {
        print!("  ");
        print_histogram_bar(*pct);
    }

    // Display spectral centroid
    print!("  │  Centroid: ");
    print_spectrum_position(metrics.centroid);
    print!(" ({:>5.1})", metrics.centroid);

    // Display spectral spread
    print!("  │  Spread: ");
    print_spread_bar(metrics.spread);
    print!(" ({:>5.1})", metrics.spread);

    // Display zero-crossing rate
    print!("  │  ZCR: ");
    print_spread_bar(metrics.zero_crossing_rate);
    println!(" ({:>5.1})", metrics.zero_crossing_rate);
}

fn analyze_frequency_distribution(
    path: &Path,
) -> Result<SpectrumMetrics, Box<dyn std::error::Error>> {
    let (all_samples, sample_rate) = get_samples(path)?;

    if all_samples.is_empty() {
        return Err("No audio data found".into());
    };

    let bands = get_bands(sample_rate);

    // Calculate energy distribution
    let band_energies = calculate_band_energies(&all_samples, sample_rate, &bands)?;

    // Calculate zero-crossing rate
    let zcr = calculate_zero_crossing_rate(&all_samples);

    // Calculate total energy
    let total_energy: f64 = band_energies.iter().sum();

    // Convert to percentages
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

    // Calculate spectral centroid (weighted average position)
    // Map each band to a position: 0 (sub-bass) to 100 (highs)
    let band_positions = [8.0, 18.0, 30.0, 45.0, 62.0, 78.0, 92.0];
    let centroid = band_percentages
        .iter()
        .zip(band_positions.iter())
        .map(|(pct, pos)| pct * pos)
        .sum::<f32>()
        / 100.0;

    // Calculate spectral spread (standard deviation from centroid)
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

    // Normalize spread to 0-100 scale (typical spread ranges from 0-35)
    let normalized_spread = (spread / 35.0 * 100.0).min(100.0);

    Ok(SpectrumMetrics {
        centroid,
        spread: normalized_spread,
        zero_crossing_rate: zcr,
        band_percentages,
    })
}
