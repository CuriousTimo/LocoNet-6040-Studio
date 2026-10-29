//! `KeyMap.h` for the Arduino IDE: the firmware loads it with the program (see the
//! firmware README, "Another way: write it down in a file").

use crate::model::{Key, Layout};

/// The whole `KeyMap.h`. Every Keyboard of the layout gets a line, automatic ones with the
/// turnouts they switch, so the file sets up the Nano as the layout shows it. The options
/// are not part of KeyMap.h; they stay as they are on the Nano.
pub fn to_keymap_h(layout: &Layout) -> String {
    let mut out = String::new();
    out.push_str(HEADER);
    if layout.keyboards.is_empty() {
        out.push_str("// The layout has no Keyboards: the key map is not used.\n");
        out.push_str("const bool kUseKeyMap = false;\n\n");
        out.push_str(STRUCT);
        out.push_str("const KeyMapLine kKeyMap[] PROGMEM = {\n");
        out.push_str("    {1, {1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16}},\n");
    } else {
        out.push_str("const bool kUseKeyMap = true;\n\n");
        out.push_str(STRUCT);
        out.push_str("const KeyMapLine kKeyMap[] PROGMEM = {\n");
        for (index, keyboard) in layout.keyboards.iter().enumerate() {
            if index > 0 {
                out.push('\n');
            }
            out.push_str(&format!("    // Keyboard {}", keyboard.number));
            if !keyboard.name.is_empty() {
                out.push_str(&format!(": {}", keyboard.name));
            }
            out.push('\n');
            for (pair, name) in keyboard.key_names.iter().enumerate() {
                if !name.is_empty() {
                    out.push_str(&format!("    //   key pair {}: {name}\n", pair + 1));
                }
            }
            let entries: Vec<String> = keyboard
                .effective_keys()
                .iter()
                .map(|key| match key {
                    Key::Nothing => "0".to_owned(),
                    Key::Turnout(number) => number.to_string(),
                    Key::Sensor(number) => format!("SENSOR({number})"),
                })
                .collect();
            out.push_str(&format!(
                "    {{{}, {{{}}}}},\n",
                keyboard.number,
                entries.join(", ")
            ));
        }
    }
    out.push_str("};\n\n#endif\n");
    out
}

const HEADER: &str = "\
// Key map: what every key pair does, written by LocoNet 6040 Studio. Load it onto the Nano
// together with the Marklin-6040-LocoNet program; see its README, \"Another way: write it
// down in a file\". The options are not in this file; they stay as they are on the Nano.
//
// One line per Keyboard: its number (1 to 16, as in Märklin's switch table), then its 16
// key pairs in the order printed on the Keyboard (1 to 8 on top, 9 to 16 below):
// a turnout (1 to 2048), 0 for nothing, or SENSOR(33) for a track section (1 to 4096).

#ifndef KEY_MAP_H
#define KEY_MAP_H

#include <Arduino.h>

";

const STRUCT: &str = "\
#define SENSOR(number) (0x8000u | (number))

struct KeyMapLine {
  uint8_t keyboard;
  uint16_t keys[16];
};

";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Layout;

    #[test]
    fn writes_one_line_per_keyboard() {
        let mut layout = Layout::default();
        layout.keyboard_mut(1);
        let station = layout.keyboard_mut(2);
        station.automatic = false;
        station.name = "Adorf".to_owned();
        station.keys = std::array::from_fn(|pair| Key::Turnout(101 + pair as u16));
        station.keys[9] = Key::Nothing;
        station.keys[10] = Key::Sensor(33);
        station.key_names[10] = "Track 1".to_owned();
        let text = to_keymap_h(&layout);
        assert!(text.contains("const bool kUseKeyMap = true;"));
        assert!(
            text.contains("    {1, {1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16}},\n")
        );
        assert!(text.contains("    // Keyboard 2: Adorf\n    //   key pair 11: Track 1\n"));
        assert!(text.contains(
            "    {2, {101, 102, 103, 104, 105, 106, 107, 108, 109, 0, SENSOR(33), 112, 113, 114, 115, 116}},\n"
        ));
        assert!(text.ends_with("};\n\n#endif\n"));
    }

    #[test]
    fn an_empty_layout_switches_the_key_map_off() {
        let text = to_keymap_h(&Layout::default());
        assert!(text.contains("const bool kUseKeyMap = false;"));
        assert!(
            text.contains("kKeyMap[] PROGMEM = {\n    {1, {"),
            "never an empty array"
        );
    }
}
