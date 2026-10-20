use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Public state snapshot for UI readout
#[derive(Debug, Clone, Default)]
pub struct ReverseChannelState {
    pub mic_active: bool,
    pub mic_peak_db: f32,
    pub mic_rms_db: f32,
    pub mic_device_name: String,
    pub android_volume_pct: u8,
    pub pending_volume_command: Option<u8>,
    pub virtual_device_detected: bool,
    pub virtual_device_warning: Option<String>,
}

/// Check if a virtual audio device is available on the system.
/// On Windows, warn by default and clear when a named pipe consumer connects.
/// On non-Windows, skip the check (no virtual device needed).
fn check_virtual_audio_device(state: &Arc<Mutex<ReverseChannelState>>) {
    #[cfg(windows)]
    {
        if let Ok(mut s) = state.lock() {
            s.virtual_device_detected = false;
            s.virtual_device_warning = Some(
                "Mic input requires a virtual audio device (e.g. VB-Cable). Named pipe fallback is active — check your audio settings.".into(),
            );
        }
        info!(
            "Virtual audio device check: warning set (cleared when named pipe consumer connects)"
        );
    }
    #[cfg(not(windows))]
    {
        if let Ok(mut s) = state.lock() {
            s.virtual_device_detected = true;
            s.virtual_device_warning = None;
        }
    }
}

/// Server that accepts reverse-channel audio (port 7878) and control (port 7879) connections
pub struct ReverseChannelServer {
    reverse_port: u16,
    control_port: u16,
    audio_shutdown_tx: Option<broadcast::Sender<()>>,
    control_shutdown_tx: Option<broadcast::Sender<()>>,
    audio_task: Option<JoinHandle<()>>,
    control_task: Option<JoinHandle<()>>,
    pub state: Arc<Mutex<ReverseChannelState>>,
}

impl Default for ReverseChannelServer {
    fn default() -> Self {
        Self::new()
    }
}

impl ReverseChannelServer {
    pub fn new() -> Self {
        Self {
            reverse_port: 7878,
            control_port: 7879,
            audio_shutdown_tx: None,
            control_shutdown_tx: None,
            audio_task: None,
            control_task: None,
            state: Arc::new(Mutex::new(ReverseChannelState::default())),
        }
    }

    /// Start both listeners. Returns Ok even if a listener fails to bind (best-effort).
    pub async fn start(&mut self) -> anyhow::Result<()> {
        check_virtual_audio_device(&self.state);

        let (audio_tx, audio_rx) = broadcast::channel::<()>(1);
        let (control_tx, control_rx) = broadcast::channel::<()>(1);
        self.audio_shutdown_tx = Some(audio_tx);
        self.control_shutdown_tx = Some(control_tx);

        let state = Arc::clone(&self.state);

        // Audio listener (port 7878)
        let reverse_bind: SocketAddr = format!("0.0.0.0:{}", self.reverse_port).parse()?;
        match TcpListener::bind(reverse_bind).await {
            Ok(listener) => {
                info!("Reverse audio listener bound on port {}", self.reverse_port);
                let state_clone = Arc::clone(&state);
                self.audio_task = Some(tokio::spawn(async move {
                    if let Err(e) = run_reverse_audio(listener, state_clone, audio_rx).await {
                        error!("Reverse audio task exited: {}", e);
                    }
                }));
            }
            Err(e) => {
                warn!(
                    "Could not bind reverse audio port {}: {}",
                    self.reverse_port, e
                );
            }
        }

        // Control listener (port 7879)
        let control_bind: SocketAddr = format!("0.0.0.0:{}", self.control_port).parse()?;
        match TcpListener::bind(control_bind).await {
            Ok(listener) => {
                info!(
                    "Reverse control listener bound on port {}",
                    self.control_port
                );
                let state_clone = Arc::clone(&state);
                self.control_task = Some(tokio::spawn(async move {
                    if let Err(e) = run_control_listener(listener, state_clone, control_rx).await {
                        error!("Reverse control task exited: {}", e);
                    }
                }));
            }
            Err(e) => {
                warn!(
                    "Could not bind reverse control port {}: {}",
                    self.control_port, e
                );
            }
        }

        Ok(())
    }

    /// Signal all tasks to stop and abort them
    pub fn stop(&mut self) {
        if let Some(tx) = self.audio_shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(tx) = self.control_shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.audio_task.take() {
            handle.abort();
        }
        if let Some(handle) = self.control_task.take() {
            handle.abort();
        }
        if let Ok(mut s) = self.state.lock() {
            s.mic_active = false;
        }
    }
}

// ---- audio receiver implementation ----

