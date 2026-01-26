use rustfft::{FftPlanner, num_complex::Complex};

const FRAME_SIZE: usize = 2048;
const HOP_SIZE: usize = 512;

pub struct FrequencyBand {
    pub(crate) name: &'static str,
    pub(crate) low_hz: usize,
    pub(crate) high_hz: usize,
}
pub struct BandAnalysis {
    pub(crate) band: FrequencyBand,
    pub(crate) total_energy: f64,
    pub(crate) percentage: f32,
}

pub struct SpectrumMetrics {
    pub(crate) centroid: f32, // Where on the spectrum (0-100, low to high)
    pub(crate) spread: f32,   // How distributed (0-100, focused to broad)
    pub(crate) band_percentages: Vec<f32>,
}

pub fn get_bands(sample_rate: usize) -> Vec<FrequencyBand> {
    vec![
        FrequencyBand {
            name: "Mud",
            low_hz: 0,
            high_hz: 20,
        },
        FrequencyBand {
            name: "Sub-bass",
            low_hz: 20,
            high_hz: 60,
        },
        FrequencyBand {
            name: "Bass",
            low_hz: 60,
            high_hz: 250,
        },
        FrequencyBand {
            name: "Low-mids",
            low_hz: 250,
            high_hz: 500,
        },
        FrequencyBand {
            name: "Mids",
            low_hz: 500,
            high_hz: 2000,
        },
        FrequencyBand {
            name: "High-mids",
            low_hz: 2000,
            high_hz: 4000,
        },
        FrequencyBand {
            name: "Highs",
            low_hz: 4000,
            high_hz: 6000,
        },
        FrequencyBand {
            name: "Air",
            low_hz: 6000,
            high_hz: sample_rate / 2,
        },
    ]
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
