use std::fs;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::{collections::HashMap, fs::File};

use minimp3::{Decoder, Frame};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct CachedMetrics {
    pub(crate) filename: String,
    pub(crate) centroid: f32,
    pub(crate) spread: f32,
    pub(crate) band_percentages: Vec<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) file_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) modified_time: Option<u64>,
}

pub fn get_samples(path: &Path) -> Result<(Vec<f32>, usize), Box<dyn std::error::Error>> {
    let file = File::open(Path::new(path))?;
    let mut decoder = Decoder::new(file);

    let mut all_samples = Vec::new();
    let mut sample_rate = 0;

    loop {
        match decoder.next_frame() {
            Ok(Frame {
                data,
                sample_rate: sr,
                ..
            }) => {
                sample_rate = sr as usize;
                // Convert to mono by averaging channels
                for chunk in data.chunks(2) {
                    let mono = chunk.iter().map(|&x| x as f32).sum::<f32>() / chunk.len() as f32;
                    all_samples.push(mono);
                }
            }
            Err(minimp3::Error::Eof) => break,
            Err(e) => return Err(Box::new(e)),
        }
    }
    return Ok((all_samples, sample_rate));
}

pub fn truncate_filename(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        name.to_string()
    } else {
        format!("{}...", &name[..max_len - 3])
    }
}

pub fn load_cache(cache_file: &Path) -> HashMap<String, CachedMetrics> {
    if let Ok(file) = File::open(cache_file) {
        let reader = BufReader::new(file);
        serde_json::from_reader(reader).unwrap_or_default()
    } else {
        HashMap::new()
    }
}

pub fn save_cache(cache_file: &Path, cache: &HashMap<String, CachedMetrics>) {
    if let Ok(file) = File::create(cache_file) {
        let writer = BufWriter::new(file);
        let _ = serde_json::to_writer_pretty(writer, cache);
    }
}

pub fn should_analyze(
    file_path: &Path,
    cache: &HashMap<String, CachedMetrics>,
    filename: &str,
) -> bool {
    // If not in cache, analyze
    let Some(cached) = cache.get(filename) else {
        return true;
    };

    // If file metadata changed, re-analyze
    if let Ok(metadata) = fs::metadata(file_path) {
        if let Some(cached_size) = cached.file_size {
            if metadata.len() != cached_size {
                return true;
            }
        }

        if let Some(cached_time) = cached.modified_time {
            if let Ok(modified) = metadata.modified() {
                if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                    if duration.as_secs() != cached_time {
                        return true;
                    }
                }
            }
        }
    }

    // File hasn't changed, use cache
    false
}
