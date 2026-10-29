//! The firmware's `backup` text. The Nano writes its whole setup as the commands that
//! recreate it, so the same text is read from the Nano, saved as a backup file, and sent
//! back line by line:
//!
//! ```text
//! # Marklin-6040-LocoNet 0.1.0 backup
//! option auto on
//! option coil 200
//! # keyboard 0: on the bus, automatic
//! clear 0
//! # keyboard 2: on the bus, own assignment
//! keys 2 101 102 103 104 210 106 0 108 s33 110 111 112 113 114 115 116
//! # end of backup
//! ```

use crate::model::{Key, Keyboard, Layout, Options, KEYBOARDS, KEY_PAIRS};

/// The line the firmware ends its backup with.
pub const END: &str = "# end of backup";

/// The oldest firmware the Studio works with: 0.2.0 numbers Keyboards 1 to 16 as Märklin
/// does; 0.1.x counted 0 to 15, so its numbers would all be one off.
pub const OLDEST_FIRMWARE: (u32, u32) = (0, 2);

/// Whether the Studio can work with this firmware version ("0.2.0").
pub fn firmware_supported(version: &str) -> bool {
    let mut parts = version.split('.').map(|part| part.parse::<u32>().ok());
    match (parts.next().flatten(), parts.next().flatten()) {
        (Some(major), Some(minor)) => (major, minor) >= OLDEST_FIRMWARE,
        _ => false,
    }
}

/// The commands that make the Nano's setup equal to the layout: the options, then every
/// Keyboard 1 to 16, so a Keyboard the layout does not have goes back to automatic.
pub fn to_commands(layout: &Layout) -> Vec<String> {
    let options = &layout.options;
    let on_off = |name: &str, on: bool| format!("option {name} {}", if on { "on" } else { "off" });
    let mut commands = vec![
        on_off("auto", options.auto_assign),
        on_off("stopgo", options.stop_go),
        on_off("hold", options.hold_without_power),
        on_off("log", options.log),
        on_off("watchdog", options.watchdog),
        format!("option coil {}", options.coil_ms),
    ];
    for number in 1..=KEYBOARDS {
        match layout.keyboard(number) {
            Some(keyboard) if !keyboard.automatic => {
                let entries: Vec<String> = keyboard.keys.iter().map(|key| key.to_entry()).collect();
                commands.push(format!("keys {number} {}", entries.join(" ")));
            }
            _ => commands.push(format!("clear {number}")),
        }
    }
    commands
}

/// A backup file: the commands with a heading and a note per Keyboard, as the firmware
/// writes it; names from the Studio go into the notes.
pub fn to_text(layout: &Layout) -> String {
    let mut text = String::from("# LocoNet 6040 Studio backup");
    if !layout.name.is_empty() {
        text.push_str(&format!(": {}", layout.name));
    }
    text.push('\n');
    for command in to_commands(layout) {
        if let Some(number) = command
            .strip_prefix("keys ")
            .or_else(|| command.strip_prefix("clear "))
            .and_then(|rest| rest.split(' ').next())
            .and_then(|number| number.parse::<u8>().ok())
        {
            let keyboard = layout.keyboard(number);
            let name = keyboard.map(|k| k.name.as_str()).unwrap_or_default();
            if keyboard.is_some() || !name.is_empty() {
                text.push_str(&format!("# keyboard {number}"));
                if !name.is_empty() {
                    text.push_str(&format!(": {name}"));
                }
                text.push('\n');
            }
        }
        text.push_str(&command);
        text.push('\n');
    }
    text.push_str(END);
    text.push('\n');
    text
}

/// What a backup text held.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Parsed {
    /// The setup as a layout: Keyboards with their own assignment, and automatic ones the
    /// Nano found on the bus.
    pub layout: Layout,
    /// The firmware version from the heading, when the Nano wrote it.
    pub firmware: Option<String>,
    /// Keyboards the Nano found on the bus, from its notes.
    pub on_the_bus: Vec<u8>,
    /// Lines that could not be read, with their line number, in plain words.
    pub problems: Vec<String>,
    /// Whether the text ended with the end-of-backup line.
    pub complete: bool,
}

