use std::path::Path;
use std::sync::Arc;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Audio configuration for speech PCM processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeechAudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
}

impl SpeechAudioConfig {
    pub const TARGET_SAMPLE_RATE: u32 = 16000;
    pub const TARGET_CHANNELS: u16 = 1;

    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            sample_rate,
            channels,
        }
    }

    pub fn standard_16k_mono() -> Self {
        Self {
            sample_rate: Self::TARGET_SAMPLE_RATE,
            channels: Self::TARGET_CHANNELS,
        }
    }

    pub fn is_standard_16k_mono(&self) -> bool {
        self.sample_rate == Self::TARGET_SAMPLE_RATE && self.channels == Self::TARGET_CHANNELS
    }
}

impl Default for SpeechAudioConfig {
    fn default() -> Self {
        Self::standard_16k_mono()
    }
}

enum SpeechEngineBackend {
    Mock {
        mock_transcript: Option<String>,
    },
    Whisper(Arc<WhisperContext>),
}

/// Offline voice-to-code speech processing module powered by whisper.cpp.
/// Thread-safe and callable concurrently (`Send + Sync`).
#[derive(Clone)]
pub struct SpeechEngine {
    backend: Arc<SpeechEngineBackend>,
}

impl std::fmt::Debug for SpeechEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpeechEngine")
            .field("is_model_loaded", &self.is_model_loaded())
            .finish()
    }
}

impl SpeechEngine {
    /// Creates a mock SpeechEngine for test environments or offline fallbacks.
    pub fn new_mock() -> Self {
        Self {
            backend: Arc::new(SpeechEngineBackend::Mock {
                mock_transcript: None,
            }),
        }
    }

    /// Creates a mock SpeechEngine with a customized default transcription.
    pub fn new_mock_with_transcript(transcript: impl Into<String>) -> Self {
        Self {
            backend: Arc::new(SpeechEngineBackend::Mock {
                mock_transcript: Some(transcript.into()),
            }),
        }
    }

    /// Creates a SpeechEngine. If model_path is None, falls back to clean mock mode.
    /// If model_path is provided, loads the Whisper model context from disk.
    pub fn new(model_path: Option<&Path>) -> Result<Self, String> {
        match model_path {
            None => Ok(Self::new_mock()),
            Some(path) => {
                if !path.exists() {
                    return Err(format!(
                        "Whisper model file does not exist: {}",
                        path.display()
                    ));
                }

                let path_str = path.to_str().ok_or_else(|| {
                    format!("Invalid UTF-8 in model path: {}", path.display())
                })?;

                let mut params = WhisperContextParameters::default();
                params.use_gpu(true);

                let ctx = WhisperContext::new_with_params(path_str, params)
                    .map_err(|e| format!("Failed to load Whisper model from {}: {e}", path.display()))?;

                Ok(Self {
                    backend: Arc::new(SpeechEngineBackend::Whisper(Arc::new(ctx))),
                })
            }
        }
    }

    /// Returns true if a real Whisper model is loaded; false if running in mock mode.
    pub fn is_model_loaded(&self) -> bool {
        matches!(&*self.backend, SpeechEngineBackend::Whisper(_))
    }

    /// Normalizes PCM audio samples:
    /// 1. Verifies channel and sample rate configuration
    /// 2. Converts multi-channel audio to mono by averaging channels
    /// 3. Resamples audio to 16,000 Hz using linear interpolation
    /// 4. Sanitizes NaNs and Infinities
    /// 5. Bounds-checks and scales/clamps samples to [-1.0, 1.0]
    pub fn normalize_pcm(samples: &[f32], config: SpeechAudioConfig) -> Result<Vec<f32>, String> {
        if config.sample_rate == 0 {
            return Err("Invalid sample rate: 0".to_string());
        }
        if config.channels == 0 {
            return Err("Invalid channel count: 0".to_string());
        }
        if samples.is_empty() {
            return Ok(Vec::new());
        }

        // 1. Channel downmixing to mono
        let num_channels = config.channels as usize;
        let frame_count = samples.len() / num_channels;
        let mut mono = Vec::with_capacity(frame_count);

        if num_channels == 1 {
            mono.extend_from_slice(&samples[..frame_count]);
        } else {
            for frame_idx in 0..frame_count {
                let offset = frame_idx * num_channels;
                let mut sum = 0.0f32;
                for ch in 0..num_channels {
                    let s = samples[offset + ch];
                    sum += if s.is_finite() { s } else { 0.0 };
                }
                mono.push(sum / (num_channels as f32));
            }
        }

        // 2. Resampling to 16,000 Hz if necessary
        let resampled = if config.sample_rate == SpeechAudioConfig::TARGET_SAMPLE_RATE {
            mono
        } else {
            let target_rate = SpeechAudioConfig::TARGET_SAMPLE_RATE as f64;
            let src_rate = config.sample_rate as f64;
            let ratio = target_rate / src_rate;
            let target_len = ((mono.len() as f64) * ratio).round() as usize;

            if target_len == 0 {
                return Ok(Vec::new());
            }

            let mut out = Vec::with_capacity(target_len);
            let step = src_rate / target_rate;

            for i in 0..target_len {
                let src_pos = (i as f64) * step;
                let idx0 = src_pos.floor() as usize;
                let frac = (src_pos - idx0 as f64) as f32;
                let idx1 = (idx0 + 1).min(mono.len().saturating_sub(1));

                let s0 = mono.get(idx0).copied().unwrap_or(0.0);
                let s1 = mono.get(idx1).copied().unwrap_or(s0);

                let s0_clean = if s0.is_finite() { s0 } else { 0.0 };
                let s1_clean = if s1.is_finite() { s1 } else { 0.0 };

                out.push(s0_clean + frac * (s1_clean - s0_clean));
            }
            out
        };

        // 3. Bounds checking and normalization to [-1.0, 1.0]
        Ok(Self::normalize_bounds(&resampled))
    }