async fn run_reverse_audio(
    listener: TcpListener,
    state: Arc<Mutex<ReverseChannelState>>,
    mut shutdown: broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                info!("Reverse audio receiver shutting down");
                break;
            }
            result = listener.accept() => {
                match result {
                    Ok((stream, peer)) => {
                        info!("Reverse audio client connected from {}", peer);
                        if let Ok(mut s) = state.lock() {
                            s.mic_active = true;
                        }
                        let state_for_handler = Arc::clone(&state);
                        let state_for_cleanup = Arc::clone(&state);
                        let mut shutdown_clone = shutdown.resubscribe();
                        tokio::spawn(async move {
                            if let Err(e) = handle_audio_client(stream, state_for_handler, &mut shutdown_clone)
                                .await
                            {
                                error!("Reverse audio client error ({}): {}", peer, e);
                            }
                            if let Ok(mut s) = state_for_cleanup.lock() {
                                s.mic_active = false;
                            }
                        });
                    }
                    Err(e) => {
                        error!("Reverse audio accept error: {}", e);
                    }
                }
            }
        }
    }
    Ok(())
}

async fn handle_audio_client(
    mut stream: TcpStream,
    state: Arc<Mutex<ReverseChannelState>>,
    shutdown: &mut broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    let mut opus_decoder = opus::Decoder::new(48000, opus::Channels::Mono)?;
    let mut len_buf = [0u8; 4];
    let mut frame_buf = vec![0u8; 4096];
    let mut pcm_buf = vec![0i16; 480 * 120]; // 120ms of mono 48kHz PCM

    // Open named pipe for virtual audio device
    #[cfg(windows)]
    let pipe_path = r"\\.\pipe\lan_audio_reverse_mic";

    // On Linux, expose the phone mic as a PulseAudio/PipeWire virtual source
    // ("LAN Audio Mic") fed through `pacat`.
    #[cfg(target_os = "linux")]
    let mut linux_mic = match linux_virtual_mic::LinuxVirtualMic::open() {
        Ok(mic) => {
            info!(
                "Reverse mic available as PulseAudio source `{}`",
                linux_virtual_mic::SOURCE_NAME
            );
            if let Ok(mut s) = state.lock() {
                s.virtual_device_detected = true;
                s.virtual_device_warning = None;
                s.mic_device_name = "LAN Audio Mic".into();
            }
            Some(mic)
        }
        Err(e) => {
            warn!("Could not create Linux virtual mic: {e}");
            if let Ok(mut s) = state.lock() {
                s.virtual_device_detected = false;
                s.virtual_device_warning = Some(format!(
                    "Linux virtual mic unavailable ({e}). Install pulseaudio-utils (pactl/pacat)."
                ));
            }
            None
        }
    };

    #[cfg(windows)]
    let mut named_pipe: Option<std::fs::File> = {
        match std::fs::OpenOptions::new().write(true).open(pipe_path) {
            Ok(file) => {
                info!("Opened named pipe for reverse mic: {}", pipe_path);
                if let Ok(mut s) = state.lock() {
                    s.virtual_device_detected = true;
                }
                Some(file)
            }
            Err(e) => {
                warn!(
                    "Could not open named pipe {}: {} (virtual audio device may not be connected)",
                    pipe_path, e
                );
                if let Ok(mut s) = state.lock() {
                    s.virtual_device_detected = false;
                    s.virtual_device_warning = Some(
                        "Mic input requires a virtual audio device (e.g. VB-Cable). Named pipe fallback is active — check your audio settings.".into()
                    );
                }
                None
            }
        }
    };

    #[cfg(not(windows))]
    #[allow(unused)]
    let named_pipe: Option<std::fs::File> = None;

    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                break;
            }
            result = stream.read_exact(&mut len_buf) => {
                if let Err(e) = result {
                    warn!("Reverse audio read error (header): {}", e);
                    break;
                }
                let payload_len = u32::from_le_bytes(len_buf) as usize;
                if payload_len == 0 || payload_len > frame_buf.len() {
                    warn!("Invalid reverse audio frame length: {}", payload_len);
                    continue;
                }
                if let Err(e) = stream.read_exact(&mut frame_buf[..payload_len]).await {
                    warn!("Reverse audio read error (payload): {}", e);
                    break;
                }

                match opus_decoder.decode(&frame_buf[..payload_len], &mut pcm_buf, false) {
                    Ok(samples) => {
                        // Compute peak and RMS
                        let mut peak = 0.0f32;
                        let mut sum_sq = 0.0f64;
                        for &s in &pcm_buf[..samples] {
                            let f = s as f32 / 32768.0;
                            let abs = f.abs();
                            if abs > peak { peak = abs; }
                            sum_sq += (f as f64) * (f as f64);
                        }
                        let rms = if samples > 0 {
                            (sum_sq / samples as f64).sqrt() as f32
                        } else {
                            0.0
                        };
                        let peak_db = if peak > 0.0 { 20.0 * peak.log10() } else { -96.0 };
                        let rms_db = if rms > 0.0 { 20.0 * rms.log10() } else { -96.0 };

                        if let Ok(mut s) = state.lock() {
                            s.mic_peak_db = peak_db;
                            s.mic_rms_db = rms_db;
                        }

                        // Write to named pipe using bytemuck for safe transmute to bytes
                        let pcm_bytes: &[u8] = bytemuck::cast_slice(&pcm_buf[..samples]);

                        #[cfg(target_os = "linux")]
                        if let Some(mic) = linux_mic.as_mut() {
                            if let Err(e) = mic.write(pcm_bytes) {
                                warn!("Linux virtual mic write failed: {e}");
                                linux_mic = None;
                            }
                        }
                        #[cfg(not(any(windows, target_os = "linux")))]
                        let _ = pcm_bytes;

                        #[cfg(windows)]
                        if let Some(ref mut pipe) = named_pipe {
                            use std::io::Write;
                            match pipe.write_all(pcm_bytes) {
                                Ok(()) => {
                                    // Clear virtual device warning on first successful write
                                    if let Ok(mut s) = state.lock() {
                                        if s.virtual_device_warning.is_some() {
                                            s.virtual_device_detected = true;
                                            s.virtual_device_warning = None;
                                            info!("Named pipe consumer detected — virtual audio device warning cleared");
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to write to named pipe: {}", e);
                                }
                            }
                            let _ = pipe.flush();
                        }
                    }
                    Err(ref e) if e.code() == opus::ErrorCode::BufferTooSmall => {
                        warn!("Opus decode buffer too small for reverse channel");
                    }
                    Err(e) => {
                        warn!("Opus decode error on reverse channel: {}", e);
                    }
                }
            }
        }
    }

    Ok(())
}

