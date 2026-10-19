//! Linux system-audio loopback capture.
//!
//! Captures the monitor of the default output sink through `parec`
//! (from `pulseaudio-utils`). This works on both PulseAudio and PipeWire
//! (via `pipewire-pulse`), which covers practically every modern Linux
//! desktop, without linking against any native audio library.
//!
//! `parec` is asked to deliver raw `float32le` interleaved PCM at the target
//! sample rate/channel count, so the server-side frame format matches the
//! Windows WASAPI path (48 kHz / stereo / f32) and the rest of the pipeline
//! (encoder, packetizer, sync engine) is unchanged.
//!
//! Environment overrides:
//! - `LAN_AUDIO_LINUX_SOURCE`: PulseAudio source name to record from
//!   (default `@DEFAULT_MONITOR@`, i.e. "what you hear"). Use
//!   `pactl list short sources` to list alternatives.
//! - `LAN_AUDIO_PAREC`: path to the `parec` binary (default: `parec` in PATH).

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio::time::{timeout_at, Instant};
use tracing::{debug, info, warn};

use super::pcm_accumulator::PcmFrameAccumulator;
use super::{
    AudioCaptureSource, AudioFormat, AudioFrame, CaptureDebugDumpConfig, CaptureError,
    CaptureSourceState, PacketKind,
};

/// Peak level below which a captured chunk is treated as digital silence.
const SILENCE_PEAK_THRESHOLD: f32 = 1.0e-5;
/// Max PCM chunks queued between the reader thread and the async consumer.
/// Each chunk is roughly one frame, so this caps queued latency.
const CHANNEL_CAPACITY: usize = 32;
/// If the accumulator holds more than this many frames, the oldest audio is
/// dropped to keep end-to-end latency bounded.
const MAX_BUFFERED_FRAMES: usize = 8;

pub const DEFAULT_LINUX_SOURCE: &str = "@DEFAULT_MONITOR@";

pub struct LinuxLoopbackCapture {
    target_format: AudioFormat,
    source: String,
    parec_bin: String,
    state: CaptureSourceState,
    child: Option<Child>,
    rx: Option<mpsc::Receiver<Vec<f32>>>,
    accumulator: PcmFrameAccumulator,
    device_name: Option<String>,
}

impl std::fmt::Debug for LinuxLoopbackCapture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinuxLoopbackCapture")
            .field("source", &self.source)
            .field("state", &self.state)
            .finish()
    }
}

