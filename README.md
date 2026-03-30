# bbsaver

ANSI art screensaver that scrolls through [16colors](https://16colo.rs/) art packs at simulated modem speeds.

Renders CP437 glyphs pixel-perfect at any resolution using GPU-instanced rendering (wgpu). Each art file scrolls through row-by-row like a real BBS terminal receiving data over a modem, with attribution lines between pieces showing the artist and group from SAUCE metadata.

## Usage

```sh
# From a local directory
bbsaver --pack /path/to/artpack/

# From a ZIP file
bbsaver --pack /path/to/artpack.zip

# From a URL (downloads automatically)
bbsaver --pack https://raw.githubusercontent.com/sixteencolors/sixteencolors-archive/master/1996/acid-50a.zip

# Adjust speed (default: 9600 baud)
bbsaver --pack /path/to/pack --baud 2400

# Fullscreen
bbsaver --pack /path/to/pack --fullscreen

# Fullscreen on all monitors
bbsaver --pack /path/to/pack --fullscreen --all-monitors

# Smooth scrolling instead of row-by-row stepping
bbsaver --pack /path/to/pack --smooth
```

### Flags

| Flag | Description |
|------|-------------|
| `--pack <path\|url>` | Path to art pack directory, ZIP file, or URL (required) |
| `--baud <rate>` | Simulated baud rate (default: 9600) |
| `--fullscreen` | Launch in fullscreen mode |
| `--all-monitors` | Show on all connected monitors (requires `--fullscreen`) |
| `--smooth` | Smooth sub-pixel scrolling instead of row-by-row stepping |

### Baud rates

| Baud  | Rows/sec | Feel                          |
|-------|----------|-------------------------------|
| 2400  | 3        | Slow, can read every line     |
| 9600  | 12       | Art paints top to bottom      |
| 14400 | 18       | Quick downward wipe           |
| 28800 | 36       | Near-instant                  |

## Install

### From release

Download the latest binary from [releases](https://github.com/eveld/bbsaver/releases):

```sh
# Linux x86_64
curl -sL https://github.com/eveld/bbsaver/releases/latest/download/bbsaver-linux-x86_64.tar.gz | tar xz
sudo mv bbsaver /usr/local/bin/

# Linux aarch64
curl -sL https://github.com/eveld/bbsaver/releases/latest/download/bbsaver-linux-aarch64.tar.gz | tar xz
sudo mv bbsaver /usr/local/bin/

# macOS Apple Silicon
curl -sL https://github.com/eveld/bbsaver/releases/latest/download/bbsaver-macos-aarch64.tar.gz | tar xz
sudo mv bbsaver /usr/local/bin/
```

### From source

```sh
git clone https://github.com/eveld/bbsaver.git
cd bbsaver
cargo build --release
sudo cp target/release/bbsaver /usr/local/bin/
```

## Art packs

Download packs from [16colo.rs](https://16colo.rs/) or the [GitHub archive](https://github.com/sixteencolors/sixteencolors-archive):

```sh
mkdir -p ~/.local/share/bbsaver/packs
cd ~/.local/share/bbsaver/packs

# Classic ACiD Productions
curl -sLO https://raw.githubusercontent.com/sixteencolors/sixteencolors-archive/master/1996/acid-50a.zip
curl -sLO https://raw.githubusercontent.com/sixteencolors/sixteencolors-archive/master/1996/acid-52.zip

# Blocktronics (modern ANSI art group)
curl -sLO https://raw.githubusercontent.com/sixteencolors/sixteencolors-archive/master/2016/blocktronics_block_n_roll.zip
curl -sLO https://raw.githubusercontent.com/sixteencolors/sixteencolors-archive/master/2013/BLOCKTRONICS_SPACE_INVADERS.zip
```

Supports variable-width art -- packs with mixed 80/160/210 column files render correctly with narrower pieces centered.

## Multi-monitor

With `--fullscreen --all-monitors`, bbsaver opens a window on every connected display. Each window renders at its monitor's native resolution with the art centered and scaled to fill the height. All monitors scroll in sync.

The art is always centered horizontally, so ultrawide monitors get black bars on the sides rather than stretched characters.

## Screensaver setup

### Hyprland (Omarchy / CachyOS Hyprland)

Add to `~/.config/hypr/hypridle.conf`:

```
listener {
    timeout = 150
    on-timeout = bbsaver --fullscreen --all-monitors --pack ~/.local/share/bbsaver/packs/acid-50a.zip
    on-resume = pkill bbsaver
}

listener {
    timeout = 300
    on-timeout = hyprlock
}
```

### Niri + Noctalia Shell

Noctalia has built-in idle management. Edit `~/.config/noctalia/settings.json`:

```json
{
  "idle": {
    "enabled": true,
    "screenOffTimeout": 150,
    "lockTimeout": 300,
    "suspendTimeout": 1800,
    "screenOffCommand": "bbsaver --fullscreen --all-monitors --pack ~/.local/share/bbsaver/packs/acid-50a.zip",
    "resumeScreenOffCommand": "pkill bbsaver"
  }
}
```

### Niri + swayidle (without Noctalia)

```sh
swayidle -w \
    timeout 150 'bbsaver --fullscreen --all-monitors --pack ~/.local/share/bbsaver/packs/acid-50a.zip' \
    resume 'pkill bbsaver' \
    timeout 300 'swaylock'
```

### Sway

Add to `~/.config/sway/config`:

```
exec swayidle -w \
    timeout 150 'bbsaver --fullscreen --all-monitors --pack ~/.local/share/bbsaver/packs/acid-50a.zip' \
    resume 'pkill bbsaver' \
    timeout 300 'swaylock -f'
```

## How it works

1. Loads `.ANS` / `.ICE` files from an art pack (directory, ZIP, or URL)
2. Reads SAUCE metadata for canvas width (supports 80, 160, 210+ columns) and attribution
3. Parses each file through an ANSI state machine into a cell buffer
4. Centers narrower files within the widest file's column count
5. Renders cells as GPU-instanced textured quads using an embedded IBM VGA 8x16 font atlas
6. Scales art to fill screen height, centers horizontally (no stretching on ultrawide)
7. Scrolls row-by-row at the configured baud rate, with full-screen gaps between pieces
8. On all monitors: each window renders at native resolution, all synced to the same scroll position
9. Loops forever. Exits cleanly on Escape, window close, or SIGTERM.
