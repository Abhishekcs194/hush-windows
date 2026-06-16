use std::sync::Arc;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const MODEL_NAME: &str = "ggml-base.en.bin";
const MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin";

#[derive(Debug, Clone)]
pub struct TranscriptionSegment {
    pub text: String,
    pub pause_before_secs: f32,
}

pub struct Transcriber {
    ctx: Arc<WhisperContext>,
}

impl Transcriber {
    pub async fn new<F>(on_progress: F) -> anyhow::Result<Self>
    where
        F: Fn(&str, u32) + Send + 'static,
    {
        let model_path = Self::model_path()?;
        if !model_path.exists() {
            log::info!("Downloading Whisper model from HuggingFace…");
            use tokio::io::AsyncWriteExt;
            let mut resp = reqwest::get(MODEL_URL).await?;
            if !resp.status().is_success() {
                anyhow::bail!("Model download failed: {}", resp.status());
            }
            let total = resp.content_length().unwrap_or(148_000_000);
            let tmp_path = model_path.with_extension("tmp");
            let mut file = tokio::fs::File::create(&tmp_path).await?;
            let mut downloaded = 0u64;
            while let Some(chunk) = resp.chunk().await? {
                file.write_all(&chunk).await?;
                downloaded += chunk.len() as u64;
                let pct = ((downloaded as f64 / total as f64) * 100.0) as u32;
                on_progress("downloading", pct.min(99));
            }
            file.flush().await?;
            drop(file);
            tokio::fs::rename(&tmp_path, &model_path).await?;
            log::info!("Whisper model saved to {:?}", model_path);
        }
        on_progress("loading", 0);
        let path_str = model_path.to_string_lossy().to_string();
        let ctx = tokio::task::spawn_blocking(move || {
            WhisperContext::new_with_params(&path_str, WhisperContextParameters::default())
        })
        .await??;
        let ctx = Arc::new(ctx);
        // Warmup: one silent inference so the first real call isn't slow
        let ctx2 = Arc::clone(&ctx);
        tokio::task::spawn_blocking(move || {
            let mut state = ctx2.create_state()?;
            let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            p.set_language(Some("en"));
            p.set_print_progress(false);
            p.set_print_special(false);
            p.set_print_realtime(false);
            p.set_print_timestamps(false);
            let _ = state.full(p, &vec![0f32; 16_000]);
            Ok::<(), anyhow::Error>(())
        })
        .await??;
        log::info!("Whisper model ready");
        Ok(Self { ctx })
    }

    pub async fn transcribe(
        &self,
        samples: &[f32],
        sample_rate: u32,
        pause_before_secs: f32,
    ) -> anyhow::Result<TranscriptionSegment> {
        let samples_16k = resample(samples, sample_rate, 16_000);
        let ctx = Arc::clone(&self.ctx);
        let text = tokio::task::spawn_blocking(move || {
            let mut state = ctx.create_state()?;
            let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            p.set_language(Some("en"));
            p.set_print_progress(false);
            p.set_print_special(false);
            p.set_print_realtime(false);
            p.set_print_timestamps(false);
            p.set_suppress_blank(true);
            p.set_no_context(true);
            state.full(p, &samples_16k)?;
            let n = state.full_n_segments()?;
            let mut text = String::new();
            for i in 0..n {
                text.push_str(&state.full_get_segment_text(i)?);
            }
            Ok::<String, anyhow::Error>(text.trim().to_string())
        })
        .await??;
        Ok(TranscriptionSegment { text, pause_before_secs })
    }

    fn model_path() -> anyhow::Result<std::path::PathBuf> {
        let data_dir = dirs::data_dir().ok_or_else(|| anyhow::anyhow!("No AppData"))?;
        let models_dir = data_dir.join("Hush").join("models");
        std::fs::create_dir_all(&models_dir)?;
        Ok(models_dir.join(MODEL_NAME))
    }

}

fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let out_len = (samples.len() as f64 / ratio).ceil() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_pos = i as f64 * ratio;
        let lo = src_pos as usize;
        let frac = (src_pos - lo as f64) as f32;
        let s0 = samples.get(lo).copied().unwrap_or(0.0);
        let s1 = samples.get(lo + 1).copied().unwrap_or(0.0);
        out.push(s0 + (s1 - s0) * frac);
    }
    out
}
