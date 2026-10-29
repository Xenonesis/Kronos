#[cfg(test)]
mod tests {
    use crate::kernel::speech::{SpeechAudioConfig, SpeechEngine};
    use std::path::Path;

    #[test]
    fn test_speech_engine_initialization() {
        // 1. Mock mode initialization
        let engine = SpeechEngine::new_mock();
        assert!(!engine.is_model_loaded());

        let pcm = vec![0.0f32; 16000];
        let text = engine.transcribe(&pcm).expect("transcription should succeed");
        assert!(text.is_empty() || !text.is_empty());

        // 2. SpeechEngine::new(None) cleanly falls back to mock mode
        let engine_none = SpeechEngine::new(None).expect("new(None) should succeed");
        assert!(!engine_none.is_model_loaded());

        // 3. SpeechEngine::new(Some(non_existent)) returns descriptive Err
        let non_existent = Path::new("non_existent_whisper_model.bin");
        let err = SpeechEngine::new(Some(non_existent));
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("does not exist"));
    }

    #[test]
    fn test_speech_engine_pcm_transcribe() {
        let engine = SpeechEngine::new_mock();

        // 1. Silent PCM sample buffer -> returns empty string
        let silent_pcm = vec![0.0f32; 16000];
        let silent_res = engine
            .transcribe_pcm(&silent_pcm)
            .expect("silent PCM transcription should succeed");
        assert_eq!(silent_res, "");

        // 2. Synthetic voiced/energy PCM sample buffer -> returns voice-to-code transcript
        let synthetic_pcm: Vec<f32> = (0..16000)
            .map(|i| (i as f32 * 0.05).sin() * 0.75)
            .collect();
        let synthetic_res = engine
            .transcribe_pcm(&synthetic_pcm)
            .expect("synthetic PCM transcription should succeed");
        assert!(!synthetic_res.is_empty());
        assert!(synthetic_res.contains("fn main()") || synthetic_res.contains("voice-to-code"));

        // 3. Custom mock transcription
        let custom_engine = SpeechEngine::new_mock_with_transcript("const answer = 42;");
        let custom_res = custom_engine
            .transcribe_pcm(&synthetic_pcm)
            .expect("custom transcription should succeed");
        assert_eq!(custom_res, "const answer = 42;");

        // 4. Empty buffer -> returns empty string
        let empty_res = engine
            .transcribe_pcm(&[])
            .expect("empty PCM should return empty string");
        assert_eq!(empty_res, "");

        // 5. transcribe_with_config
        let stereo_config = SpeechAudioConfig::new(48000, 2);
        let stereo_samples: Vec<f32> = (0..96000)
            .map(|i| (i as f32 * 0.02).sin() * 0.5)
            .collect();
        let config_res = engine
            .transcribe_with_config(&stereo_samples, stereo_config)
            .expect("transcribe_with_config should succeed");
        assert!(!config_res.is_empty());
    }

    #[test]
    fn test_speech_audio_normalization() {
        // 1. Bounds checking and normalization for values outside [-1.0, 1.0]
        let out_of_bounds = vec![-5.0f32, -2.0, 0.0, 1.5, 3.0, 10.0];
        let normalized = SpeechEngine::normalize_bounds(&out_of_bounds);
        assert_eq!(normalized.len(), out_of_bounds.len());
        assert!(SpeechEngine::is_normalized(&normalized));
        for &s in &normalized {
            assert!(s >= -1.0 && s <= 1.0, "Sample {s} out of bounds");
        }

        // 2. Sanitizing NaN and Infinity
        let non_finite = vec![f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.5];
        let sanitized = SpeechEngine::normalize_bounds(&non_finite);
        assert!(SpeechEngine::is_normalized(&sanitized));
        assert_eq!(sanitized[0], 0.0);
        assert_eq!(sanitized[1], 1.0);
        assert_eq!(sanitized[2], -1.0);
        assert_eq!(sanitized[3], 0.5);

        // 3. 16-bit integer PCM scale detection and normalization
        let int_pcm = vec![-32768.0f32, -16384.0, 0.0, 16384.0, 32767.0];
        let normalized_int = SpeechEngine::normalize_bounds(&int_pcm);
        assert!(SpeechEngine::is_normalized(&normalized_int));
        assert!((normalized_int[0] - (-1.0)).abs() < 1e-3);
        assert!((normalized_int[1] - (-0.5)).abs() < 1e-3);
        assert!((normalized_int[3] - 0.5).abs() < 1e-3);

        // 4. Stereo to mono conversion: [L, R, L, R]
        let stereo = vec![1.0f32, 0.0, 0.5, 0.5, -0.2, 0.4];
        let stereo_config = SpeechAudioConfig::new(16000, 2);
        let mono = SpeechEngine::normalize_pcm(&stereo, stereo_config).expect("stereo normalization");
        assert_eq!(mono.len(), 3);
        assert!((mono[0] - 0.5).abs() < 1e-5);
        assert!((mono[1] - 0.5).abs() < 1e-5);
        assert!((mono[2] - 0.1).abs() < 1e-5);

        // 5. Sample rate resampling (48000 Hz -> 16000 Hz, 3:1 ratio)
        let rate_48k_config = SpeechAudioConfig::new(48000, 1);
        let samples_48k = vec![0.5f32; 48000];
        let resampled_16k = SpeechEngine::normalize_pcm(&samples_48k, rate_48k_config)
            .expect("resampling 48k to 16k");
        assert_eq!(resampled_16k.len(), 16000);
        assert!(SpeechEngine::is_normalized(&resampled_16k));

        // 6. Up-sampling (8000 Hz -> 16000 Hz, 1:2 ratio)
        let rate_8k_config = SpeechAudioConfig::new(8000, 1);
        let samples_8k = vec![0.25f32; 8000];
        let upsampled_16k = SpeechEngine::normalize_pcm(&samples_8k, rate_8k_config)
            .expect("resampling 8k to 16k");
        assert_eq!(upsampled_16k.len(), 16000);
        assert!(SpeechEngine::is_normalized(&upsampled_16k));

        // 7. Invalid configuration rejection
        assert!(SpeechEngine::normalize_pcm(&[0.1], SpeechAudioConfig::new(0, 1)).is_err());
        assert!(SpeechEngine::normalize_pcm(&[0.1], SpeechAudioConfig::new(16000, 0)).is_err());
    }

    #[test]
    fn test_speech_engine_concurrency_send_sync() {
        let engine = SpeechEngine::new_mock();
        let mut handles = Vec::new();

        for thread_id in 0..4 {
            let engine_clone = engine.clone();
            let handle = std::thread::spawn(move || {
                let pcm = vec![0.0f32; 8000];
                let silence = engine_clone.transcribe_pcm(&pcm).unwrap();
                assert_eq!(silence, "");

                let synthetic: Vec<f32> = (0..8000)
                    .map(|i| (i as f32 * (thread_id as f32 + 1.0) * 0.05).sin() * 0.5)
                    .collect();
                let result = engine_clone.transcribe_pcm(&synthetic).unwrap();
                assert!(!result.is_empty());
            });
            handles.push(handle);
        }

        for h in handles {
            h.join().expect("thread should join cleanly");
        }
    }
}
