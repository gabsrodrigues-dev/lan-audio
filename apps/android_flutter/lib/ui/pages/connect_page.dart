import 'package:flutter/material.dart';

import '../../connect_history.dart';
import '../../widgets/capsule_button.dart';
import '../../widgets/server_card.dart';
import '../audio_console_theme.dart';
import 'more_page.dart';

/// ConnectPage widget for the Connect tab (index 1).
///
/// Displays a connection mode selector (Auto Discovery, Manual Input, USB)
/// and switches content based on the selected mode.
class ConnectPage extends StatelessWidget {
  const ConnectPage({
    super.key,
    required this.isZh,
    required this.connectMode,
    required this.onConnectModeChanged,
    required this.isConnecting,
    required this.probeRunning,
    required this.nsdDiscoveryRunning,
    required this.discoveryTimedOut,
    required this.manualHostController,
    required this.servers,
    required this.selectedServerId,
    required this.onServerSelected,
    required this.isRecentHost,
    required this.onConnectSelected,
    required this.onConnectManual,
    required this.onConnectUsb,
    required this.onScanLan,
    required this.wsConnected,
    required this.connectHistory,
  });

  final bool isZh;

  // Connection mode: 0 = auto discovery, 1 = manual input, 2 = USB
  final int connectMode;
  final ValueChanged<int> onConnectModeChanged;

  final bool isConnecting;
  final bool probeRunning;
  final bool nsdDiscoveryRunning;
  final bool discoveryTimedOut;
  final TextEditingController manualHostController;
  final List<MoreServerData> servers;
  final String? selectedServerId;
  final ValueChanged<String> onServerSelected;
  final bool Function(String host) isRecentHost;
  final VoidCallback onConnectSelected;
  final VoidCallback onConnectManual;
  final VoidCallback onConnectUsb;
  final VoidCallback onScanLan;
  final bool wsConnected;
  final List<ConnectHistoryEntry> connectHistory;

