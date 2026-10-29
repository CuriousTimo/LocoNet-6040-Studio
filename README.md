# LocoNet 6040 Studio

<p align="center">
  <img src="app/ui/logo.png" alt="Losse Eindj.es by Curious Timo" width="360">
</p>

**Set up your Märklin Keyboards 6040 on LocoNet with the mouse instead of typed commands.**

![LocoNet 6040 Studio](docs/screenshot.png)

LocoNet 6040 Studio is the setup program for [Marklin-6040-LocoNet](https://github.com/CuriousTimo/Arduino-ModelRail/tree/master/Marklin-6040-LocoNet): an Arduino Nano that lets unmodified Märklin Keyboards 6040 work on LocoNet, like an Uhlenbrock IB-Switch. In the Studio you see your Keyboards as they look on the layout, click a key pair and choose what it does. Then you send the setup to the Nano over its USB cable.

> **Beta, not yet tested with a real Nano.** Version 0.1.0 has been built and its file handling tested on a computer, but not yet used with a Nano and real Keyboards. Test reports are very welcome.

## What it does

- **Your Keyboards as you know them:** each Keyboard 6040 with its 16 key pairs in two rows of 8, red above green, numbered as on the Keyboard.
- **Click a key pair and choose:** it switches a turnout (1 to 2048), it shows a track section (red occupied, green free), or it does nothing. Give it a name, such as "Entry track 1", so you still know what it was for next year.
- **Read from Nano:** takes over the setup that is on the Nano now.
- **Send to Nano:** puts your setup on the Nano and then reads it back to check that the Nano really has it.
- **Save and open** your setup as a file on the computer, with all its names. That is also your backup: if a Nano is replaced, open the file and send it to the new one.
- **Settings of the Nano:** automatic numbering, coil time, no keys without track power, and the others, with a plain explanation for each.
- **Export backup:** the setup as the list of commands the Nano understands, the same text as its `backup` command, to keep or to type in on the Serial Monitor.
- **Export KeyMap.h:** the setup as a file for the Arduino IDE, for those who load the program themselves.

## How to use it

1. Connect the Nano with its USB cable.
2. Start LocoNet 6040 Studio. At the top right, choose the Nano's USB port (press **Refresh** if it is not in the list).
3. Press **Read from Nano** to start from what the Nano has now, or start with an empty layout.
4. On the left, choose a Keyboard or add one. Its number is the one set with the four switches on the back of the Keyboard (0 to 15).
5. Choose **Own assignment**, click a key pair and choose what it does. The key pairs are numbered as on the Keyboard: 1 to 8 in the top row, 9 to 16 in the bottom row.
6. Press **Send to Nano**. The Studio tells you when the Nano has the setup.
7. **Save** the layout (File → Save) to keep it.

The Nano restarts each time the Studio connects to it; that takes about two seconds and is normal.

## Download

There are no ready-made downloads yet. Until then, the Studio is built from the source code (below).

## Building from source

You need [Rust](https://rustup.rs). On Linux also the libraries Tauri needs (`libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`, `patchelf`) and `libudev-dev`.

```sh
cargo test -p l6040-core                  # the layout, backup and KeyMap.h code
cargo run -p loconet-6040-studio          # start the app
cargo install tauri-cli --version "^2"    # once, for installers
cd app/src-tauri && cargo tauri build     # installers in target/release/bundle
```

The code:

| Folder | What is in it |
| --- | --- |
| `crates/l6040-core` | The layout, reading and writing the Nano's `backup` text, comparing a layout with the Nano, `KeyMap.h` |
| `app/src-tauri` | The app: windows, menus, files, and the USB connection to the Nano (`nano.rs`) |
| `app/ui` | What you see: plain HTML, CSS and JavaScript |

The Studio talks to the Nano with the same commands a person types on the Serial Monitor (`backup`, `keys`, `clear`, `option`), one at a time, waiting for each answer. See the firmware's README for the commands.

## Planned

- Loading the Marklin-6040-LocoNet program onto a new Nano from the Studio, without the Arduino IDE
- Ready-made downloads for Windows, macOS and Linux
- A test view: see each key press and switch command live while the Nano is connected
- Printing labels for the strips above the keys of a Keyboard 6040

## License

[Apache License 2.0](LICENSE). The logo is the logo of Losse Eindj.es by Curious Timo.

Märklin is a trademark of Gebr. Märklin & Cie. GmbH, Uhlenbrock and Intellibox of Uhlenbrock Elektronik GmbH. This is an independent hobby project.
