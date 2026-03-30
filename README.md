# bbsaver

ANSI art screensaver that scrolls through [16colors](https://16colo.rs/) art packs at simulated modem speeds.

Renders CP437 glyphs pixel-perfect at any resolution using GPU-instanced rendering (wgpu). Each art file scrolls through row-by-row like a real BBS terminal receiving data over a modem, with attribution lines between pieces showing the artist and group from SAUCE metadata.

## Usage

```sh
# From a local directory
bbsaver --pack /path/to/artpack/

# From a ZIP file
bbsaver --pack /path/to/artpack.zip

# From a URL
bbsaver --pack https://raw.githubusercontent.com/sixteencolors/sixteencolors-archive/master/1996/acid-50a.zip

# Adjust speed (default: 9600 baud)
bbsaver --pack /path/to/pack --baud 2400

# Fullscreen
bbsaver --pack /path/to/pack --fullscreen

# Smooth scrolling instead of row-by-row stepping
bbsaver --pack /path/to/pack --smooth
```

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

# Grab some packs
curl -sLO https://raw.githubusercontent.com/sixteencolors/sixteencolors-archive/master/1996/acid-50a.zip
curl -sLO https://raw.githubusercontent.com/sixteencolors/sixteencolors-archive/master/1996/acid-52.zip
curl -sLO https://raw.githubusercontent.com/sixteencolors/sixteencolors-archive/master/1997/ice-9710.zip
```

## Screensaver setup

### Hyprland (Omarchy / CachyOS Hyprland)

Add to `~/.config/hypr/hypridle.conf`:

```
listener {
    timeout = 150
    on-timeout = bbsaver --fullscreen --pack ~/.local/share/bbsaver/packs/acid-50a.zip
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
    "screenOffCommand": "bbsaver --fullscreen --pack ~/.local/share/bbsaver/packs/acid-50a.zip",
    "resumeScreenOffCommand": "pkill bbsaver"
  }
}
```

### Niri + swayidle (without Noctalia)

```sh
swayidle -w \
    timeout 150 'bbsaver --fullscreen --pack ~/.local/share/bbsaver/packs/acid-50a.zip' \
    resume 'pkill bbsaver' \
    timeout 300 'swaylock'
```

### Sway

Add to `~/.config/sway/config`:

```
exec swayidle -w \
    timeout 150 'bbsaver --fullscreen --pack ~/.local/share/bbsaver/packs/acid-50a.zip' \
    resume 'pkill bbsaver' \
    timeout 300 'swaylock -f'
```

## How it works

1. Loads `.ANS` / `.ICE` files from an art pack (directory or ZIP)
2. Parses each file through an ANSI state machine into a cell buffer (80 columns, variable height)
3. Reads SAUCE metadata for attribution (title, author, group)
4. Renders cells as GPU-instanced textured quads using an embedded IBM VGA 8x16 font atlas
5. Scrolls through the buffer at the configured baud rate, with blank screen gaps between pieces
6. Loops forever. Exits cleanly on Escape, window close, or SIGTERM.
