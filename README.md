# Kinetix

Kinetix is a fast, single-binary automatic tiling window manager for KDE Plasma 6 on Wayland (X11 support in progress).

Built in Rust, it injects a transient KWin scripting payload via D-Bus and manages windows entirely inside KWin's native tile system — no root access, no permanent config files, no daemons.

---

## Features

- **Organic BSP Layout Engine**: Kinetix uses a smart BSP (Binary Space Partitioning) algorithm that respects your manual tile edits. New windows are organically placed into the largest available tiles without aggressively rebuilding the entire tree or destroying your manual setups.
- **Surgical Split & Swap (Drag-and-Drop)**: 
  - Drop a window onto the **center** of another window to **swap** their positions.
  - Drop near the **left, right, top, or bottom edge** of a window to perfectly **split** that tile exactly where you dropped it.
- **Accidental Drag Cancellation**: Releasing a window back near its starting position (<50px) immediately cancels the drag operation, hiding the overlay and preserving its original tile layout without unexpected reshuffling.
- **Auto-Collapsing Free Space Engine**: Automatically forces KWin layout re-evaluations whenever tiles are deleted, ensuring zero leftover gaps or abandoned free spaces.
- **Live Split Previews**: A native Rust Wayland/X11 overlay window dynamically highlights exactly how the tiles will split in real-time as you drag.
- **Robust 1-Window-Per-Tile Rule**: Kinetix enforces a strict non-overlapping rule, instantly ejecting windows if KWin accidentally stacks them in a single tile.
- **Minimized / maximized windows are skipped**: Kinetix never forces a minimized or maximized window into the grid.
- **Single binary**: No runtime dependencies beyond a D-Bus session and KDE Plasma 6.
- **Configurable**: gaps, window cap, swap zone size, and more.

---

## Requirements

- Linux x86_64
- KDE Plasma 6 on Wayland (`kwin_wayland`)
- D-Bus session bus

---

## Building

```bash
git clone https://github.com/your-username/kinetix.git
cd kinetix
cargo build --release
```

Binary at `target/release/kinetix`.

---

## Usage

```bash
./target/release/kinetix [OPTIONS]
```

### Options

| Option | Default | Description |
|---|---|---|
| `--max-windows <N>` | `0` | Max tiled windows (`0` = unlimited) |
| `--gaps <PX>` | `0` | Outer screen padding in pixels |
| `--inner-gaps <PX>` | `0` | Gap between adjacent tiles in pixels |
| `--swap-zone-ratio <RATIO>` | `0.4` | Center fraction reserved for drag-to-swap (`0.0`–`1.0`) |
| `--debug` | `false` | Verbose logging to `./.kinetix/kinetix.log` |

### Examples

```bash
# Run with defaults
./kinetix

# 12px outer padding, 6px inner gaps
./kinetix --gaps 12 --inner-gaps 6

# Limit tiling to 4 windows
./kinetix --max-windows 4

# Larger center swap zone (drag must be closer to edge to split)
./kinetix --swap-zone-ratio 0.6
```

---

## Drag-and-Drop

When you drag a window title bar over another tiled window, a native UI overlay will visualize the target zones:

```
+--------------------------------------------------+
|                    TOP SPLIT                     |
|    +--------------------------------------+      |
|    |                                      |      |
| L  |            SWAP ZONE                 |  R   |
| E  |         (center region)              |  I   |
| F  |                                      |  G   |
| T  +--------------------------------------+  H   |
|                  BOTTOM SPLIT                T   |
+--------------------------------------------------+
```

- **Center zone** → swap the two windows' positions.
- **Edge zones** (left / right / top / bottom) → split the target tile surgically along that axis and place the dragged window on the chosen side.

The center zone size is controlled by `--swap-zone-ratio` (default `0.4`, meaning the inner 40% of the tile width and height).

---

## How It Works

On startup Kinetix:

1. Connects to `org.kde.KWin` via D-Bus and registers a bridge at `org.kde.kinetix.Bridge`.
2. Writes a self-contained JavaScript tiling engine (v9.9+) to a temp file and loads it as a transient KWin script.
3. The JS engine tiles all open windows organically, hooks window lifecycle events, and intercepts drag signals.
4. During drag operations, the JS engine calls back into the Rust daemon via D-Bus to display a high-performance visual overlay.

On exit (Ctrl+C / SIGTERM), Kinetix unloads the script and removes all custom tile assignments.

---

## Troubleshooting

### Check what the script is doing
```bash
journalctl --user -f | grep "js:"
```

### Force KWin to re-read its state
```bash
qdbus org.kde.KWin /KWin reconfigure
```

### Multiple tiling instances running?
If you `kill -9` the Rust daemon without letting it shut down cleanly (Ctrl+C), KWin will silently keep the old JavaScript plugins running in the background. This leads to chaotic behavior where multiple layout engines fight over the same windows (causing massive overlaps or broken splits).

To forcefully kill all zombie KWin scripts, run:
```bash
for i in {1..30}; do
  dbus-send --session --dest=org.kde.KWin /Scripting/Script$i org.kde.kwin.Script.stop 2>/dev/null || true
done
```
Then restart Kinetix cleanly.

---

## License

GPL-3.0. See [LICENSE](LICENSE).
