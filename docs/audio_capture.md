# Audio Capture Layer (Stage 4)

## 目标

Windows loopback raw PCM 可持续产帧，并为 Android 播放链路提供稳定输入。

## 已完成

- `PcmFrameAccumulator`：任意 packet 累积为固定 10ms 帧。
- Windows loopback：持续读包、静音/无包区分、峰值/RMS统计。
- 可选 debug wav 导出（用于证明采集真实有效）。

## 当前输出形态

- 当前网络主路径发送 PCM16 payload。
- loopback 内部维持 f32 处理，发送前最小转换。

## 注意

- 目前支持 mix format 主路径：float32 / i16 PCM。
- 复杂格式与完整重采样仍为后续工作。

## Linux (PulseAudio / PipeWire)

The Linux source records the default output monitor using `parec` from
`pulseaudio-utils`. PipeWire needs its PulseAudio compatibility service running.
Startup waits for PCM before reporting success. Recorder failures reconnect after
one second; startup failures now reach the desktop error display instead of
leaving a connected server with no audio transport.

To check the audio session and available monitors:

```sh
pactl info
pactl list short sources
```

In the desktop **Audio output** selector, choose the headphones, speakers or HDMI
output used by the application playing sound. **Refresh outputs** updates the list
after connecting a device. The choice is saved across app restarts. An unavailable
saved output stays visible rather than silently switching to another device.
The signal indicator shows whether audio is being captured from that output.

For the headless server, use `--audio-output-source <monitor-source-name>`.
`LAN_AUDIO_LINUX_SOURCE` is also available; an explicit desktop/CLI selection
takes precedence over this environment variable.

Validation:

```sh
cargo test --workspace
cargo test -p lan_audio_server --test multi_client_broadcast linux_ -- --ignored --nocapture --test-threads=1
```

The opt-in Linux tests require a live audio session. They play a tone into a
temporary null sink, select it as the default output, decode PCM16 and Opus at a
UDP receiver, and restore the original output on cleanup. They additionally capture
an explicitly selected output while a different output is the system default, and verify that
a rejected capture format fails the service promptly. These tests do not verify
Android hardware playback.

### Linux network connection

The desktop address, QR code and discovery use LAN adapters rather than VPN,
Docker or emulator interfaces. If capture shows **Audio detected** but no phone
is connected, check that the phone uses the PC's LAN address and that the host
firewall allows inbound TCP port 39991 from the local subnet. Audio is sent from
UDP port 39992 to the port negotiated by the Android client.