    /// Normalizes and clamps float samples into the [-1.0, 1.0] range.
    /// Handles NaN, Infinity, and 16-bit integer scale detection (values > 1.5).
    pub fn normalize_bounds(samples: &[f32]) -> Vec<f32> {
        if samples.is_empty() {
            return Vec::new();
        }

        // Find peak absolute amplitude among finite samples
        let mut max_abs = 0.0f32;
        for &s in samples {
            if s.is_finite() {
                let abs = s.abs();
                if abs > max_abs {
                    max_abs = abs;
                }
            }
        }

        // If samples appear to be unnormalized 16-bit integers (e.g. range [-32768, 32767])
        let scale = if max_abs > 32768.0 {
            1.0 / max_abs
        } else if max_abs > 1.5 {
            1.0 / 32768.0
        } else {
            1.0
        };

        samples
            .iter()
            .map(|&s| {
                if s.is_nan() {
                    0.0
                } else if s.is_infinite() {
                    if s.is_sign_positive() {
                        1.0
                    } else {
                        -1.0
                    }
                } else {
                    (s * scale).clamp(-1.0, 1.0)
                }
            })
            .collect()
    }

    /// Verifies if all samples are finite and strictly bounded within [-1.0, 1.0].
    pub fn is_normalized(samples: &[f32]) -> bool {
        samples.iter().all(|&s| s.is_finite() && (-1.0..=1.0).contains(&s))
    }

    /// Detects if an audio buffer contains only silence (amplitude below threshold).
    pub fn is_silence(samples: &[f32], threshold: f32) -> bool {
        samples.iter().all(|&s| !s.is_finite() || s.abs() < threshold)
    }

    /// Transcribes 16kHz mono float PCM samples into code/text.
    /// In mock mode:
    /// - Silent samples return empty string.
    /// - Synthetic/voiced samples return a mock voice-to-code transcript.
    /// In Whisper mode:
    /// - Normalizes sample bounds.
    /// - Creates a fresh WhisperState and runs greedy transcription.
    /// - Concatenates all recognized segments into a single trimmed string.
    pub fn transcribe_pcm(&self, samples: &[f32]) -> Result<String, String> {
        if samples.is_empty() {
            return Ok(String::new());
        }

        match &*self.backend {
            SpeechEngineBackend::Mock { mock_transcript } => {
                if Self::is_silence(samples, 1e-4) {
                    Ok(String::new())
                } else if let Some(text) = mock_transcript {
                    Ok(text.clone())
                } else {
                    Ok("fn main() {\n    println!(\"voice-to-code\");\n}".to_string())
                }
            }
            SpeechEngineBackend::Whisper(ctx) => {
                let normalized = if Self::is_normalized(samples) {
                    samples.to_vec()
                } else {
                    Self::normalize_bounds(samples)
                };

                if Self::is_silence(&normalized, 1e-4) {
                    return Ok(String::new());
                }

                let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
                let threads = std::thread::available_parallelism()
                    .map(|n| n.get() as i32)
                    .unwrap_or(4)
                    .min(8);

                params.set_n_threads(threads);
                params.set_translate(false);
                params.set_print_special(false);
                params.set_print_progress(false);
                params.set_print_realtime(false);
                params.set_print_timestamps(false);

                let mut state = ctx
                    .create_state()
                    .map_err(|e| format!("Failed to create Whisper state: {e}"))?;

                state
                    .full(params, &normalized)
                    .map_err(|e| format!("Whisper transcription failed: {e}"))?;

                let num_segments = state.full_n_segments();
                let mut transcript = String::new();
                for i in 0..num_segments {
                    if let Some(segment) = state.get_segment(i) {
                        if let Ok(text) = segment.to_str() {
                            transcript.push_str(text);
                        }
                    }
                }

                Ok(transcript.trim().to_string())
            }
        }
    }

    /// Convenience alias for `transcribe_pcm`.
    pub fn transcribe(&self, samples: &[f32]) -> Result<String, String> {
        self.transcribe_pcm(samples)
    }

    /// Normalizes PCM samples according to config and transcribes the resulting 16kHz mono buffer.
    pub fn transcribe_with_config(
        &self,
        samples: &[f32],
        config: SpeechAudioConfig,
    ) -> Result<String, String> {
        let normalized = Self::normalize_pcm(samples, config)?;
        self.transcribe_pcm(&normalized)
    }
}