/// Reads a backup: from the Nano (with its echo and log lines in between) or from a file.
pub fn parse(text: &str) -> Parsed {
    let mut options = Options::default();
    let mut keyboards: Vec<Keyboard> = Vec::new();
    let mut on_the_bus = Vec::new();
    let mut firmware = None;
    let mut problems = Vec::new();
    let mut complete = false;

    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        let at = index + 1;
        if line.is_empty() || line.starts_with("> ") || is_log_line(line) {
            continue;
        }
        if line == END {
            complete = true;
            continue;
        }
        if let Some(note) = line.strip_prefix('#') {
            read_note(note.trim(), &mut firmware, &mut on_the_bus);
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let result = match words[0].to_ascii_lowercase().as_str() {
            "option" => read_option(&words, &mut options),
            "clear" => read_keyboard_number(words.get(1)).map(|number| {
                keyboards.retain(|k| k.number != number);
            }),
            "keys" => read_keys(&words).map(|keyboard| {
                keyboards.retain(|k| k.number != keyboard.number);
                keyboards.push(keyboard);
            }),
            _ => Err("this is not a backup line".to_owned()),
        };
        if let Err(problem) = result {
            problems.push(format!("Line {at} ({line}): {problem}"));
        }
    }

    for &number in &on_the_bus {
        if !keyboards.iter().any(|k| k.number == number) {
            keyboards.push(Keyboard::new(number));
        }
    }
    keyboards.sort_by_key(|keyboard| keyboard.number);
    Parsed {
        layout: Layout {
            options,
            keyboards,
            ..Layout::default()
        },
        firmware,
        on_the_bus,
        problems,
        complete,
    }
}

/// Lines the firmware logs while it runs, which can come between the backup lines.
fn is_log_line(line: &str) -> bool {
    [
        "KEY:",
        "LN:",
        "I2C:",
        "REFUSED:",
        "RETRY:",
        "STOP ",
        "GO ",
        "Keyboard ",
    ]
    .iter()
    .any(|prefix| line.starts_with(prefix))
}

/// `# Marklin-6040-LocoNet 0.1.0 backup` and `# keyboard 2: on the bus, own assignment`.
fn read_note(note: &str, firmware: &mut Option<String>, on_the_bus: &mut Vec<u8>) {
    if let Some(rest) = note.strip_prefix("Marklin-6040-LocoNet ") {
        if let Some(version) = rest.split_whitespace().next() {
            *firmware = Some(version.to_owned());
        }
    } else if let Some(rest) = note.strip_prefix("keyboard ") {
        let (number, state) = rest.split_once(':').unwrap_or((rest, ""));
        if let (Ok(number), true) = (number.trim().parse::<u8>(), state.contains("on the bus")) {
            if (1..=KEYBOARDS).contains(&number) && !on_the_bus.contains(&number) {
                on_the_bus.push(number);
            }
        }
    }
}

fn read_keyboard_number(word: Option<&&str>) -> Result<u8, String> {
    word.and_then(|word| word.parse::<u8>().ok())
        .filter(|number| (1..=KEYBOARDS).contains(number))
        .ok_or_else(|| "Keyboards are 1 to 16".to_owned())
}

fn read_keys(words: &[&str]) -> Result<Keyboard, String> {
    let number = read_keyboard_number(words.get(1))?;
    let entries = &words[2..];
    if entries.len() != KEY_PAIRS {
        return Err(format!("needs 16 key pairs, has {}", entries.len()));
    }
    let mut keyboard = Keyboard::new(number);
    keyboard.automatic = false;
    for (pair, entry) in entries.iter().enumerate() {
        keyboard.keys[pair] = Key::from_entry(entry).ok_or_else(|| {
            format!(
                "key pair {}: {entry} is not a turnout or a track section",
                pair + 1
            )
        })?;
    }
    Ok(keyboard)
}

fn read_option(words: &[&str], options: &mut Options) -> Result<(), String> {
    let (Some(name), Some(value)) = (words.get(1), words.get(2)) else {
        return Err("an option needs a name and a value".to_owned());
    };
    if name.eq_ignore_ascii_case("coil") {
        let ms: u16 = value
            .parse()
            .ok()
            .filter(|ms| *ms <= 12750)
            .ok_or_else(|| "the coil time is 0 to 12750 ms".to_owned())?;
        options.coil_ms = ms;
        return Ok(());
    }
    let on = match value.to_ascii_lowercase().as_str() {
        "on" => true,
        "off" => false,
        _ => return Err("give on or off".to_owned()),
    };
    let option = match name.to_ascii_lowercase().as_str() {
        "auto" => &mut options.auto_assign,
        "stopgo" => &mut options.stop_go,
        "hold" => &mut options.hold_without_power,
        "log" => &mut options.log,
        "watchdog" => &mut options.watchdog,
        _ => return Err(format!("unknown option {name}")),
    };
    *option = on;
    Ok(())
}

