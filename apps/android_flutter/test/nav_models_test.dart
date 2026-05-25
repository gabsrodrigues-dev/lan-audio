import 'package:flutter_test/flutter_test.dart';
import 'package:lan_audio_android/models/nav_models.dart';

void main() {
  group('NavTab', () {
    test('has exactly 3 values', () {
      expect(NavTab.values.length, 3);
    });

    test('indices match expected order', () {
      expect(NavTab.play.index, 0);
      expect(NavTab.connect.index, 1);
      expect(NavTab.settings.index, 2);
    });
  });

  group('SignalQuality', () {
    test('has exactly 4 values', () {
      expect(SignalQuality.values.length, 4);
    });

    test('contains good, moderate, poor, unknown', () {
      expect(SignalQuality.values, contains(SignalQuality.good));
      expect(SignalQuality.values, contains(SignalQuality.moderate));
      expect(SignalQuality.values, contains(SignalQuality.poor));
      expect(SignalQuality.values, contains(SignalQuality.unknown));
    });
  });

  group('ServerCardData', () {
    ServerCardData makeCard({int? latencyMs}) => ServerCardData(
          serverId: 'test-id',
          serverName: 'Test Server',
          host: '192.168.1.100',
          wsPort: 39991,
          udpPort: 39992,
          latencyMs: latencyMs,
          source: 'mdns',
        );

    test('signalQuality returns unknown when latencyMs is null', () {
      final card = makeCard(latencyMs: null);
      expect(card.signalQuality, SignalQuality.unknown);
    });

    test('signalQuality returns good when latencyMs < 30', () {
      expect(makeCard(latencyMs: 0).signalQuality, SignalQuality.good);
      expect(makeCard(latencyMs: 15).signalQuality, SignalQuality.good);
      expect(makeCard(latencyMs: 29).signalQuality, SignalQuality.good);
    });

    test('signalQuality returns moderate when 30 <= latencyMs <= 100', () {
      expect(makeCard(latencyMs: 30).signalQuality, SignalQuality.moderate);
      expect(makeCard(latencyMs: 65).signalQuality, SignalQuality.moderate);
      expect(makeCard(latencyMs: 100).signalQuality, SignalQuality.moderate);
    });

    test('signalQuality returns poor when latencyMs > 100', () {
      expect(makeCard(latencyMs: 101).signalQuality, SignalQuality.poor);
      expect(makeCard(latencyMs: 500).signalQuality, SignalQuality.poor);
      expect(makeCard(latencyMs: 9999).signalQuality, SignalQuality.poor);
    });

    test('equality works correctly', () {
      final a = makeCard(latencyMs: 50);
      final b = makeCard(latencyMs: 50);
      expect(a, equals(b));
      expect(a.hashCode, equals(b.hashCode));
    });

    test('inequality when fields differ', () {
      final a = makeCard(latencyMs: 50);
      final b = makeCard(latencyMs: 100);
      expect(a, isNot(equals(b)));
    });

    test('isRecent defaults to false', () {
      final card = makeCard();
      expect(card.isRecent, false);
    });

    test('toString contains key info', () {
      final card = makeCard(latencyMs: 42);
      expect(card.toString(), contains('Test Server'));
      expect(card.toString(), contains('192.168.1.100'));
      expect(card.toString(), contains('39991'));
    });
  });
}