impl LinuxLoopbackCapture {
    /// Captures the monitor of the default output sink.
    pub fn new_default_output(
        format: AudioFormat,
        _debug_cfg: CaptureDebugDumpConfig,
    ) -> Result<Self, CaptureError> {
        let source = std::env::var("LAN_AUDIO_LINUX_SOURCE")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_LINUX_SOURCE.to_string());
        let parec_bin = std::env::var("LAN_AUDIO_PAREC")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "parec".to_string());

        // Fail fast (so the service can fall back to synthetic) when the
        // tool is missing instead of failing later inside start().
        let probe = Command::new(&parec_bin)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if !matches!(probe, Ok(status) if status.success()) {
            return Err(CaptureError::DeviceActivationFailed(format!(
                "`{parec_bin}` not found or not runnable; install `pulseaudio-utils` \
                 (works with PulseAudio and PipeWire via pipewire-pulse)"
            )));
        }

        Ok(Self {
            target_format: format,
            source,
            parec_bin,
            state: CaptureSourceState::Created,
            child: None,
            rx: None,
            accumulator: PcmFrameAccumulator::new(format),
            device_name: None,
        })
    }

    fn resolve_device_name(&self) -> Option<String> {
        if self.source != DEFAULT_LINUX_SOURCE {
            return Some(self.source.clone());
        }
        let out = Command::new("pactl")
            .arg("get-default-sink")
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let sink = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if sink.is_empty() {
            None
        } else {
            Some(format!("{sink}.monitor"))
        }
    }

    fn spawn_parec(&mut self) -> Result<(), CaptureError> {
        let fmt = self.target_format;
        let latency_ms = u32::from(fmt.frame_duration_ms).max(5);
        let mut child = Command::new(&self.parec_bin)
            .arg(format!("--device={}", self.source))
            .arg("--format=float32le")
            .arg(format!("--rate={}", fmt.sample_rate_hz))
            .arg(format!("--channels={}", fmt.channels))
            .arg(format!("--latency-msec={latency_ms}"))
            .arg("--client-name=LAN Audio")
            .arg("--stream-name=LAN Audio loopback")
            .arg("--raw")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| CaptureError::StartFailed(format!("spawn parec: {e}")))?;

        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| CaptureError::StartFailed("parec stdout unavailable".into()))?;
        if let Some(mut stderr) = child.stderr.take() {
            std::thread::Builder::new()
                .name("lan-audio-parec-stderr".into())
                .spawn(move || {
                    let mut buf = String::new();
                    let _ = stderr.read_to_string(&mut buf);
                    let msg = buf.trim();
                    if !msg.is_empty() {
                        warn!(stderr = %msg, "parec reported an error");
                    }
                })
                .ok();
        }

        let (tx, rx) = mpsc::channel::<Vec<f32>>(CHANNEL_CAPACITY);
        let chunk_bytes = fmt.total_samples_per_frame() * std::mem::size_of::<f32>();
        std::thread::Builder::new()
            .name("lan-audio-parec-reader".into())
            .spawn(move || {
                let mut buf = vec![0u8; chunk_bytes.max(256)];
                let mut pending: Vec<u8> = Vec::with_capacity(chunk_bytes * 2);
                loop {
                    let n = match stdout.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(e) => {
                            warn!(error = %e, "parec read failed");
                            break;
                        }
                    };
                    pending.extend_from_slice(&buf[..n]);
                    let whole = pending.len() / 4 * 4;
                    if whole == 0 {
                        continue;
                    }
                    let samples: Vec<f32> = pending[..whole]
                        .chunks_exact(4)
                        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                        .collect();
                    pending.drain(..whole);
                    match tx.try_send(samples) {
                        Ok(()) => {}
                        Err(mpsc::error::TrySendError::Full(_)) => {
                            // Consumer is behind; drop this chunk to keep latency bounded.
                            debug!("linux loopback queue full, dropping chunk");
                        }
                        Err(mpsc::error::TrySendError::Closed(_)) => break,
                    }
                }
                debug!("parec reader thread exited");
            })
            .map_err(|e| CaptureError::StartFailed(format!("spawn reader thread: {e}")))?;

        self.child = Some(child);
        self.rx = Some(rx);
        Ok(())
    }

    fn push_chunk(&mut self, chunk: Vec<f32>) {
        let peak = chunk.iter().fold(0.0_f32, |m, s| m.max(s.abs()));
        self.accumulator
            .push_samples(&chunk, peak < SILENCE_PEAK_THRESHOLD);
        let frame = self.target_format.total_samples_per_frame();
        let max = frame * MAX_BUFFERED_FRAMES;
        if self.accumulator.buffered_samples() > max {
            // Drop whole frames from the front until we're back under budget.
            while self.accumulator.buffered_samples() > frame * 2 {
                if self.accumulator.pop_frame().is_none() {
                    break;
                }
            }
            debug!("linux loopback accumulator overflow, dropped stale audio");
        }
    }

    fn kill_child(&mut self) {
        self.rx = None;
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[async_trait]
impl AudioCaptureSource for LinuxLoopbackCapture {
    async fn start(&mut self) -> Result<(), CaptureError> {
        self.device_name = self.resolve_device_name();
        self.state = CaptureSourceState::DeviceResolved;
        if let Err(e) = self.spawn_parec() {
            self.state = CaptureSourceState::Failed;
            return Err(e);
        }
        self.state = CaptureSourceState::Started;
        info!(
            source = %self.source,
            device = ?self.device_name,
            rate = self.target_format.sample_rate_hz,
            channels = self.target_format.channels,
            frame_ms = self.target_format.frame_duration_ms,
            "linux loopback capture started (parec)"
        );
        Ok(())
    }

    async fn read_frame(&mut self) -> Result<AudioFrame, CaptureError> {
        if self.state != CaptureSourceState::Started {
            return Err(CaptureError::NotStarted);
        }

        let deadline = Instant::now()
            + Duration::from_millis(u64::from(self.target_format.frame_duration_ms) + 2);
        let mut saw_packet = false;
        loop {
            if let Some(acc) = self.accumulator.pop_frame() {
                return self.accumulator.into_audio_frame(now_ms(), acc, None);
            }
            let Some(rx) = self.rx.as_mut() else {
                return Err(CaptureError::NotStarted);
            };
            match timeout_at(deadline, rx.recv()).await {
                Ok(Some(chunk)) => {
                    saw_packet = true;
                    self.push_chunk(chunk);
                }
                Ok(None) => {
                    // parec exited (e.g. audio server restarted).
                    self.state = CaptureSourceState::Failed;
                    self.kill_child();
                    return Err(CaptureError::ReadBufferFailed(
                        "parec stream ended unexpectedly".into(),
                    ));
                }
                Err(_) => break,
            }
        }

        // Nothing (or not enough) arrived within one frame: emit silence so the
        // downstream pacing keeps running.
        let mut frame = AudioFrame::silence(now_ms(), self.target_format);
        frame.packet_kind = if saw_packet {
            PacketKind::SilentPacket
        } else {
            PacketKind::NoPacket
        };
        Ok(frame)
    }

    async fn stop(&mut self) -> Result<(), CaptureError> {
        self.kill_child();
        self.state = CaptureSourceState::Stopped;
        Ok(())
    }

    fn format(&self) -> AudioFormat {
        self.target_format
    }

    fn state(&self) -> CaptureSourceState {
        self.state
    }

    fn source_name(&self) -> &'static str {
        "linux_loopback"
    }

    fn device_name(&self) -> Option<String> {
        self.device_name.clone()
    }
}

impl Drop for LinuxLoopbackCapture {
    fn drop(&mut self) {
        self.kill_child();
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_parec_binary_is_reported() {
        std::env::set_var("LAN_AUDIO_PAREC", "/nonexistent/parec-lan-audio-test");
        let err = LinuxLoopbackCapture::new_default_output(
            AudioFormat::default(),
            CaptureDebugDumpConfig {
                enabled: false,
                seconds: 1,
                output_dir: "x".into(),
            },
        )
        .expect_err("must fail without parec");
        std::env::remove_var("LAN_AUDIO_PAREC");
        assert!(matches!(err, CaptureError::DeviceActivationFailed(_)));
    }
}