/// Where a layout and what the Nano reports differ, in plain words; empty if they match.
/// Names are the Studio's own and do not count.
pub fn differences(wanted: &Layout, on_the_nano: &Layout) -> Vec<String> {
    let mut differences = Vec::new();
    if wanted.options != on_the_nano.options {
        differences.push("The options differ".to_owned());
    }
    for number in 1..=KEYBOARDS {
        let assignment = |layout: &Layout| match layout.keyboard(number) {
            Some(keyboard) if !keyboard.automatic => Some(keyboard.keys),
            _ => None,
        };
        let (wanted_keys, nano_keys) = (assignment(wanted), assignment(on_the_nano));
        if wanted_keys == nano_keys {
            continue;
        }
        match (wanted_keys, nano_keys) {
            (Some(wanted_keys), Some(nano_keys)) => {
                for pair in 0..KEY_PAIRS {
                    if wanted_keys[pair] != nano_keys[pair] {
                        differences.push(format!(
                            "Keyboard {number} key pair {}: wanted {}, the Nano has {}",
                            pair + 1,
                            wanted_keys[pair].to_entry(),
                            nano_keys[pair].to_entry()
                        ));
                    }
                }
            }
            (Some(_), None) => differences.push(format!(
                "Keyboard {number}: the Nano still has the automatic assignment"
            )),
            (None, Some(_)) => differences.push(format!(
                "Keyboard {number}: the Nano still has its own assignment"
            )),
            (None, None) => {}
        }
    }
    differences
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A backup as the firmware's `backup` command writes it, with the echo and a log
    /// line in between, as it arrives over USB.
    const FROM_THE_NANO: &str = "\
> backup
# Marklin-6040-LocoNet 0.2.0 backup
option auto on
option stopgo off
option hold on
option log on
option watchdog on
option coil 250
# keyboard 1: on the bus, automatic
clear 1
# keyboard 2: not found, automatic
clear 2
KEY: keyboard 1 key 3 green -> LocoNet switch 3 closed: Done
# keyboard 3: on the bus, own assignment
keys 3 101 102 103 104 210 106 0 108 s33 110 111 112 113 114 115 116
# end of backup
";

    #[test]
    fn reads_a_backup_from_the_nano() {
        let parsed = parse(FROM_THE_NANO);
        assert!(parsed.complete);
        assert!(parsed.problems.is_empty(), "{:?}", parsed.problems);
        assert_eq!(parsed.firmware.as_deref(), Some("0.2.0"));
        assert_eq!(parsed.on_the_bus, [1, 3]);
        assert_eq!(parsed.layout.options.coil_ms, 250);
        let numbers: Vec<u8> = parsed.layout.keyboards.iter().map(|k| k.number).collect();
        assert_eq!(
            numbers,
            [1, 3],
            "keyboard 2 is automatic and not on the bus"
        );
        let keyboard = parsed.layout.keyboard(3).unwrap();
        assert!(!keyboard.automatic);
        assert_eq!(keyboard.keys[4], Key::Turnout(210));
        assert_eq!(keyboard.keys[6], Key::Nothing);
        assert_eq!(keyboard.keys[8], Key::Sensor(33));
        assert!(parsed.layout.keyboard(1).unwrap().automatic);
    }

    #[test]
    fn commands_read_back_as_the_same_layout() {
        let parsed = parse(FROM_THE_NANO);
        let text = to_text(&parsed.layout);
        let again = parse(&text);
        assert!(again.complete);
        assert!(again.problems.is_empty(), "{:?}", again.problems);
        assert!(differences(&parsed.layout, &again.layout).is_empty());
        assert_eq!(to_commands(&parsed.layout).len(), 6 + 16);
    }

    #[test]
    fn names_problems_with_their_line() {
        let parsed = parse("keys 2 1 2 3\noption coil 99999\nfly away\nclear 0\n");
        assert!(!parsed.complete);
        assert_eq!(
            parsed.problems,
            [
                "Line 1 (keys 2 1 2 3): needs 16 key pairs, has 3",
                "Line 2 (option coil 99999): the coil time is 0 to 12750 ms",
                "Line 3 (fly away): this is not a backup line",
                "Line 4 (clear 0): Keyboards are 1 to 16",
            ]
        );
    }

    #[test]
    fn firmware_versions() {
        assert!(!firmware_supported("0.1.0"));
        assert!(firmware_supported("0.2.0"));
        assert!(firmware_supported("0.10.1"));
        assert!(firmware_supported("1.0.0"));
        assert!(!firmware_supported("unknown"));
    }

    #[test]
    fn differences_are_named() {
        let mut wanted = parse(FROM_THE_NANO).layout;
        let nano = wanted.clone();
        wanted.keyboard_mut(3).keys[0] = Key::Turnout(99);
        wanted.keyboard_mut(5).automatic = false;
        assert_eq!(
            differences(&wanted, &nano),
            [
                "Keyboard 3 key pair 1: wanted 99, the Nano has 101",
                "Keyboard 5: the Nano still has the automatic assignment",
            ]
        );
    }
}
