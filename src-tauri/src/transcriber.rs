const GROQ_WHISPER_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";

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
    client: reqwest::Client,
}

impl Transcriber {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("HTTP client"),
        }
    }

    pub fn is_ready(&self) -> bool {
        true
    }

    pub async fn transcribe(
        &self,
        samples: &[f32],
        sample_rate: u32,
        pause_before_secs: f32,
        api_key: &str,
    ) -> anyhow::Result<TranscriptionSegment> {
        let samples_16k = resample(samples, sample_rate, 16_000);
        let wav_bytes = encode_wav_pcm16(&samples_16k, 16_000);

        let part = reqwest::multipart::Part::bytes(wav_bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")?;

        let form = reqwest::multipart::Form::new()
            .text("model", "whisper-large-v3-turbo")
            .text("response_format", "text")
            .text("language", "en")
            .part("file", part);

        let resp = self
            .client
            .post(GROQ_WHISPER_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .multipart(form)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Groq Whisper error {}: {}", status, body);
        }

        let text = resp.text().await?.trim().to_string();
        Ok(TranscriptionSegment {
            text,
            pause_before_secs,
            tokens: vec![],
        })
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

fn encode_wav_pcm16(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let data_size = (samples.len() * 2) as u32;
    let file_size = 36 + data_size;

    let mut buf = Vec::with_capacity((44 + data_size) as usize);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");

    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());          // PCM
    buf.extend_from_slice(&1u16.to_le_bytes());          // mono
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    buf.extend_from_slice(&2u16.to_le_bytes());          // block align
    buf.extend_from_slice(&16u16.to_le_bytes());         // bits/sample

    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());
    for &s in samples {
        let s16 = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        buf.extend_from_slice(&s16.to_le_bytes());
    }
    buf
}
