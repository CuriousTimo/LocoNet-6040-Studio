//! The layout the Studio edits and saves: the Nano's options and what every key pair of
//! every Keyboard 6040 does, with names that only the Studio keeps.

use serde::{Deserialize, Serialize};

/// Keyboards are numbered 1 to 16, as Märklin does (the Keyboard's manual and the switch
/// table for the four switches on its back: all off is Keyboard 1).
pub const KEYBOARDS: u8 = 16;
/// Key pairs per Keyboard, numbered 1 to 16 from the left.
pub const KEY_PAIRS: usize = 16;
pub const LAST_TURNOUT: u16 = 2048;
pub const LAST_SENSOR: u16 = 4096;

/// The format id and version in a saved layout file.
pub const FORMAT: &str = "loconet-6040-studio";
pub const FORMAT_VERSION: u32 = 1;

/// What one key pair does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Key {
    /// Nothing: the lights still follow the key.
    #[default]
    Nothing,
    /// Switches a LocoNet turnout, 1 to 2048.
    Turnout(u16),
    /// Shows a LocoNet sensor (track section), 1 to 4096: red occupied, green free.
    Sensor(u16),
}

impl Key {
    /// As the firmware writes it in `keys` and `backup`: `12`, `0` or `s33`.
    pub fn to_entry(self) -> String {
        match self {
            Key::Nothing => "0".to_owned(),
            Key::Turnout(number) => number.to_string(),
            Key::Sensor(number) => format!("s{number}"),
        }
    }

    /// Reads `12`, `0` or `s33`; None if it is not a valid entry.
    pub fn from_entry(text: &str) -> Option<Key> {
        let (sensor, digits) = match text.strip_prefix(['s', 'S']) {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let number: u16 = digits.parse().ok()?;
        let key = match (sensor, number) {
            (false, 0) => Key::Nothing,
            (false, _) => Key::Turnout(number),
            (true, _) => Key::Sensor(number),
        };
        key.is_valid().then_some(key)
    }

    pub fn is_valid(self) -> bool {
        match self {
            Key::Nothing => true,
            Key::Turnout(number) => (1..=LAST_TURNOUT).contains(&number),
            Key::Sensor(number) => (1..=LAST_SENSOR).contains(&number),
        }
    }
}

/// One Keyboard 6040.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keyboard {
    /// 1 to 16, as Märklin numbers it and as set with the switches on its back.
    pub number: u8,
    /// Automatic: the firmware's own numbering, Keyboard 1 switches 1 to 16, Keyboard 2 17
    /// to 32. Otherwise `keys`.
    pub automatic: bool,
    pub keys: [Key; KEY_PAIRS],
    /// A name for the whole Keyboard, for example the station it controls.
    #[serde(default)]
    pub name: String,
    /// A name per key pair, for example "Entry signal"; kept by the Studio only.
    #[serde(default)]
    pub key_names: [String; KEY_PAIRS],
}

impl Keyboard {
    /// A Keyboard with the automatic assignment.
    pub fn new(number: u8) -> Self {
        Keyboard {
            number,
            automatic: true,
            keys: Self::automatic_keys(number),
            name: String::new(),
            key_names: Default::default(),
        }
    }

    /// The firmware's automatic assignment: Keyboard n switches (n − 1) × 16 + 1 to n × 16.
    pub fn automatic_keys(number: u8) -> [Key; KEY_PAIRS] {
        let first = u16::from(number.saturating_sub(1)) * 16 + 1;
        std::array::from_fn(|pair| Key::Turnout(first + pair as u16))
    }

    /// What the key pairs do on the Nano: the automatic assignment or the own one.
    pub fn effective_keys(&self) -> [Key; KEY_PAIRS] {
        if self.automatic {
            Self::automatic_keys(self.number)
        } else {
            self.keys
        }
    }
}

/// The Nano's options, as `option` sets them on the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Options {
    /// Keyboards without their own assignment switch 16 turnouts in a row.
    pub auto_assign: bool,
    /// The Märklin bus STOP and GO lines switch LocoNet power.
    pub stop_go: bool,
    /// Keys are not sent while track power is off.
    pub hold_without_power: bool,
    /// Log keys and switch commands on the terminal.
    pub log: bool,
    /// Restart the Nano if it hangs.
    pub watchdog: bool,
    /// How long a turnout coil stays on, 0 to 12750 ms in steps of 50; 0 switches it off
    /// when the key is released.
    pub coil_ms: u16,
}

impl Default for Options {
    /// The firmware's factory settings.
    fn default() -> Self {
        Options {
            auto_assign: true,
            stop_go: false,
            hold_without_power: true,
            log: true,
            watchdog: true,
            coil_ms: 200,
        }
    }
}

