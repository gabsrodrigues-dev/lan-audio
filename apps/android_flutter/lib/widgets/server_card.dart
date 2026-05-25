import 'package:flutter/material.dart';

import '../models/nav_models.dart';
import '../ui/audio_console_theme.dart';

/// A dark-surface card displaying server discovery information with signal
/// quality indicator. Used in the Connect tab's server list.
///
/// Layout:
/// ┌─────────────────────────────────────────────────┐
/// │  [wifi]  ServerName (bold)          [●] signal  │
/// │          192.168.1.1:39991 · mDNS   (colored)   │
/// └─────────────────────────────────────────────────┘
class ServerCard extends StatelessWidget {
  const ServerCard({
    super.key,
    required this.serverName,
    required this.host,
    required this.port,
    required this.connectionType,
    required this.latencyMs,
    required this.isSelected,
    required this.isRecent,
    required this.onTap,
  });

  /// Human-readable server name (e.g. "My PC").
  final String serverName;

  /// IPv4 address of the server.
  final String host;

  /// WebSocket port number.
  final int port;

  /// Discovery source label (e.g. 'mDNS', 'UDP', 'probe').
  final String connectionType;

  /// Measured round-trip latency in milliseconds, or null if unknown.
  final int? latencyMs;

  /// Whether this card is currently selected/highlighted.
  final bool isSelected;

  /// Whether this server appears in the user's recent connection history.
  final bool isRecent;

  /// Callback when the card is tapped.
  final VoidCallback onTap;

  /// Returns the [SignalQuality] classification for the current [latencyMs].
  SignalQuality get signalQuality {
    if (latencyMs == null) return SignalQuality.unknown;
    if (latencyMs! < 30) return SignalQuality.good;
    if (latencyMs! <= 100) return SignalQuality.moderate;
    return SignalQuality.poor;
  }

  /// Maps [SignalQuality] to the corresponding design-system color.
  ///
  /// - good (RTT < 30ms): teal / green
  /// - moderate (30–100ms): amber / yellow
  /// - poor (> 100ms): error / red
  /// - unknown: muted text color
  Color _signalColor() {
    switch (signalQuality) {
      case SignalQuality.good:
        return AudioConsoleColors.teal;
      case SignalQuality.moderate:
        return AudioConsoleColors.amber;
      case SignalQuality.poor:
        return AudioConsoleColors.error;
      case SignalQuality.unknown:
        return AudioConsoleColors.text3;
    }
  }

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap,
      child: Container(
        constraints: const BoxConstraints(
          minHeight: AudioConsoleSizes.serverCardMinHeight,
        ),
        padding: const EdgeInsets.symmetric(
          horizontal: AudioConsoleSpacing.lg,
          vertical: AudioConsoleSpacing.md,
        ),
        decoration: BoxDecoration(
          color: AudioConsoleColors.surface,
          borderRadius: AudioConsoleRadius.card,
          border: isSelected
              ? Border.all(color: AudioConsoleColors.teal, width: 1.5)
              : Border.all(color: AudioConsoleColors.border, width: 1),
        ),
        child: Row(
          children: [
            // Left: wifi icon
            Icon(
              Icons.wifi,
              color: isSelected
                  ? AudioConsoleColors.teal
                  : AudioConsoleColors.text2,
              size: 22,
            ),
            const SizedBox(width: AudioConsoleSpacing.md),

            // Center: server name + IP:port + connection type
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Text(
                    serverName,
                    style: const TextStyle(
                      fontFamily: AudioConsoleType.sansFamily,
                      fontSize: 14,
                      fontWeight: FontWeight.w600,
                      color: AudioConsoleColors.text,
                    ),
                    overflow: TextOverflow.ellipsis,
                    maxLines: 1,
                  ),
                  const SizedBox(height: 2),
                  Text(
                    '$host:$port · $connectionType',
                    style: const TextStyle(
                      fontFamily: AudioConsoleType.monoFamily,
                      fontSize: 12,
                      color: AudioConsoleColors.text2,
                    ),
                    overflow: TextOverflow.ellipsis,
                    maxLines: 1,
                  ),
                ],
              ),
            ),

            const SizedBox(width: AudioConsoleSpacing.xs),

            // Right: signal strength indicator (colored dot)
            _SignalIndicator(color: _signalColor(), latencyMs: latencyMs),
          ],
        ),
      ),
    );
  }
}

/// A small colored dot with optional latency label indicating signal quality.
class _SignalIndicator extends StatelessWidget {
  const _SignalIndicator({
    required this.color,
    required this.latencyMs,
  });

  final Color color;
  final int? latencyMs;

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        Container(
          width: 10,
          height: 10,
          decoration: BoxDecoration(
            color: color,
            shape: BoxShape.circle,
          ),
        ),
        if (latencyMs != null) ...[
          const SizedBox(height: 2),
          Text(
            '${latencyMs}ms',
            style: TextStyle(
              fontFamily: AudioConsoleType.monoFamily,
              fontSize: 9,
              color: color,
            ),
          ),
        ],
      ],
    );
  }
}
