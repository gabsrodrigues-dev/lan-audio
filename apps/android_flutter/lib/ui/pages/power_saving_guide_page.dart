import 'dart:ui';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../power_saving_guide.dart';

class PowerSavingGuidePage extends StatefulWidget {
  const PowerSavingGuidePage({super.key});

  static const routeName = '/power-saving-guide';

  @override
  State<PowerSavingGuidePage> createState() => _PowerSavingGuidePageState();
}

class _PowerSavingGuidePageState extends State<PowerSavingGuidePage> {
  static const MethodChannel _platformChannel =
      MethodChannel('lan_audio/platform');

  String _manufacturer = '';

  @override
  void initState() {
    super.initState();
    _loadManufacturer();
  }

  Future<void> _loadManufacturer() async {
    try {
      final manufacturer =
          await _platformChannel.invokeMethod<String>('getDeviceManufacturer');
      if (mounted) {
        setState(() => _manufacturer = manufacturer ?? '');
      }
    } catch (_) {
      // Keep generic instructions when native platform data is unavailable.
    }
  }

  Future<void> _openDontKillMyApp() async {
    try {
      await _platformChannel.invokeMethod(
          'openExternalUrl', {'url': 'https://dontkillmyapp.com/'});
    } catch (_) {
      // Fallback: silently ignore if platform channel is unavailable.
    }
  }

  @override
  Widget build(BuildContext context) {
    final locale =
        PlatformDispatcher.instance.locale.languageCode.toLowerCase();
    final isZh = locale.startsWith('zh');
    final steps = orderedPowerSavingGuideSteps(_manufacturer);
    return Scaffold(
      appBar: AppBar(
        title: Text(isZh ? '后台播放' : 'Background playback'),
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Text(
            isZh
                ? 'LAN Audio 被省电模式限制时，请按下面步骤允许后台播放。'
                : 'If battery saver limits LAN Audio, allow background playback with the steps below.',
            style: Theme.of(context).textTheme.titleMedium,
          ),
          const SizedBox(height: 12),
          if (_manufacturer.isNotEmpty)
            Text(
              isZh
                  ? '已识别设备品牌：$_manufacturer'
                  : 'Detected manufacturer: $_manufacturer',
              style: const TextStyle(color: Colors.black54),
            ),
          if (_manufacturer.isNotEmpty) const SizedBox(height: 12),
          for (final step in steps) ...[
            ListTile(
              contentPadding: EdgeInsets.zero,
              leading: const Icon(Icons.battery_saver),
              title: Text(step.zh),
              subtitle: Text(step.en),
            ),
            const Divider(height: 1),
          ],
          const SizedBox(height: 16),
          ListTile(
            contentPadding: EdgeInsets.zero,
            leading: const Icon(Icons.link, color: Colors.blueAccent),
            title: Text(
              isZh
                  ? '查看你的手机品牌的详细设置 →'
                  : 'See detailed settings for your phone brand →',
              style: const TextStyle(
                color: Colors.blueAccent,
                fontWeight: FontWeight.w600,
              ),
            ),
            onTap: _openDontKillMyApp,
          ),
        ],
      ),
    );
  }
}
