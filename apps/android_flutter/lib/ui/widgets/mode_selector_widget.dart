import 'package:flutter/material.dart';

import '../../widgets/capsule_button.dart';
import '../audio_console_theme.dart';

class ModeSelectorItem {
  const ModeSelectorItem({
    required this.id,
    required this.name,
    required this.desc,
  });

  final String id;
  final String name;
  final String desc;
}

/// A horizontal row of 4 [CapsuleButton] widgets for selecting the audio mode.
///
/// Selected mode: teal background, dark text.
/// Unselected mode: transparent background, gray border, white text.
///
/// The mode selector remains enabled when disconnected to allow pre-selection.
class ModeSelectorWidget extends StatelessWidget {
  const ModeSelectorWidget({
    required this.items,
    required this.selectedId,
    required this.enabled,
    required this.onSelected,
    super.key,
  });

  final List<ModeSelectorItem> items;
  final String selectedId;
  final bool enabled;
  final ValueChanged<String> onSelected;

  @override
  Widget build(BuildContext context) {
    return Opacity(
      opacity: enabled ? 1.0 : 0.5,
      child: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: items.map((item) {
          final isLast = item == items.last;
          return Padding(
            padding: EdgeInsets.only(
              right: isLast ? 0 : AudioConsoleSpacing.xs,
            ),
            child: CapsuleButton(
              key: Key('mode_button_${item.id}'),
              label: item.name,
              selected: item.id == selectedId,
              onTap: enabled ? () => onSelected(item.id) : () {},
            ),
          );
        }).toList(),
      ),
    );
  }
}
