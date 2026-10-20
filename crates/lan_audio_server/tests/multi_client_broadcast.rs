use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use lan_audio_protocol::{
    AudioMode, ControlMessageV2, Hello, HelloAck, ProtocolCapabilities, PROTOCOL_VERSION_V2,
};
use lan_audio_server::config::{AudioSourceKind, ServerConfig, TransportMode};
use lan_audio_server::service::LanAudioService;
use tokio::net::UdpSocket;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream};
use uuid::Uuid;

type WsStream = WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct TestClient {
    ws: WsStream,
    udp: UdpSocket,
}

#[cfg(target_os = "linux")]
#[tokio::test]
#[ignore = "requires a running PulseAudio/PipeWire session and pulseaudio-utils"]
async fn linux_capture_failure_stops_service_instead_of_accepting_silent_connections() {
    let cfg = ServerConfig {
        audio_source: AudioSourceKind::LinuxLoopback,
        audio_source_fallback_to_synthetic: false,
        // Invalid capture format: parec must reject it after spawning.
        channels: 0,
        ws_bind: format!("127.0.0.1:{}", pick_free_tcp_port())
            .parse()
            .unwrap(),
        udp_bind: "127.0.0.1:0".parse().unwrap(),
        discovery_bind: "127.0.0.1:0".parse().unwrap(),
        discovery_broadcast: "127.0.0.1:39990".parse().unwrap(),
        ..ServerConfig::default()
    };
    let service = LanAudioService::new(cfg).await.unwrap();
    let result = timeout(Duration::from_secs(8), service.run_until_shutdown()).await;
    service.stop();
    let error = result
        .expect("capture failure must reach the caller without waiting for shutdown")
        .expect_err("invalid capture format must fail the service");
    assert!(error.to_string().contains("parec"), "{error}");
    assert_eq!(service.metrics_snapshot().capture_start_failures, 1);
}

/// Exercises the real PulseAudio/PipeWire monitor, encoder, and UDP receiver.
/// Temporarily selects a silent virtual output and restores the original output
/// on drop. Run separately from other tests that use the reverse-channel ports:
/// cargo test -p lan_audio_server --test multi_client_broadcast linux_system_audio -- --ignored --nocapture
#[cfg(target_os = "linux")]
#[tokio::test]
#[ignore = "requires a running PulseAudio/PipeWire session and pulseaudio-utils"]
async fn linux_system_audio_reaches_udp_in_pcm16_and_opus() {
    use lan_audio_protocol::{UdpAudioPacket, UdpAudioPacketV2};
    use lan_audio_server::config::{CodecSelection, DataPlaneFormat};

    for explicit_output in [false, true] {
        let output = LinuxTestOutput::new(!explicit_output);
        let devices =
            lan_audio_server::audio_capture::output_devices::list_audio_outputs().unwrap();
        assert!(devices
            .iter()
            .any(|device| device.source_name == output.source_name));
        if explicit_output {
            assert_ne!(
                LinuxTestOutput::pactl(&["get-default-sink"]),
                output.source_name.trim_end_matches(".monitor")
            );
        }
        for codec in [CodecSelection::Pcm16, CodecSelection::Opus] {
            let cfg = ServerConfig {
                audio_source: AudioSourceKind::LinuxLoopback,
                audio_output_source: explicit_output.then(|| output.source_name.clone()),
                audio_source_fallback_to_synthetic: false,
                codec_selection: codec,
                data_plane_format: if codec == CodecSelection::Pcm16 {
                    DataPlaneFormat::LegacyLas1
                } else {
                    DataPlaneFormat::V2Header
                },
                ws_bind: format!("127.0.0.1:{}", pick_free_tcp_port())
                    .parse()
                    .unwrap(),
                udp_bind: "127.0.0.1:0".parse().unwrap(),
                discovery_bind: "127.0.0.1:0".parse().unwrap(),
                discovery_broadcast: "127.0.0.1:39990".parse().unwrap(),
                ..ServerConfig::default()
            };
            let service = std::sync::Arc::new(LanAudioService::new(cfg.clone()).await.unwrap());
            let runner = {
                let service = service.clone();
                tokio::spawn(async move { service.run_until_shutdown().await })
            };
            let client = connect_client(cfg.ws_bind.port(), "linux-real-monitor").await;
            let mut decoder = opus::Decoder::new(48_000, opus::Channels::Stereo).unwrap();
            let mut received_tone_frames = 0;
            timeout(Duration::from_secs(8), async {
                let mut wire = vec![0; 8192];
                while received_tone_frames < 20 {
                    let n = client.udp.recv(&mut wire).await.unwrap();
                    let samples = if codec == CodecSelection::Pcm16 {
                        UdpAudioPacket::decode(&wire[..n])
                            .unwrap()
                            .payload
                            .chunks_exact(2)
                            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
                            .collect::<Vec<_>>()
                    } else {
                        let packet = UdpAudioPacketV2::decode(&wire[..n]).unwrap();
                        let mut samples = vec![0.0; 5760 * 2];
                        let frames = decoder
                            .decode_float(&packet.payload, &mut samples, false)
                            .unwrap();
                        samples.truncate(frames * 2);
                        samples
                    };
                    let rms =
                        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
                    if rms > 0.05 {
                        // Verify the played tone, allowing Opus startup transients.
                        let mono: Vec<_> = samples.iter().step_by(2).copied().collect();
                        let (re, im) = mono.iter().enumerate().fold(
                            (0.0_f32, 0.0_f32),
                            |(re, im), (i, sample)| {
                                let phase = std::f32::consts::TAU * 440.0 * i as f32 / 48_000.0;
                                (re + sample * phase.cos(), im + sample * phase.sin())
                            },
                        );
                        let tone_energy =
                            2.0 * (re * re + im * im) / (mono.len() * mono.len()) as f32;
                        if tone_energy > rms * rms * 0.5 {
                            received_tone_frames += 1;
                        }
                    }
                }
            })
            .await
            .expect("real system audio must reach the UDP receiver");
            let metrics = service.metrics_snapshot();
            assert_eq!(metrics.current_audio_source, "linux_loopback");
            assert_eq!(metrics.capture_device_name, output.source_name);
            assert!(metrics.capture_non_silent_frames > 0);
            println!("{codec:?}, explicit_output={explicit_output}: decoded {received_tone_frames} real monitor tone frames over UDP");
            service.stop();
            timeout(Duration::from_secs(5), runner)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        }
    }
}

