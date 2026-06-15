use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::Sender;
use std::sync::{Arc, Mutex};

const SILENCE_CUT_SECS: f32 = 1.0;
const MIN_CHUNK_SECS: f32 = 1.6;
const MAX_CHUNK_SECS: f32 = 14.0;
const TAIL_DRAIN_SECS: f32 = 0.3;

pub struct AudioChunk {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub pause_before_secs: f32,
}

pub enum AudioEvent {
    Chunk(AudioChunk),
    LevelUpdate(f32),
    RecordingStarted,
    RecordingStopped,
}

struct VadState {
    chunk_samples: Vec<f32>,
    silence_samples: usize,
    speech_in_chunk: bool,
    session_peak: f32,
    sample_rate: u32,
    tail_drain_remaining: usize,
    is_draining_tail: bool,
    last_silence_secs: f32,
    recording: bool,
}

impl VadState {
    fn new(sample_rate: u32) -> Self {
        Self {
            chunk_samples: Vec::new(),
            silence_samples: 0,
            speech_in_chunk: false,
            session_peak: 0.0,
            sample_rate,
            tail_drain_remaining: (TAIL_DRAIN_SECS * sample_rate as f32) as usize,
            is_draining_tail: false,
            last_silence_secs: 0.0,
            recording: false,
        }
    }

    fn adaptive_threshold(&self) -> f32 {
        let raw = self.session_peak * 0.30;
        raw.max(0.002).min(0.010)
    }

    fn chunk_secs(&self) -> f32 {
        self.chunk_samples.len() as f32 / self.sample_rate as f32
    }

    fn silence_secs(&self) -> f32 {
        self.silence_samples as f32 / self.sample_rate as f32
    }
}

pub struct AudioCapture {
    stream: Option<cpal::Stream>,
    vad: Arc<Mutex<VadState>>,
    event_tx: Sender<AudioEvent>,
    sample_rate: u32,
}

impl AudioCapture {
    pub fn new(event_tx: Sender<AudioEvent>) -> anyhow::Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No input device found"))?;
        let config = device.default_input_config()?;
        let sample_rate = config.sample_rate().0;

        Ok(Self {
            stream: None,
            vad: Arc::new(Mutex::new(VadState::new(sample_rate))),
            event_tx,
            sample_rate,
        })
    }

    pub fn named(device_name: &str, event_tx: Sender<AudioEvent>) -> anyhow::Result<Self> {
        let host = cpal::default_host();
        let device = host
            .input_devices()?
            .find(|d| d.name().map(|n| n == device_name).unwrap_or(false))
            .ok_or_else(|| anyhow::anyhow!("Device '{}' not found", device_name))?;
        let config = device.default_input_config()?;
        let sample_rate = config.sample_rate().0;

        Ok(Self {
            stream: None,
            vad: Arc::new(Mutex::new(VadState::new(sample_rate))),
            event_tx,
            sample_rate,
        })
    }

    pub fn list_devices() -> anyhow::Result<Vec<String>> {
        let host = cpal::default_host();
        let names = host
            .input_devices()?
            .filter_map(|d| d.name().ok())
            .collect();
        Ok(names)
    }

    pub fn start(&mut self) -> anyhow::Result<()> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No input device"))?;
        let config = device.default_input_config()?;
        let sample_rate = config.sample_rate().0;

        let vad = Arc::clone(&self.vad);
        let tx = self.event_tx.clone();

        {
            let mut v = vad.lock().unwrap();
            v.recording = true;
            v.chunk_samples.clear();
            v.silence_samples = 0;
            v.speech_in_chunk = false;
            v.session_peak = 0.0;
            v.is_draining_tail = false;
            v.tail_drain_remaining = (TAIL_DRAIN_SECS * sample_rate as f32) as usize;
        }

        let _ = tx.try_send(AudioEvent::RecordingStarted);

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                build_stream::<f32>(&device, &config.into(), vad, tx)?
            }
            cpal::SampleFormat::I16 => {
                build_stream::<i16>(&device, &config.into(), vad, tx)?
            }
            cpal::SampleFormat::U16 => {
                build_stream::<u16>(&device, &config.into(), vad, tx)?
            }
            _ => return Err(anyhow::anyhow!("Unsupported sample format")),
        };

        stream.play()?;
        self.stream = Some(stream);
        self.sample_rate = sample_rate;
        Ok(())
    }

    pub fn stop(&mut self) {
        // Signal tail drain
        if let Ok(mut v) = self.vad.lock() {
            v.is_draining_tail = true;
        }
        // Drop the stream after a short delay to capture tail
        std::thread::sleep(std::time::Duration::from_millis(
            (TAIL_DRAIN_SECS * 1000.0) as u64,
        ));
        self.stream = None;

        if let Ok(mut v) = self.vad.lock() {
            // Flush any remaining speech chunk
            if v.speech_in_chunk && !is_silence_only(&v.chunk_samples) {
                let samples = normalize_samples(v.chunk_samples.clone());
                let _ = self.event_tx.try_send(AudioEvent::Chunk(AudioChunk {
                    samples,
                    sample_rate: v.sample_rate,
                    pause_before_secs: v.last_silence_secs,
                }));
            }
            v.chunk_samples.clear();
            v.recording = false;
        }

        let _ = self.event_tx.try_send(AudioEvent::RecordingStopped);
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    vad: Arc<Mutex<VadState>>,
    tx: Sender<AudioEvent>,
) -> anyhow::Result<cpal::Stream>
where
    T: cpal::Sample + cpal::SizedSample + Into<f32> + Send + 'static,
{
    let err_tx = tx.clone();
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _| {
            let samples: Vec<f32> = data.iter().map(|s| (*s).into()).collect();
            process_samples(samples, &vad, &tx);
        },
        move |err| {
            log::error!("Audio stream error: {}", err);
            let _ = err_tx.try_send(AudioEvent::RecordingStopped);
        },
        None,
    )?;
    Ok(stream)
}

