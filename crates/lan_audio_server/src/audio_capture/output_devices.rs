//! Playback outputs and their exact PulseAudio monitor names.

use serde::Serialize;

use super::CaptureError;

#[derive(Debug, Clone, Serialize)]
pub struct AudioOutputDevice {
    pub sink_name: String,
    pub source_name: String,
    pub label: String,
    pub is_default: bool,
    pub is_playing: bool,
}

pub fn list_audio_outputs() -> Result<Vec<AudioOutputDevice>, CaptureError> {
    #[cfg(target_os = "linux")]
    {
        let run = |args: &[&str]| -> Result<String, CaptureError> {
            let result = std::process::Command::new("pactl")
                .args(args)
                .output()
                .map_err(|e| CaptureError::DeviceActivationFailed(format!("pactl: {e}")))?;
            if !result.status.success() {
                return Err(CaptureError::DeviceActivationFailed(
                    String::from_utf8_lossy(&result.stderr).trim().to_string(),
                ));
            }
            Ok(String::from_utf8_lossy(&result.stdout).into_owned())
        };
        let default_sink = run(&["get-default-sink"])?;
        parse_outputs(
            &run(&["--format=json", "list", "sinks"])?,
            default_sink.trim(),
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        Ok(Vec::new())
    }
}

#[cfg(any(target_os = "linux", test))]
fn parse_outputs(json: &str, default_sink: &str) -> Result<Vec<AudioOutputDevice>, CaptureError> {
    let sinks: Vec<serde_json::Value> = serde_json::from_str(json).map_err(|e| {
        CaptureError::DeviceActivationFailed(format!("cannot read PulseAudio outputs: {e}"))
    })?;
    let valid_text = |text: &str| !text.trim().is_empty() && text != "(null)";
    Ok(sinks
        .into_iter()
        .filter_map(|sink| {
            let name = sink["name"].as_str()?;
            // This output carries the phone microphone, not desktop playback.
            if name == "lan_audio_mic_sink" {
                return None;
            }
            let source_name = sink["monitor_source"].as_str().filter(|s| valid_text(s))?;
            let description = [
                sink["description"].as_str(),
                sink["properties"]["node.nick"].as_str(),
                sink["properties"]["alsa.name"].as_str(),
            ]
            .into_iter()
            .flatten()
            .find(|text| valid_text(text))
            .unwrap_or(name);
            let active_port = sink["active_port"].as_str();
            let port_description = sink["ports"].as_array().and_then(|ports| {
                ports
                    .iter()
                    .find(|port| port["name"].as_str() == active_port)
                    .and_then(|port| port["description"].as_str())
                    .filter(|text| valid_text(text))
            });
            let label = match port_description {
                Some(port) => format!("{description} — {port}"),
                None => description.to_string(),
            };
            Some(AudioOutputDevice {
                sink_name: name.to_string(),
                source_name: source_name.to_string(),
                label,
                is_default: name == default_sink,
                is_playing: sink["state"].as_str() == Some("RUNNING"),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_exact_monitor_and_headphone_port_with_unicode_label() {
        let outputs = parse_outputs(r#"[
            {"name":"headset","monitor_source":"actual-monitor-name","description":"Fone USB",
             "state":"RUNNING","active_port":"headphones","ports":[{"name":"headphones","description":"Saída de fones"}]},
            {"name":"hdmi","monitor_source":"hdmi.monitor","description":"(null)","properties":{"node.nick":"LG FHD"}},
            {"name":"lan_audio_mic_sink","monitor_source":"lan_audio_mic_sink.monitor"}
        ]"#, "headset").unwrap();
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].source_name, "actual-monitor-name");
        assert_eq!(outputs[0].label, "Fone USB — Saída de fones");
        assert!(outputs[0].is_default && outputs[0].is_playing);
        assert_eq!(outputs[1].label, "LG FHD");
        assert!(!outputs[1].is_default);
    }
}