#[cfg(target_os = "linux")]
struct LinuxTestOutput {
    original_sink: String,
    source_name: String,
    module: String,
    player: std::process::Child,
}

#[cfg(target_os = "linux")]
impl LinuxTestOutput {
    fn pactl(args: &[&str]) -> String {
        let out = std::process::Command::new("pactl")
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "pactl {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    fn new(make_default: bool) -> Self {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let original_sink = Self::pactl(&["get-default-sink"]);
        let sink = format!("lan_audio_test_{}", Uuid::new_v4().simple());
        let module = Self::pactl(&[
            "load-module",
            "module-null-sink",
            &format!("sink_name={sink}"),
        ]);
        let mut player = Command::new("pacat")
            .args([
                "--playback",
                "--raw",
                "--format=float32le",
                "--rate=48000",
                "--channels=2",
                &format!("--device={sink}"),
            ])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = player.stdin.take().unwrap();
        let output = Self {
            original_sink,
            source_name: format!("{sink}.monitor"),
            module,
            player,
        };
        Self::pactl(&[
            "set-default-sink",
            if make_default {
                &sink
            } else {
                &output.original_sink
            },
        ]);
        std::thread::spawn(move || {
            let samples: Vec<u8> = (0..48_000)
                .flat_map(|i| {
                    let sample = 0.2 * (std::f32::consts::TAU * 440.0 * i as f32 / 48_000.0).sin();
                    [sample.to_le_bytes(), sample.to_le_bytes()].concat()
                })
                .collect();
            while stdin.write_all(&samples).is_ok() {}
        });
        output
    }
}

#[cfg(target_os = "linux")]
impl Drop for LinuxTestOutput {
    fn drop(&mut self) {
        let _ = self.player.kill();
        let _ = self.player.wait();
        // Cleanup must also run if an assertion fails.
        let _ = std::process::Command::new("pactl")
            .args(["set-default-sink", &self.original_sink])
            .status();
        let _ = std::process::Command::new("pactl")
            .args(["unload-module", &self.module])
            .status();
    }
}

