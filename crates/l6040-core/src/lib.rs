//! What LocoNet 6040 Studio knows about the Marklin-6040-LocoNet firmware: the layout it
//! edits ([`model`]), the firmware's `backup` text, which is also the list of commands that
//! sends a layout to the Nano ([`backup`]), and the `KeyMap.h` file for the Arduino IDE
//! ([`keymap`]).

pub mod backup;
pub mod keymap;
pub mod model;

pub use model::{Key, Keyboard, Layout, Options};