  String tr(String zh, String en) => isZh ? zh : en;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: AudioConsoleColors.bg,
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(16, 20, 16, 16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              // ─── Title ───
              Text(
                tr('连接到服务器', 'Connect to Server'),
                style: AudioConsoleType.title().copyWith(
                      fontWeight: FontWeight.bold,
                    ),
              ),
              const SizedBox(height: 20),

              // ─── 3-segment connection mode selector ───
              _buildModeSelector(),
              const SizedBox(height: 20),

              // ─── Content area based on selected mode ───
              Expanded(
                child: _buildModeContent(),
              ),
            ],
          ),
        ),
      ),
    );
  }

  /// Builds the 3-segment connection mode selector using CapsuleButton widgets.
  Widget _buildModeSelector() {
    final modes = <_ModeOption>[
      _ModeOption(label: tr('自动发现', 'Auto Discovery'), value: 0),
      _ModeOption(label: tr('手动输入', 'Manual Input'), value: 1),
      _ModeOption(label: tr('USB 连接', 'USB Connection'), value: 2),
    ];

    return Row(
      children: modes.map((mode) {
        return Padding(
          padding: EdgeInsets.only(right: mode.value < 2 ? 8.0 : 0),
          child: CapsuleButton(
            label: mode.label,
            selected: connectMode == mode.value,
            onTap: () => onConnectModeChanged(mode.value),
          ),
        );
      }).toList(),
    );
  }

  /// Builds the content area that switches based on the selected connection mode.
  Widget _buildModeContent() {
    switch (connectMode) {
      case 0:
        return _buildAutoDiscoveryContent();
      case 1:
        return _buildManualInputContent();
      case 2:
        return _buildUsbContent();
      default:
        return _buildAutoDiscoveryContent();
    }
  }

  /// Full implementation of Auto Discovery mode content.
  ///
  /// Displays:
  /// 1. Full-width teal "Scan Servers" button with search icon
  /// 2. Loading indicator when scanning
  /// 3. "Available Servers" section with ServerCard list
  /// 4. "Recent Connections" section with history badge
  /// 5. Hint when no servers found and discovery timed out
  Widget _buildAutoDiscoveryContent() {
    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // ─── Scan Button ───
          SizedBox(
            width: double.infinity,
            child: GestureDetector(
              onTap: onScanLan,
              child: Container(
                padding: const EdgeInsets.symmetric(
                  vertical: AudioConsoleSpacing.md,
                ),
                decoration: BoxDecoration(
                  color: AudioConsoleColors.teal,
                  borderRadius: AudioConsoleRadius.button,
                ),
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    const Icon(
                      Icons.search,
                      color: AudioConsoleColors.bg,
                      size: 20,
                    ),
                    const SizedBox(width: AudioConsoleSpacing.xs),
                    Text(
                      tr('扫描服务器', 'Scan Servers'),
                      style: AudioConsoleType.buttonLabel(
                        color: AudioConsoleColors.bg,
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),

          // ─── Loading Indicator ───
          if (probeRunning || nsdDiscoveryRunning) ...[
            const SizedBox(height: AudioConsoleSpacing.lg),
            const Center(
              child: SizedBox(
                width: 24,
                height: 24,
                child: CircularProgressIndicator(
                  strokeWidth: 2.5,
                  valueColor: AlwaysStoppedAnimation<Color>(
                    AudioConsoleColors.teal,
                  ),
                ),
              ),
            ),
          ],

          const SizedBox(height: AudioConsoleSpacing.xl),

          // ─── Available Servers Section ───
          Text(
            tr('可用服务器', 'AVAILABLE SERVERS'),
            style: AudioConsoleType.caption(
              color: AudioConsoleColors.text2,
            ).copyWith(letterSpacing: 1.2),
          ),
          const SizedBox(height: AudioConsoleSpacing.md),

          if (servers.isNotEmpty)
            ...servers.map((server) => Padding(
                  padding: const EdgeInsets.only(
                    bottom: AudioConsoleSpacing.xs,
                  ),
                  child: ServerCard(
                    serverName: server.serverName,
                    host: server.host,
                    port: server.wsPort,
                    connectionType: server.latencyMs != null ? 'mDNS' : 'UDP',
                    latencyMs: server.latencyMs,
                    isSelected: server.serverId == selectedServerId,
                    isRecent: isRecentHost(server.host),
                    onTap: () => onServerSelected(server.serverId),
                  ),
                )),

          // ─── No servers hint ───
          if (servers.isEmpty && discoveryTimedOut)
            Padding(
              padding: const EdgeInsets.symmetric(
                vertical: AudioConsoleSpacing.lg,
              ),
              child: Center(
                child: Text(
                  tr(
                    '未发现服务器，请尝试手动输入或 USB 连接',
                    'No servers found. Try manual input or USB connection.',
                  ),
                  style: AudioConsoleType.body(
                    color: AudioConsoleColors.text3,
                  ),
                  textAlign: TextAlign.center,
                ),
              ),
            ),

          const SizedBox(height: AudioConsoleSpacing.xl),

          // ─── Recent Connections Section ───
          if (connectHistory.isNotEmpty) ...[
            Row(
              children: [
                Text(
                  tr('最近连接', 'RECENT CONNECTIONS'),
                  style: AudioConsoleType.caption(
                    color: AudioConsoleColors.text2,
                  ).copyWith(letterSpacing: 1.2),
                ),
                const SizedBox(width: AudioConsoleSpacing.xs),
                Container(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 6,
                    vertical: 2,
                  ),
                  decoration: BoxDecoration(
                    color: AudioConsoleColors.tealDim,
                    borderRadius: AudioConsoleRadius.chip,
                  ),
                  child: Text(
                    '${connectHistory.length}',
                    style: const TextStyle(
                      fontFamily: AudioConsoleType.monoFamily,
                      fontSize: 10,
                      fontWeight: FontWeight.w600,
                      color: AudioConsoleColors.teal,
                    ),
                  ),
                ),
              ],
            ),
            const SizedBox(height: AudioConsoleSpacing.md),
            ...connectHistory.map((entry) => Padding(
                  padding: const EdgeInsets.only(
                    bottom: AudioConsoleSpacing.xs,
                  ),
                  child: ServerCard(
                    serverName: entry.hostname,
                    host: entry.ip,
                    port: entry.port,
                    connectionType: 'history',
                    latencyMs: entry.lastLatencyMs > 0
                        ? entry.lastLatencyMs
                        : null,
                    isSelected: false,
                    isRecent: true,
                    onTap: () => onServerSelected(
                      '${entry.ip}:${entry.port}',
                    ),
                  ),
                )),
          ],
        ],
      ),
    );
  }

  /// Manual Input mode: text field for IPv4 address + connect button.
  Widget _buildManualInputContent() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // Section label
        Text(
          tr('服务器地址', 'Server Address'),
          style: AudioConsoleType.body(color: AudioConsoleColors.text2),
        ),
        const SizedBox(height: 12),

        // IPv4 address text field with dark background and monospace font
        Container(
          decoration: BoxDecoration(
            color: AudioConsoleColors.surface,
            borderRadius: AudioConsoleRadius.card,
            border: Border.all(color: AudioConsoleColors.borderStrong),
          ),
          child: TextField(
            controller: manualHostController,
            style: AudioConsoleType.monoValue(color: AudioConsoleColors.text),
            decoration: InputDecoration(
              hintText: tr('例如 192.168.1.23', 'e.g. 192.168.1.23'),
              hintStyle: AudioConsoleType.monoMeta(),
              labelText: tr('IPv4 地址', 'IPv4 Address'),
              labelStyle: AudioConsoleType.body(color: AudioConsoleColors.text2),
              border: InputBorder.none,
              contentPadding: const EdgeInsets.symmetric(
                horizontal: 16,
                vertical: 14,
              ),
              prefixIcon: const Icon(
                Icons.dns_outlined,
                color: AudioConsoleColors.text2,
              ),
            ),
            keyboardType: TextInputType.number,
            enabled: !isConnecting,
          ),
        ),
        const SizedBox(height: 16),

        // Connect button
        SizedBox(
          width: double.infinity,
          child: ElevatedButton(
            onPressed: isConnecting ? null : onConnectManual,
            style: ElevatedButton.styleFrom(
              backgroundColor: AudioConsoleColors.teal,
              foregroundColor: AudioConsoleColors.bg,
              disabledBackgroundColor: AudioConsoleColors.tealDim,
              padding: const EdgeInsets.symmetric(vertical: 14),
              shape: RoundedRectangleBorder(
                borderRadius: AudioConsoleRadius.button,
              ),
            ),
            child: isConnecting
                ? Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      const SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(
                          strokeWidth: 2,
                          color: AudioConsoleColors.bg,
                        ),
                      ),
                      const SizedBox(width: 8),
                      Text(
                        tr('连接中...', 'Connecting...'),
                        style: AudioConsoleType.buttonLabel(
                          color: AudioConsoleColors.bg,
                        ),
                      ),
                    ],
                  )
                : Text(
                    tr('连接', 'Connect'),
                    style: AudioConsoleType.buttonLabel(
                      color: AudioConsoleColors.bg,
                    ),
                  ),
          ),
        ),

        // Hint when auto-discovery times out
        if (discoveryTimedOut) ...[
          const SizedBox(height: 20),
          Container(
            width: double.infinity,
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              color: AudioConsoleColors.surface,
              borderRadius: AudioConsoleRadius.card,
              border: Border.all(color: AudioConsoleColors.amber.withValues(alpha: 0.3)),
            ),
            child: Row(
              children: [
                const Icon(
                  Icons.info_outline,
                  size: 18,
                  color: AudioConsoleColors.amber,
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    tr(
                      '自动发现未找到服务器，请输入 IP 地址手动连接。',
                      'Auto-discovery found no servers. Enter IP address to connect manually.',
                    ),
                    style: AudioConsoleType.body(color: AudioConsoleColors.text2),
                  ),
                ),
              ],
            ),
          ),
        ],
      ],
    );
  }

  /// USB Connection mode: USB device status display + connect button.
  Widget _buildUsbContent() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // USB device status card
        Container(
          width: double.infinity,
          padding: const EdgeInsets.all(16),
          decoration: BoxDecoration(
            color: AudioConsoleColors.surface,
            borderRadius: AudioConsoleRadius.card,
            border: Border.all(color: AudioConsoleColors.borderStrong),
          ),
          child: Row(
            children: [
              Icon(
                Icons.usb,
                size: 32,
                color: wsConnected
                    ? AudioConsoleColors.teal
                    : AudioConsoleColors.text2,
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      tr('USB 设备状态', 'USB Device Status'),
                      style: AudioConsoleType.body(
                        color: AudioConsoleColors.text,
                      ),
                    ),
                    const SizedBox(height: 4),
                    Text(
                      wsConnected
                          ? tr('已连接 (127.0.0.1)', 'Connected (127.0.0.1)')
                          : tr('未连接', 'Not Connected'),
                      style: AudioConsoleType.monoMeta(
                        color: wsConnected
                            ? AudioConsoleColors.teal
                            : AudioConsoleColors.text2,
                      ),
                    ),
                  ],
                ),
              ),
              // Status indicator dot
              Container(
                width: 10,
                height: 10,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: wsConnected
                      ? AudioConsoleColors.teal
                      : AudioConsoleColors.text3,
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 12),

        // USB connection info
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 4),
          child: Text(
            tr(
              '通过 USB 调试桥 (ADB) 连接到电脑端服务器。',
              'Connect to PC server via USB Debug Bridge (ADB).',
            ),
            style: AudioConsoleType.body(color: AudioConsoleColors.text2),
          ),
        ),
        const SizedBox(height: 16),

        // Connect button
        SizedBox(
          width: double.infinity,
          child: ElevatedButton(
            onPressed: isConnecting ? null : onConnectUsb,
            style: ElevatedButton.styleFrom(
              backgroundColor: AudioConsoleColors.teal,
              foregroundColor: AudioConsoleColors.bg,
              disabledBackgroundColor: AudioConsoleColors.tealDim,
              padding: const EdgeInsets.symmetric(vertical: 14),
              shape: RoundedRectangleBorder(
                borderRadius: AudioConsoleRadius.button,
              ),
            ),
            child: isConnecting
                ? Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      const SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(
                          strokeWidth: 2,
                          color: AudioConsoleColors.bg,
                        ),
                      ),
                      const SizedBox(width: 8),
                      Text(
                        tr('连接中...', 'Connecting...'),
                        style: AudioConsoleType.buttonLabel(
                          color: AudioConsoleColors.bg,
                        ),
                      ),
                    ],
                  )
                : Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      const Icon(Icons.usb, size: 18),
                      const SizedBox(width: 8),
                      Text(
                        tr('USB 连接', 'Connect via USB'),
                        style: AudioConsoleType.buttonLabel(
                          color: AudioConsoleColors.bg,
                        ),
                      ),
                    ],
                  ),
          ),
        ),

        // Hint when auto-discovery times out
        if (discoveryTimedOut) ...[
          const SizedBox(height: 20),
          Container(
            width: double.infinity,
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              color: AudioConsoleColors.surface,
              borderRadius: AudioConsoleRadius.card,
              border: Border.all(color: AudioConsoleColors.amber.withValues(alpha: 0.3)),
            ),
            child: Row(
              children: [
                const Icon(
                  Icons.info_outline,
                  size: 18,
                  color: AudioConsoleColors.amber,
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    tr(
                      '自动发现未找到服务器，请尝试 USB 连接。',
                      'Auto-discovery found no servers. Try USB connection.',
                    ),
                    style: AudioConsoleType.body(color: AudioConsoleColors.text2),
                  ),
                ),
              ],
            ),
          ),
        ],
      ],
    );
  }
}

/// Internal helper for mode option data.
class _ModeOption {
  const _ModeOption({required this.label, required this.value});
  final String label;
  final int value;
}