#[tokio::test]
async fn multi_client_broadcast_handles_disconnect_and_rejects_over_limit() {
    let mut cfg = ServerConfig::default();
    cfg.audio_source = AudioSourceKind::Synthetic;
    cfg.transport_mode = TransportMode::WiFi;
    cfg.ws_bind = format!("127.0.0.1:{}", pick_free_tcp_port())
        .parse()
        .expect("ws bind");
    cfg.udp_bind = format!("127.0.0.1:{}", pick_free_udp_port())
        .parse()
        .expect("udp bind");
    cfg.discovery_bind = "127.0.0.1:0".parse().expect("discovery bind");
    cfg.discovery_broadcast = "127.0.0.1:39990".parse().expect("discovery broadcast");

    let service = LanAudioService::new(cfg.clone())
        .await
        .expect("create service");
    let service = std::sync::Arc::new(service);
    let runner = {
        let service = std::sync::Arc::clone(&service);
        tokio::spawn(async move { service.run_until_shutdown().await })
    };

    let mut c1 = connect_client(cfg.ws_bind.port(), "c1").await;
    let c2 = connect_client(cfg.ws_bind.port(), "c2").await;
    let c3 = connect_client(cfg.ws_bind.port(), "c3").await;

    assert_receives_frame(&c1.udp).await;
    assert_receives_frame(&c2.udp).await;
    assert_receives_frame(&c3.udp).await;

    c1.ws
        .close(None)
        .await
        .expect("close first client websocket");
    tokio::time::sleep(Duration::from_millis(150)).await;

    assert_receives_frame(&c2.udp).await;
    assert_receives_frame(&c3.udp).await;

    let mut _c1r = connect_client(cfg.ws_bind.port(), "c1r").await;
    let mut _c4 = connect_client(cfg.ws_bind.port(), "c4").await;

    let mut c5 = connect_raw_client(cfg.ws_bind.port(), "c5").await;
    let ack = wait_for_hello_ack(&mut c5).await;
    assert!(!ack.accepted, "the 5th client must be rejected");
    assert_eq!(ack.message, "too_many_clients");

    service.stop();
    let joined = timeout(Duration::from_secs(5), runner)
        .await
        .expect("service should stop in time");
    let run_result = joined.expect("join ok");
    run_result.expect("service exits cleanly");
}

async fn connect_client(ws_port: u16, name: &str) -> TestClient {
    let udp = UdpSocket::bind("127.0.0.1:0").await.expect("bind udp");
    let udp_port = udp.local_addr().expect("udp addr").port();
    let ws = connect_raw(ws_port)
        .await
        .expect("connect websocket for client");
    let mut ws = ws;
    send_v2_hello(&mut ws, name, udp_port).await;
    let ack = wait_for_hello_ack(&mut ws).await;
    assert!(ack.accepted, "hello_ack should accept client {name}");
    TestClient { ws, udp }
}

async fn connect_raw_client(ws_port: u16, name: &str) -> WsStream {
    let mut ws = connect_raw(ws_port)
        .await
        .expect("connect websocket for raw client");
    send_v2_hello(&mut ws, name, 0).await;
    ws
}

async fn connect_raw(ws_port: u16) -> anyhow::Result<WsStream> {
    let url = format!("ws://127.0.0.1:{ws_port}/");
    for _ in 0..30 {
        match connect_async(&url).await {
            Ok((ws, _)) => return Ok(ws),
            Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }
    Err(anyhow::anyhow!("failed to connect websocket: {url}"))
}

async fn send_v2_hello(ws: &mut WsStream, name: &str, udp_port: u16) {
    let hello = ControlMessageV2::Hello(Hello {
        protocol_version: PROTOCOL_VERSION_V2,
        device_name: name.to_string(),
        client_id: format!("{name}-{}", Uuid::new_v4()),
        udp_port,
        desired_sample_rate: 48_000,
        channels: 2,
        capabilities: test_capabilities(),
        preferred_audio_mode: AudioMode::Balanced,
    });
    ws.send(Message::Text(
        serde_json::to_string(&hello)
            .expect("serialize hello")
            .into(),
    ))
    .await
    .expect("send hello");
}

async fn wait_for_hello_ack(ws: &mut WsStream) -> HelloAck {
    let mut attempts = 0;
    while attempts < 20 {
        attempts += 1;
        let msg = timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("timed out waiting hello_ack")
            .expect("websocket stream ended")
            .expect("websocket message error");
        if let Message::Text(text) = msg {
            if let Ok(ControlMessageV2::HelloAck(ack)) =
                serde_json::from_str::<ControlMessageV2>(text.as_str())
            {
                return ack;
            }
        }
    }
    panic!("did not receive hello_ack in time");
}

async fn assert_receives_frame(udp: &UdpSocket) {
    let mut buf = vec![0_u8; 4096];
    let n = timeout(Duration::from_secs(3), udp.recv(&mut buf))
        .await
        .expect("timed out waiting broadcast frame")
        .expect("udp receive failed");
    assert!(n > 0, "received frame should not be empty");
}

fn pick_free_tcp_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind tcp port")
        .local_addr()
        .expect("local addr")
        .port()
}

fn pick_free_udp_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .expect("bind udp port")
        .local_addr()
        .expect("local addr")
        .port()
}

fn test_capabilities() -> ProtocolCapabilities {
    ProtocolCapabilities {
        supports_pcm16: true,
        supports_f32: false,
        supports_modes: true,
        supports_metrics: true,
        supports_opus_future: true,
        supports_opus: true,
        supports_opus_experimental: true,
        supports_low_latency: true,
        supports_high_quality: true,
        supports_native_audio_track: true,
        supports_fast_path: true,
        supports_stable_audio_track: true,
        supports_usb_tethering: true,
        supports_usb_direct_future: false,
        supports_reverse_channel: false,
        supports_hires_pcm24: false,
        supports_ultra_low_latency: true,
    }
}