/// A saved layout: one Nano with its Keyboards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layout {
    pub format: String,
    pub version: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub options: Options,
    /// The Keyboards, by number; at most one per number.
    #[serde(default)]
    pub keyboards: Vec<Keyboard>,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            format: FORMAT.to_owned(),
            version: FORMAT_VERSION,
            name: String::new(),
            notes: String::new(),
            options: Options::default(),
            keyboards: Vec::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LayoutError {
    #[error("this is not a LocoNet 6040 Studio layout")]
    NotALayout,
    #[error("this layout was saved by a newer LocoNet 6040 Studio (format {0})")]
    Newer(u32),
    #[error("cannot read the layout: {0}")]
    Json(#[from] serde_json::Error),
}

impl Layout {
    pub fn from_json(text: &str) -> Result<Layout, LayoutError> {
        let layout: Layout = serde_json::from_str(text)?;
        if layout.format != FORMAT {
            return Err(LayoutError::NotALayout);
        }
        if layout.version > FORMAT_VERSION {
            return Err(LayoutError::Newer(layout.version));
        }
        Ok(layout)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a layout always serializes")
    }

    pub fn keyboard(&self, number: u8) -> Option<&Keyboard> {
        self.keyboards
            .iter()
            .find(|keyboard| keyboard.number == number)
    }

    /// The Keyboard with this number, added with the automatic assignment if missing.
    pub fn keyboard_mut(&mut self, number: u8) -> &mut Keyboard {
        let index = match self.keyboards.iter().position(|k| k.number == number) {
            Some(index) => index,
            None => {
                self.keyboards.push(Keyboard::new(number));
                self.keyboards.sort_by_key(|keyboard| keyboard.number);
                self.keyboards
                    .iter()
                    .position(|k| k.number == number)
                    .unwrap()
            }
        };
        &mut self.keyboards[index]
    }

    /// Everything that would stop the layout from going to the Nano, in plain words.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let mut seen = [false; KEYBOARDS as usize];
        for keyboard in &self.keyboards {
            if !(1..=KEYBOARDS).contains(&keyboard.number) {
                problems.push(format!(
                    "Keyboard {}: Keyboards are 1 to 16",
                    keyboard.number
                ));
                continue;
            }
            if std::mem::replace(&mut seen[keyboard.number as usize - 1], true) {
                problems.push(format!("Keyboard {} is there twice", keyboard.number));
            }
            if keyboard.automatic {
                continue;
            }
            for (pair, key) in keyboard.keys.iter().enumerate() {
                if !key.is_valid() {
                    let what = match key {
                        Key::Sensor(_) => "track sections are 1 to 4096",
                        _ => "turnouts are 1 to 2048",
                    };
                    problems.push(format!(
                        "Keyboard {} key pair {}: {what}",
                        keyboard.number,
                        pair + 1
                    ));
                }
            }
        }
        if self.options.coil_ms > 12750 || !self.options.coil_ms.is_multiple_of(50) {
            problems.push("The coil time is 0 to 12750 ms, in steps of 50".to_owned());
        }
        problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_as_the_firmware_writes_them() {
        assert_eq!(Key::Turnout(12).to_entry(), "12");
        assert_eq!(Key::Sensor(33).to_entry(), "s33");
        assert_eq!(Key::Nothing.to_entry(), "0");
        assert_eq!(Key::from_entry("12"), Some(Key::Turnout(12)));
        assert_eq!(Key::from_entry("S33"), Some(Key::Sensor(33)));
        assert_eq!(Key::from_entry("0"), Some(Key::Nothing));
        assert_eq!(Key::from_entry("2049"), None);
        assert_eq!(Key::from_entry("s0"), None);
        assert_eq!(Key::from_entry("s4097"), None);
        assert_eq!(Key::from_entry("x"), None);
        assert_eq!(Key::from_entry("-1"), None);
    }

    #[test]
    fn automatic_keys_follow_the_firmware() {
        assert_eq!(Keyboard::automatic_keys(1)[0], Key::Turnout(1));
        let keys = Keyboard::automatic_keys(2);
        assert_eq!(keys[0], Key::Turnout(17));
        assert_eq!(keys[15], Key::Turnout(32));
        assert_eq!(Keyboard::automatic_keys(16)[15], Key::Turnout(256));
    }

    #[test]
    fn a_layout_survives_saving_and_opening() {
        let mut layout = Layout {
            name: "Adorf".to_owned(),
            ..Layout::default()
        };
        let keyboard = layout.keyboard_mut(2);
        keyboard.automatic = false;
        keyboard.keys[8] = Key::Sensor(33);
        keyboard.key_names[8] = "Track 1".to_owned();
        layout.keyboard_mut(1);
        assert_eq!(layout.keyboards[0].number, 1, "sorted by number");
        let opened = Layout::from_json(&layout.to_json()).unwrap();
        assert_eq!(opened, layout);
    }

    #[test]
    fn foreign_and_newer_files_are_refused() {
        assert!(matches!(
            Layout::from_json(r#"{"format":"other","version":1}"#),
            Err(LayoutError::NotALayout)
        ));
        assert!(matches!(
            Layout::from_json(r#"{"format":"loconet-6040-studio","version":99}"#),
            Err(LayoutError::Newer(99))
        ));
    }

    #[test]
    fn problems_are_named() {
        let mut layout = Layout::default();
        let keyboard = layout.keyboard_mut(1);
        keyboard.automatic = false;
        keyboard.keys[3] = Key::Turnout(3000);
        layout.options.coil_ms = 210;
        assert_eq!(
            layout.problems(),
            [
                "Keyboard 1 key pair 4: turnouts are 1 to 2048",
                "The coil time is 0 to 12750 ms, in steps of 50"
            ]
        );
    }
}
