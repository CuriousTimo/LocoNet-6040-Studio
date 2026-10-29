//! Talking to the Nano over USB, as a person would on the Serial Monitor: one command, then
//! its answer, then the next. The Nano can only take about 64 characters while it is busy,
//! so nothing is sent before it has answered the previous command.

use std::io::{Read, Write};
use std::time::{Duration, Instant};

use serde::Serialize;
use serialport::SerialPort;

const BAUD: u32 = 115_200;
/// Opening the port restarts most Nanos; they greet within about two seconds.
const BOOT_TIME: Duration = Duration::from_secs(4);
/// A command's answer is complete when nothing more came for this long.
const QUIET: Duration = Duration::from_millis(150);
const ANSWER_TIME: Duration = Duration::from_secs(3);
const BACKUP_TIME: Duration = Duration::from_secs(8);

#[derive(Serialize)]
pub struct PortInfo {
    pub name: String,
    /// What the port is, for the list: "USB serial (CH340)", or the name again.
    pub label: String,
    /// A USB port, as a Nano is; the first one is chosen when nothing is.
    pub usb: bool,
}

/// The serial ports, USB ones first. On macOS only the /dev/cu.* half of each pair, and
/// not the built-in debug and Bluetooth ports.
pub fn ports() -> Vec<PortInfo> {
    let mut ports: Vec<PortInfo> = serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .filter(|port| !port.port_name.starts_with("/dev/tty."))
        // macOS's own ports, never a Nano
        .filter(|port| {
            let name = port.port_name.to_lowercase();
            !name.contains("debug-console") && !name.contains("bluetooth")
        })
        .map(|port| {
            let usb = matches!(port.port_type, serialport::SerialPortType::UsbPort(_));
            let label = match &port.port_type {
                serialport::SerialPortType::UsbPort(usb) => {
                    let product = usb
                        .product
                        .clone()
                        .unwrap_or_else(|| "USB serial".to_owned());
                    format!("{} ({product})", port.port_name)
                }
                _ => port.port_name.clone(),
            };
            PortInfo {
                name: port.port_name,
                label,
                usb,
            }
        })
        .collect();
    ports.sort_by_key(|port| !port.usb);
    ports
}

pub struct Nano {
    port: Box<dyn SerialPort>,
    pending: Vec<u8>,
    /// The firmware version from the greeting or the `version` answer.
    pub firmware: Option<String>,
}

impl Nano {
    /// Opens the port and makes sure Marklin-6040-LocoNet answers on it.
    pub fn connect(name: &str) -> Result<Nano, String> {
        let port = serialport::new(name, BAUD)
            .timeout(Duration::from_millis(20))
            .open()
            .map_err(|error| format!("Cannot open {name}: {error}"))?;
        let mut nano = Nano {
            port,
            pending: Vec::new(),
            firmware: None,
        };
        // Wait for the greeting of a Nano that restarted when the port opened.
        let greeting = nano.read_until(|line| line.starts_with("Type `help`"), BOOT_TIME);
        nano.note_version(&greeting);
        if nano.firmware.is_none() {
            // A Nano that did not restart: ask it.
            let answer = nano.exchange("version")?;
            nano.note_version(&answer);
        }
        if nano.firmware.is_none() {
            return Err(format!(
                "No Marklin-6040-LocoNet answers on {name}. Is it the right port, and is the program loaded onto the Nano?"
            ));
        }
        Ok(nano)
    }

    fn note_version(&mut self, lines: &[String]) {
        for line in lines {
            if let Some(rest) = line.split("Marklin-6040-LocoNet ").nth(1) {
                if let Some(version) = rest.split_whitespace().next() {
                    self.firmware = Some(version.to_owned());
                }
            }
        }
    }

    /// The Nano's `backup`, complete with its last line.
    pub fn read_backup(&mut self) -> Result<String, String> {
        self.send_line("backup")?;
        let lines = self.read_until(|line| line == l6040_core::backup::END, BACKUP_TIME);
        if lines.last().map(String::as_str) != Some(l6040_core::backup::END) {
            return Err("The Nano did not finish its backup. Try again.".to_owned());
        }
        Ok(lines.join("\n"))
    }

    /// Sends one command and returns its answer, the echo included.
    pub fn exchange(&mut self, command: &str) -> Result<Vec<String>, String> {
        self.send_line(command)?;
        let echo = format!("> {command}");
        let start = Instant::now();
        let mut lines = Vec::new();
        let mut echoed = false;
        let mut last_data = Instant::now();
        while start.elapsed() < ANSWER_TIME {
            match self.read_line() {
                Some(line) => {
                    echoed |= line.eq_ignore_ascii_case(&echo);
                    lines.push(line);
                    last_data = Instant::now();
                }
                None if echoed && last_data.elapsed() >= QUIET => return Ok(lines),
                None => {}
            }
        }
        if echoed {
            Ok(lines)
        } else {
            Err(format!("The Nano did not answer `{command}`."))
        }
    }

    fn send_line(&mut self, command: &str) -> Result<(), String> {
        // Whatever the Nano logged in between belongs to no command.
        while self.read_line().is_some() {}
        self.port
            .write_all(format!("{command}\n").as_bytes())
            .and_then(|()| self.port.flush())
            .map_err(|error| format!("Cannot send to the Nano: {error}"))
    }

    fn read_until(&mut self, done: impl Fn(&str) -> bool, time: Duration) -> Vec<String> {
        let start = Instant::now();
        let mut lines = Vec::new();
        while start.elapsed() < time {
            if let Some(line) = self.read_line() {
                let finished = done(&line);
                lines.push(line);
                if finished {
                    break;
                }
            }
        }
        lines
    }

    /// The next complete line, or None if none came within the port's short timeout.
    fn read_line(&mut self) -> Option<String> {
        loop {
            if let Some(end) = self.pending.iter().position(|&byte| byte == b'\n') {
                let line: Vec<u8> = self.pending.drain(..=end).collect();
                return Some(String::from_utf8_lossy(&line).trim_end().to_owned());
            }
            let mut buffer = [0_u8; 256];
            match self.port.read(&mut buffer) {
                Ok(count) if count > 0 => self.pending.extend_from_slice(&buffer[..count]),
                _ => return None,
            }
        }
    }
}
