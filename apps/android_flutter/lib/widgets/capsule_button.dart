import 'package:flutter/material.dart';

import '../ui/audio_console_theme.dart';

/// A pill-shaped toggle button used for multi-option selectors (mode, codec, language, etc.).
///
/// Renders in two visual states:
/// - **Selected**: solid teal background with dark text.
/// - **Unselected**: transparent background, gray border, white text.
///
/// Uses [AudioConsoleRadius.pill] and [AudioConsoleColors] tokens exclusively.
class CapsuleButton extends StatelessWidget {
  const CapsuleButton({
    super.key,
    required this.label,
    required this.selected,
    required this.onTap,
  });

  final String label;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
        decoration: BoxDecoration(
          color: selected ? AudioConsoleColors.teal : Colors.transparent,
          borderRadius: AudioConsoleRadius.pill,
          border: selected
              ? null
              : Border.all(color: AudioConsoleColors.text3.withValues(alpha: 0.3)),
        ),
        child: Text(
          label,
          style: TextStyle(
            color: selected ? AudioConsoleColors.bg : AudioConsoleColors.text,
            fontWeight: FontWeight.w500,
            fontSize: 13,
          ),
        ),
      ),
    );
  }
}
