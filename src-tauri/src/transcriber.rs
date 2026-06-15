use std::path::PathBuf;
use std::sync::Arc;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const MODEL_NAME: &str = "ggml-base.en.bin";
const MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin";

#[derive(Debug, Clone)]
pub struct TranscriptionSegment {
    pub text: String,
    pub pause_before_secs: f32,
    pub tokens: Vec<TokenInfo>,
}

#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub text: String,
    pub probability: f32,
}

pub struct Transcriber {
    ctx: Arc<WhisperContext>,
    is_ready: bool,
}

impl Transcriber {
    pub async fn new() -> anyhow::Result<Self> {
        let model_path = Self::model_path()?;
        if !model_path.exists() {
            log::info!("Model not found, downloading to {:?}", model_path);
            Self::download_model(&model_path).await?;
        }
        Self::from_path(model_path).await
    }

    pub async fn from_path(model_path: PathBuf) -> anyhow::Result<Self> {
        let path_str = model_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid model path"))?
            .to_string();

        let ctx = tokio::task::spawn_blocking(move || {
            WhisperContext::new_with_params(&path_str, WhisperContextParameters::default())
        })
        .await??;

        let mut t = Transcriber {
            ctx: Arc::new(ctx),
            is_ready: false,
        };

        t.warmup().await?;
        t.is_ready = true;
        log::info!("Whisper model ready");
        Ok(t)
    }

    pub fn is_ready(&self) -> bool {
        self.is_ready
    }

    pub fn model_path() -> anyhow::Result<PathBuf> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Cannot resolve AppData directory"))?;
        let models_dir = data_dir.join("Hush").join("models");
        std::fs::create_dir_all(&models_dir)?;
        Ok(models_dir.join(MODEL_NAME))
    }

    pub async fn download_model(path: &PathBuf) -> anyhow::Result<()> {
        log::info!("Downloading Whisper model from {}", MODEL_URL);
        let response = reqwest::get(MODEL_URL).await?;
        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Model download failed: {}",
                response.status()
            ));
        }
        let bytes = response.bytes().await?;
        std::fs::write(path, &bytes)?;
        log::info!("Model downloaded to {:?}", path);
        Ok(())
    }

    pub async fn warmup(&self) -> anyhow::Result<()> {
        let silence = vec![0.0f32; 16000]; // 1s of silence at 16kHz
        let _ = self.transcribe_raw(&silence, 16000).await;
        Ok(())
    }

    pub async fn transcribe(
        &self,
        samples: &[f32],
        sample_rate: u32,
        pause_before_secs: f32,
    ) -> anyhow::Result<TranscriptionSegment> {
        let resampled = if sample_rate != 16000 {
            resample(samples, sample_rate, 16000)
        } else {
            samples.to_vec()
        };

        let text = self.transcribe_raw(&resampled, 16000).await?;

        Ok(TranscriptionSegment {
            text: text.trim().to_string(),
            pause_before_secs,
            tokens: vec![],
        })
    }

    async fn transcribe_raw(&self, samples: &[f32], _sample_rate: u32) -> anyhow::Result<String> {
        let ctx = Arc::clone(&self.ctx);
        let samples_owned = samples.to_vec();

        tokio::task::spawn_blocking(move || {
            let mut state = ctx.create_state()?;
            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            params.set_language(Some("en"));
            params.set_print_realtime(false);
            params.set_print_progress(false);
            params.set_print_timestamps(false);
            params.set_single_segment(false);
            params.set_no_context(true);
            // Suppress common hallucinations at end of silence
            params.set_suppress_blank(true);

            state.full(params, &samples_owned)?;

            let n = state.full_n_segments()?;
            let mut text = String::new();
            for i in 0..n {
                let seg = state.full_get_segment_text(i)?;
                text.push_str(&seg);
            }
            Ok::<String, anyhow::Error>(text)
        })
        .await?
    }
}

/// Nearest-neighbour linear resampler — good enough for voice
fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let out_len = (samples.len() as f64 / ratio).ceil() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_idx = (i as f64 * ratio) as usize;
        out.push(*samples.get(src_idx).unwrap_or(&0.0));
    }
    out
}
