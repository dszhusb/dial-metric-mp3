use std::fs::File;
use std::path::Path;

use minimp3::{Decoder, Frame};

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
