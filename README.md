# zipline-upload

Upload files to [Zipline](https://zipline.diced.sh) from the right-click menu, a drag & drop window, or the terminal. URL gets copied to clipboard automatically.

## What it does

- Right-click any file → Open With → Zipline Upload (silent, just uploads)
- Right-click → Zipline Upload (Advanced) — opens the GUI with the file loaded so you can set options first
- Drag & drop GUI with a URL shortener tab and a settings page
- Per-upload overrides for expiry, max views, password, compression, etc.
- Extension allowlist — if the file type isn't allowed it blocks before even trying
- KDE notifications with a clickable URL

## Dependencies

- `wl-copy` for clipboard on Wayland, or `xclip`/`xsel` on X11
- `notify-send` for notifications

## Building

```bash
cargo build --release
```

On Bazzite or other immutable distros, build inside a toolbox:

```bash
toolbox create && toolbox enter
sudo dnf install -y rust cargo libxcb-devel dbus-devel pkg-config openssl-devel
cargo build --release
```

## Setup

Put the binary somewhere permanent and run it once:

```bash
cp target/release/zipline-upload ~/.local/bin/
zipline-upload
```

First run registers the Open With entries and asks for your `.sxcu` file (get it from your Zipline dashboard under Settings → Upload Config).

Upgrading is just replacing the binary and running it — it'll add any missing desktop entries automatically.

## Uninstall

```bash
zipline-upload --uninstall
rm ~/.local/bin/zipline-upload
```

Config and settings in `~/.config/zipline-upload/` are left alone, delete that too if you want everything gone.

## CLI

```
zipline-upload [FILE]            # upload silently
zipline-upload --advanced [FILE] # open GUI with file + options
zipline-upload                   # open GUI
zipline-upload --uninstall
zipline-upload --kde / --no-kde  # override notification style
```

## Config files

| Path | What it is |
|------|-----------|
| `~/.config/zipline-upload/config.sxcu` | Your Zipline token and upload URL |
| `~/.config/zipline-upload/settings.json` | Upload defaults, notification style, allowed extensions |
| `~/.config/zipline-upload/errors.log` | Failed uploads |

To swap your token, hit Change… in the GUI or delete `config.sxcu` and re-run.
