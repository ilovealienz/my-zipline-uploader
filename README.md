# zipline-upload

A lightweight Linux uploader for [Zipline](https://zipline.diced.sh). Upload files via right-click → Open With, drag & drop GUI, or CLI. Copies the URL to clipboard automatically.

## Requirements

- `wl-copy` (Wayland) or `xclip`/`xsel` (X11)
- `notify-send`
- A Zipline `.sxcu` config file (generated from your Zipline dashboard)

## Building

Requires a Rust toolchain.

```bash
cargo build --release
```

## Install

Place the binary wherever you want it to live permanently, then run it once. It will register itself as an Open With handler pointing to that location and prompt for your `.sxcu` config file.

To re-run the install (e.g. after moving the binary):

```bash
rm ~/.config/zipline-upload/installed
zipline-upload
```

## Reconfigure

To switch to a different `.sxcu` file:

```bash
rm ~/.config/zipline-upload/config.sxcu
zipline-upload
```

## Usage

```
zipline-upload [FLAGS] [FILE]

FLAGS:
    --kde       Force KDE notification style
    --no-kde    Force plain text notification style
    --help      Show this help

Without FILE, opens the drag & drop GUI.
```