async fn run_control_listener(
    listener: TcpListener,
    state: Arc<Mutex<ReverseChannelState>>,
    mut shutdown: broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                info!("Reverse control listener shutting down");
                break;
            }
            result = listener.accept() => {
                match result {
                    Ok((stream, peer)) => {
                        info!("Reverse control client connected from {}", peer);
                        let state_clone = Arc::clone(&state);
                        let mut shutdown_clone = shutdown.resubscribe();
                        tokio::spawn(async move {
                            if let Err(e) = handle_control_stream(stream, state_clone, &mut shutdown_clone).await {
                                warn!("Reverse control stream ended ({}): {}", peer, e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Reverse control accept error: {}", e);
                    }
                }
            }
        }
    }
    Ok(())
}

async fn handle_control_stream(
    stream: TcpStream,
    state: Arc<Mutex<ReverseChannelState>>,
    shutdown: &mut broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let buf_reader = BufReader::new(reader);
    let mut lines = buf_reader.lines();

    // Flush any pending outgoing commands (e.g. volume change from desktop)
    let pending = state
        .lock()
        .ok()
        .and_then(|mut s| s.pending_volume_command.take());
    if let Some(pct) = pending {
        let cmd = serde_json::json!({"type": "set_volume", "volume_pct": pct});
        if let Ok(json) = serde_json::to_string(&cmd) {
            match writer.write_all(format!("{}\n", json).as_bytes()).await {
                Ok(()) => {
                    if let Ok(mut s) = state.lock() {
                        s.android_volume_pct = pct;
                    }
                    info!("Sent volume command to Android: {}%", pct);
                }
                Err(e) => {
                    warn!("Failed to send volume command to Android: {}", e);
                }
            }
        }
    }

    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                break;
            }
            line_result = lines.next_line() => {
                match line_result {
                    Ok(Some(line)) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<serde_json::Value>(trimmed) {
                            Ok(msg) => {
                                let msg_type = msg["type"].as_str().unwrap_or("");
                                match msg_type {
                                    "mic_gain" => {
                                        if let Some(gain) = msg["value"].as_f64() {
                                            debug!("Received mic_gain: {}", gain);
                                        }
                                    }
                                    "volume" => {
                                        if let Some(vol) = msg["value"].as_f64() {
                                            let pct = (vol.clamp(0.0, 1.0) * 100.0).round() as u8;
                                            debug!("Remote Android volume changed: {}%", pct);
                                            if let Ok(mut s) = state.lock() {
                                                s.android_volume_pct = pct;
                                            }
                                        }
                                    }
                                    "mic_device" => {
                                        if let Some(name) = msg["name"].as_str() {
                                            debug!("Android mic device: {}", name);
                                            if let Ok(mut s) = state.lock() {
                                                s.mic_device_name = name.to_string();
                                            }
                                        }
                                    }
                                    "mic_level" => {
                                        let peak = msg["peak_db"].as_f64().map(|v| v as f32).unwrap_or(-96.0);
                                        let rms = msg["rms_db"].as_f64().map(|v| v as f32).unwrap_or(-96.0);
                                        if let Ok(mut s) = state.lock() {
                                            s.mic_peak_db = peak;
                                            s.mic_rms_db = rms;
                                        }
                                    }
                                    "mic_active" => {
                                        let active = msg["active"].as_bool().unwrap_or(false);
                                        if let Ok(mut s) = state.lock() {
                                            s.mic_active = active;
                                        }
                                    }
                                    "volume_changed" => {
                                        if let Some(pct) = msg["volume_pct"].as_u64() {
                                            debug!("Android reported volume change: {}%", pct);
                                            if let Ok(mut s) = state.lock() {
                                                s.android_volume_pct = pct as u8;
                                            }
                                        }
                                    }
                                    "ping" => {
                                        // Keep-alive, no response needed
                                    }
                                    other => {
                                        debug!("Unknown control message type: {}", other);
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Invalid control JSON: {} — raw: {}", trimmed, e);
                            }
                        }
                    }
                    Ok(None) => {
                        debug!("Control stream EOF");
                        break;
                    }
                    Err(e) => {
                        warn!("Control read error: {}", e);
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(target_os = "linux")]
mod linux_virtual_mic {
    //! Phone-mic → Linux virtual microphone bridge.
    //!
    //! Creates a null sink (`lan_audio_mic_sink`) plus a remapped source
    //! (`lan_audio_mic`, shown as "LAN Audio Mic") with `pactl`, then pipes
    //! decoded 48 kHz mono s16le PCM into the sink through `pacat`. Apps then
    //! pick "LAN Audio Mic" as their input. Modules are unloaded on drop.
    use std::io::Write;
    use std::process::{Child, ChildStdin, Command, Stdio};

    pub const SINK_NAME: &str = "lan_audio_mic_sink";
    pub const SOURCE_NAME: &str = "lan_audio_mic";

    pub struct LinuxVirtualMic {
        modules: Vec<String>,
        child: Child,
        stdin: ChildStdin,
    }

    fn pactl(args: &[&str]) -> anyhow::Result<String> {
        let out = Command::new("pactl")
            .args(args)
            .stderr(Stdio::piped())
            .output()?;
        if !out.status.success() {
            anyhow::bail!(
                "pactl {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    fn exists(kind: &str, name: &str) -> bool {
        pactl(&["list", "short", kind])
            .map(|list| {
                list.lines()
                    .any(|l| l.split_whitespace().nth(1) == Some(name))
            })
            .unwrap_or(false)
    }

    impl LinuxVirtualMic {
        pub fn open() -> anyhow::Result<Self> {
            let mut modules = Vec::new();
            if !exists("sinks", SINK_NAME) {
                modules.push(pactl(&[
                    "load-module",
                    "module-null-sink",
                    &format!("sink_name={SINK_NAME}"),
                    "rate=48000",
                    "channels=1",
                    "sink_properties=device.description=LAN_Audio_Mic_Sink",
                ])?);
            }
            if !exists("sources", SOURCE_NAME) {
                match pactl(&[
                    "load-module",
                    "module-remap-source",
                    &format!("master={SINK_NAME}.monitor"),
                    &format!("source_name={SOURCE_NAME}"),
                    "source_properties=device.description=LAN_Audio_Mic",
                ]) {
                    Ok(id) => modules.push(id),
                    Err(e) => {
                        for m in &modules {
                            let _ = pactl(&["unload-module", m]);
                        }
                        return Err(e);
                    }
                }
            }
            let mut child = Command::new("pacat")
                .args([
                    "--playback",
                    "--raw",
                    "--format=s16le",
                    "--rate=48000",
                    "--channels=1",
                    "--latency-msec=20",
                    "--client-name=LAN Audio",
                    "--stream-name=LAN Audio phone mic",
                    &format!("--device={SINK_NAME}"),
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| {
                    for m in &modules {
                        let _ = pactl(&["unload-module", m]);
                    }
                    anyhow::anyhow!("spawn pacat: {e}")
                })?;
            let stdin = child
                .stdin
                .take()
                .ok_or_else(|| anyhow::anyhow!("pacat stdin unavailable"))?;
            Ok(Self {
                modules,
                child,
                stdin,
            })
        }

        pub fn write(&mut self, pcm: &[u8]) -> std::io::Result<()> {
            self.stdin.write_all(pcm)
        }
    }

    impl Drop for LinuxVirtualMic {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            for m in self.modules.iter().rev() {
                let _ = pactl(&["unload-module", m]);
            }
        }
    }
}
