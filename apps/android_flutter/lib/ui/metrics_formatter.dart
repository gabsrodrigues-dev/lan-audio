/// Pure formatting functions for real-time metrics display.
///
/// These functions are extracted from PlayPage to enable independent
/// property-based testing (see design Property 1 and Property 2).

/// Formats the real-time metrics line for the Play tab.
///
/// Returns a string in the format:
///   "Buf: {buffer}ms | Underrun: {underrun} | RTT: {rtt}ms | Loss: {loss}%"
///
/// When [rttMs] is null, displays "-" for RTT.
/// When [lossPercent] is null, displays "-" for loss.
String formatMetricsLine({
  required int bufferMs,
  required int underrunCount,
  required int? rttMs,
  required double? lossPercent,
}) {
  final rttStr = rttMs != null ? '${rttMs}ms' : '-';
  final lossStr =
      lossPercent != null ? '${lossPercent.toStringAsFixed(1)}%' : '-';
  return 'Buf: ${bufferMs}ms | Underrun: $underrunCount | RTT: $rttStr | Loss: $lossStr';
}

/// Formats the audio quality strip text.
///
/// Returns a display-friendly codec label, sample rate in kHz, and channel
/// configuration string.
///
/// [codec] — negotiated codec name (e.g. "opus", "pcm16").
/// [sampleRate] — sample rate in Hz (e.g. 48000).
/// [channels] — channel count (1 = Mono, 2 = Stereo, N = "N ch").
/// [isZh] — whether to use Chinese labels for channel config.
({String codec, String sampleRate, String channels}) formatAudioQualityStrip({
  required String codec,
  required int sampleRate,
  required int channels,
  bool isZh = false,
}) {
  return (
    codec: _codecLabel(codec),
    sampleRate: _sampleRateLabel(sampleRate),
    channels: _channelsLabel(channels, isZh),
  );
}

String _codecLabel(String codec) {
  switch (codec.toLowerCase()) {
    case 'opus':
      return 'Opus';
    case 'pcm16':
      return 'PCM 16';
    case 'f32':
      return 'PCM Float';
    default:
      return codec.toUpperCase();
  }
}

String _sampleRateLabel(int sampleRate) {
  if (sampleRate >= 1000) {
    final khz = sampleRate / 1000.0;
    // Drop trailing .0 for whole-kHz rates (48000 -> "48kHz").
    if (khz == khz.truncateToDouble()) {
      return '${khz.toInt()}kHz';
    }
    return '${khz.toStringAsFixed(1)}kHz';
  }
  return '${sampleRate}Hz';
}

String _channelsLabel(int channels, bool isZh) {
  switch (channels) {
    case 1:
      return isZh ? '单声道' : 'Mono';
    case 2:
      return isZh ? '立体声' : 'Stereo';
    default:
      return isZh ? '$channels 声道' : '$channels ch';
  }
}
