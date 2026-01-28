use rustfft::{FftPlanner, num_complex::Complex};

const FRAME_SIZE: usize = 2048;
const HOP_SIZE: usize = 512;

pub struct FrequencyBand {
    pub(crate) low_hz: usize,
    pub(crate) high_hz: usize,
}

pub struct SpectrumMetrics {
    pub(crate) centroid: f32, // Where on the spectrum (0-100, low to high)
    pub(crate) spread: f32,   // How distributed (0-100, focused to broad)
    pub(crate) zero_crossing_rate: f32, // Sharpness/noisiness (0-100)
    pub(crate) band_percentages: Vec<f32>,
}

pub fn get_bands(sample_rate: usize) -> Vec<FrequencyBand> {
    vec![
        FrequencyBand {
            low_hz: 20,
            high_hz: 60,
        },
        FrequencyBand {
            low_hz: 60,
            high_hz: 250,
        },
        FrequencyBand {
            low_hz: 250,
            high_hz: 500,
        },
        FrequencyBand {
            low_hz: 500,
            high_hz: 2000,
        },
        FrequencyBand {
            low_hz: 2000,
            high_hz: 4000,
        },
        FrequencyBand {
            low_hz: 4000,
            high_hz: 6000,
        },
        FrequencyBand {
            low_hz: 6000,
            high_hz: sample_rate / 2,
        },
    ]
}

pub fn calculate_band_positions(bands: &[FrequencyBand], sample_rate: usize) -> Vec<f32> {
    let nyquist = sample_rate as f32 / 2.0;

    bands
        .iter()
        .map(|band| {
            // Calculate geometric mean (center) of the band
            let low = band.low_hz.max(1) as f32;
            let high = band.high_hz.min(sample_rate / 2) as f32;
            let center = (low * high).sqrt();

            // Map to 0-100 scale using logarithmic scaling
            // Human hearing is logarithmic (octaves, not linear Hz)
            let log_min = 20.0_f32.ln(); // 20 Hz
            let log_max = nyquist.ln();
            let log_center = center.ln();

            // Normalize to 0-100
            ((log_center - log_min) / (log_max - log_min) * 100.0).clamp(0.0, 100.0)
        })
        .collect()
}

pub fn calculate_band_energies(
    samples: &[f32],
    sample_rate: usize,
    bands: &[FrequencyBand],
) -> Result<Vec<f64>, Box<dyn std::error::Error>> {
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(FRAME_SIZE);

    // Convert Hz ranges to bin indices
    let band_bins: Vec<(usize, usize)> = bands
        .iter()
        .map(|band| {
            let low_bin = (band.low_hz * FRAME_SIZE / sample_rate).max(0);
            let high_bin = (band.high_hz * FRAME_SIZE / sample_rate).min(FRAME_SIZE / 2);
            (low_bin, high_bin)
        })
        .collect();

    let mut band_energies = vec![0.0f64; bands.len()];
    let mut frame_count = 0;

    // Process audio in overlapping frames
    for i in (0..samples.len().saturating_sub(FRAME_SIZE)).step_by(HOP_SIZE) {
        let frame = &samples[i..i + FRAME_SIZE];

        // Apply Hann window
        let mut windowed: Vec<Complex<f32>> = frame
            .iter()
            .enumerate()
            .map(|(j, &s)| {
                let window = 0.5
                    * (1.0
                        - ((2.0 * std::f32::consts::PI * j as f32) / (FRAME_SIZE as f32 - 1.0))
                            .cos());
                Complex::new(s * window, 0.0)
            })
            .collect();

        // Perform FFT
        fft.process(&mut windowed);

        // Calculate magnitude spectrum
        let magnitude: Vec<f32> = windowed[..FRAME_SIZE / 2]
            .iter()
            .map(|c| (c.re * c.re + c.im * c.im).sqrt())
            .collect();

        // Accumulate energy per band
        for (band_idx, &(low_bin, high_bin)) in band_bins.iter().enumerate() {
            let band_energy: f32 = magnitude[low_bin..high_bin].iter().map(|&m| m * m).sum();
            band_energies[band_idx] += band_energy as f64;
        }

        frame_count += 1;
    }

    // Average over all frames
    for energy in &mut band_energies {
        *energy /= frame_count as f64;
    }

    Ok(band_energies)
}

pub fn calculate_zero_crossing_rate(samples: &[f32]) -> f32 {
    if samples.len() < 2 {
        return 0.0;
    }

    let mut zero_crossings = 0;

    for i in 1..samples.len() {
        // Check if sign changed (crossed zero)
        if (samples[i] >= 0.0 && samples[i - 1] < 0.0)
            || (samples[i] < 0.0 && samples[i - 1] >= 0.0)
        {
            zero_crossings += 1;
        }
    }

    // Calculate rate as crossings per sample
    let zcr = zero_crossings as f32 / samples.len() as f32;

    // Normalize to 0-100 scale
    // Typical ZCR ranges from ~0.01 (bass-heavy) to ~0.15 (very sharp/noisy)
    // We'll map 0.15 to 100 for normalization
    let normalized_zcr = (zcr / 0.15 * 100.0).min(100.0);

    normalized_zcr
}

pub fn print_spectrum_position(centroid: f32) {
    let bar_width = 20;
    let position = ((centroid / 100.0) * bar_width as f32) as usize;

    print!("[");
    for i in 0..bar_width {
        if i == position {
            print!("●");
        } else {
            print!("─");
        }
    }
    print!("]");
}

pub fn print_spread_bar(spread: f32) {
    let bar_width = 10;
    let filled = ((spread / 100.0) * bar_width as f32) as usize;

    print!("[");
    for i in 0..bar_width {
        if i < filled {
            print!("█");
        } else {
            print!("░");
        }
    }
    print!("]");
}

pub fn print_histogram_bar(percentage: f32) {
    // Each character represents 5% (max 20 chars for 100%)
    let max_width = 10;
    let blocks = (percentage / 10.0 * max_width as f32) * 8.0;
    let full_blocks = blocks as usize / 8;
    let remainder = blocks as usize % 8;

    print!("{:>5.1}% | ", percentage);
    let block_chars: [char; 9] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];
    for _ in 0..full_blocks {
        print!("█");
    }
    if remainder > 0 {
        print!("{}", block_chars[remainder]);
    }
    println!();
}
