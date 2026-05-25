/// Navigation and server card data models for the 3-tab UI redesign.
///
/// Contains:
/// - [NavTab] — the three bottom navigation tabs
/// - [ServerCardData] — structured data for server discovery cards
/// - [SignalQuality] — RTT-based signal quality classification

/// The three bottom navigation tabs in the redesigned UI.
enum NavTab {
  play, // index 0
  connect, // index 1
  settings, // index 2
}

/// Signal quality classification based on RTT latency thresholds.
///
/// - [good]: RTT < 30 ms (green)
/// - [moderate]: 30 ms ≤ RTT ≤ 100 ms (yellow)
/// - [poor]: RTT > 100 ms (red)
/// - [unknown]: no latency measurement available
enum SignalQuality { good, moderate, poor, unknown }

/// Data model for a discovered server card displayed in the Connect tab.
class ServerCardData {
  const ServerCardData({
    required this.serverId,
    required this.serverName,
    required this.host,
    required this.wsPort,
    required this.udpPort,
    this.latencyMs,
    required this.source,
    this.isRecent = false,
  });

  /// Unique identifier for this server instance.
  final String serverId;

  /// Human-readable server name (e.g. "My PC").
  final String serverName;

  /// IPv4 address of the server.
  final String host;

  /// WebSocket port (default 39991).
  final int wsPort;

  /// UDP audio data port (default 39992).
  final int udpPort;

  /// Measured round-trip latency in milliseconds, or null if not yet measured.
  final int? latencyMs;

  /// Discovery source: 'mdns', 'udp', or 'probe'.
  final String source;

  /// Whether this server appears in the user's recent connection history.
  final bool isRecent;

  /// Classifies signal quality based on [latencyMs] thresholds:
  /// - green (<30 ms) → [SignalQuality.good]
  /// - yellow (30–100 ms) → [SignalQuality.moderate]
  /// - red (>100 ms) → [SignalQuality.poor]
  /// - null → [SignalQuality.unknown]
  SignalQuality get signalQuality {
    if (latencyMs == null) return SignalQuality.unknown;
    if (latencyMs! < 30) return SignalQuality.good;
    if (latencyMs! <= 100) return SignalQuality.moderate;
    return SignalQuality.poor;
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is ServerCardData &&
          runtimeType == other.runtimeType &&
          serverId == other.serverId &&
          serverName == other.serverName &&
          host == other.host &&
          wsPort == other.wsPort &&
          udpPort == other.udpPort &&
          latencyMs == other.latencyMs &&
          source == other.source &&
          isRecent == other.isRecent;

  @override
  int get hashCode => Object.hash(
        serverId,
        serverName,
        host,
        wsPort,
        udpPort,
        latencyMs,
        source,
        isRecent,
      );

  @override
  String toString() =>
      'ServerCardData($serverName, $host:$wsPort, latency=${latencyMs}ms, $source)';
}