fn process_samples(samples: Vec<f32>, vad: &Arc<Mutex<VadState>>, tx: &Sender<AudioEvent>) {
    let mut v = match vad.lock() {
        Ok(v) => v,
        Err(_) => return,
    };

    if !v.recording {
        return;
    }

    let rms = compute_rms(&samples);
    let threshold = v.adaptive_threshold();
    let is_speech = rms > threshold;

    // Update session peak for adaptive threshold
    if rms > v.session_peak {
        v.session_peak = rms;
    }

    // Send level update for waveform UI (downsample to one per callback)
    let _ = tx.try_send(AudioEvent::LevelUpdate(rms));

    v.chunk_samples.extend_from_slice(&samples);

    if is_speech {
        v.speech_in_chunk = true;
        v.silence_samples = 0;
    } else {
        v.silence_samples += samples.len();
    }

    let chunk_secs = v.chunk_secs();
    let silence_secs = v.silence_secs();

    // Force-cut on max duration
    let force_cut = chunk_secs >= MAX_CHUNK_SECS;
    // Natural cut on silence after speech
    let natural_cut =
        v.speech_in_chunk && silence_secs >= SILENCE_CUT_SECS && chunk_secs >= MIN_CHUNK_SECS;

    if force_cut || natural_cut {
        if v.speech_in_chunk && !is_silence_only(&v.chunk_samples) {
            let chunk = normalize_samples(v.chunk_samples.clone());
            let sr = v.sample_rate;
            let pause = v.last_silence_secs;
            let _ = tx.try_send(AudioEvent::Chunk(AudioChunk {
                samples: chunk,
                sample_rate: sr,
                pause_before_secs: pause,
            }));
            v.last_silence_secs = silence_secs;
        }
        v.chunk_samples.clear();
        v.silence_samples = 0;
        v.speech_in_chunk = false;
    }
}

fn compute_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = samples.iter().map(|s| s * s).sum();
    (sum / samples.len() as f32).sqrt()
}

fn normalize_samples(mut samples: Vec<f32>) -> Vec<f32> {
    let peak = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    if peak < 0.001 {
        return samples;
    }
    let gain = (0.95 / peak).min(50.0);
    for s in samples.iter_mut() {
        *s *= gain;
    }
    samples
}

fn is_silence_only(samples: &[f32]) -> bool {
    compute_rms(samples) < 0.0005
}
