# Kinetix

Kinetix is an ultra-fast, zero-installation, single-binary automatic tiling window manager engine designed for Linux desktop environments (KDE Plasma 6 on Wayland and native X11).

Built with Rust, Kinetix bridges directly into display server interfaces (D-Bus KWin Scripting for Wayland and `x11rb` for X11) to manage window layouts dynamically. It features aspect-ratio-aware binary space partitioning (BSP / Dwindle), interactive multi-directional drag-and-drop window splitting/swapping, configurable window caps, and zero-footprint restorative exit.

---

## Architecture Overview

Kinetix operates as a lightweight user-space engine that dynamically communicates with your window manager without requiring root permissions, system daemons, or permanent configuration files:

- **Wayland Backend (KDE Plasma 6)**: Injects an in-memory transient KWin JavaScript engine payload via the `org.kde.KWin` D-Bus scripting interface (`/Scripting`). The script manages KWin `CustomTile` structures natively in KWin's event loop, avoiding slow IPC round-trips for frame rendering.
- **X11 Backend**: Connects directly to the X server socket using `x11rb`, intercepting window mapping events (`CreateNotify`, `DestroyNotify`, `ConfigureNotify`) and applying tree-calculated frame geometries.
- **Dwindle BSP Engine**: Computes recursive binary tree splits while evaluating target tile width vs. height ratios (`width / height`). Tiles wider than tall are split vertically (creating side-by-side tiles), while tall tiles are split horizontally (creating top/bottom stacked tiles) to maintain comfortable 16:9 / 4:3 window proportions.
- **Interactive Drag-and-Drop Handler**: Intercepts `interactiveMoveResizeStarted` and `interactiveMoveResizeFinished` signals. Windows unmanage from tile frames while dragged, allowing free movement. Upon drop, Kinetix evaluates target window bounding boxes and calculates drop zones (Center Swap vs. Edge Split).

---

## Features

- **Single-Binary Engine**: Fully self-contained Rust binary with no runtime scripting dependencies or external daemons.
- **Dual Display Server Support**: Works on **KDE Plasma 6 (Wayland)** and **X11**.
- **Aspect-Ratio-Aware Dwindle BSP**: Automatically balances split axes based on window dimensions to avoid ultra-skinny or excessively tall tiles.
- **Interactive Drag-and-Drop Tiling**:
  - **Center Drop Zone (Swap)**: Dropping a window into the middle center area of another tile swaps their positions.
  - **Multi-Directional Edge Splits**: Dropping near the Left, Right, Top, or Bottom edge of a tile splits that specific tile and places the dragged window into the selected side.
  - **Unconstrained Drag Movement**: Windows detach from tile constraints while being dragged for smooth cursor tracking.
- **Configurable Options**:
  - Max windows cap (`--max-windows`).
  - Outer screen gaps (`--gaps`) and inner tile gaps (`--inner-gaps`).
  - Configurable center swap threshold (`--swap-zone-ratio`).
- **Clean Restorative Exit**: Intercepts `SIGINT` / `SIGTERM` (`Ctrl+C`) to untile windows, clear custom KWin tiles, and restore original pre-tiled window dimensions.

---

## Requirements & Dependencies

### Prerequisites
- **Operating System**: Linux (x86_64)
- **Display Server**: Wayland with KDE Plasma 6 (`kwin_wayland`) or X11 (`Xorg` / `Xwayland`)
- **System Libraries**: `dbus` session bus enabled

### Build Toolchain
To compile Kinetix from source, you need Rust (cargo) 1.75 or later:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

---

## Installation & Compilation

Clone the repository and build the optimized release binary:

```bash
git clone https://github.com/your-username/kinetix.git
cd kinetix
cargo build --release
```

The compiled executable will be located at:
`./target/release/kinetix`

---

## Usage

Run the engine directly from terminal or add it to your desktop autostart commands:

```bash
./target/release/kinetix [OPTIONS]
```

### Configuration Options

| Option | Default | Description |
|---|---|---|
| `--layout <LAYOUT>` | `dwindle` | Tiling strategy algorithm (`dwindle`, `master-stack`, `floating`) |
| `--max-windows <N>` | `0` | Maximum number of tiled windows on screen (`0` = unlimited) |
| `--gaps <PX>` | `0` | Outer screen margin / padding in pixels |
| `--inner-gaps <PX>` | `0` | Inner gap between adjacent tile edges in pixels |
| `--swap-zone-ratio <RATIO>` | `0.4` | Inner center area fraction (`0.0`–`1.0`) reserved for drag-to-swap |
| `--drag-threshold <PX>` | `20` | Minimum drag distance in pixels to trigger user drag logic |
| `--restore-on-exit <BOOL>` | `true` | Automatically untile and restore original window sizes on exit |
| `--debug` | `false` | Enable verbose file logging to `./.kinetix/kinetix.log` |
| `--help` | - | Display help information |

---

## Examples

### Basic Startup
```bash
./kinetix
```

### Custom Screen Gaps and Padding
Run with 12px outer screen padding and 6px inner tile spacing:
```bash
./kinetix --gaps 12 --inner-gaps 6
```

### Cap Tiled Windows
Limit tiling to 4 windows maximum (additional windows remain floating):
```bash
./kinetix --max-windows 4
```

### Adjust Swap Zone Sensitivity
Set the center swap zone ratio to 50% of tile dimensions (making edge splits require dragging closer to window borders):
```bash
./kinetix --swap-zone-ratio 0.5
```

---

## Drag-and-Drop Interaction Guide

When dragging a window title bar over an existing tiled window, Kinetix divides the target tile surface into 5 collision regions:

```text
+-------------------------------------------------+
|                    TOP SPLIT                    |
|    +---------------------------------------+    |
|    |                                       |    |
| L  |                                       |  R |
| E  |               SWAP ZONE               |  I |
| F  |            (Center Region)            |  G |
| T  |                                       |  H |
|    |                                       |  T |
|    +---------------------------------------+    |
|                  BOTTOM SPLIT                   |
+-------------------------------------------------+
```

1. **Center Region (Swap Zone)**:
   - Evaluates if the cursor release point `(dropX, dropY)` falls within the middle area defined by `--swap-zone-ratio` (default: 40% of target tile width and height).
   - Swaps tile assignments between the dragged window and the target window.

2. **Edge Regions (Split Zones)**:
   - If the release point is outside the center swap zone, Kinetix determines the closest edge (Left, Right, Top, or Bottom).
   - Splits the target tile along the corresponding axis (Horizontal for Left/Right, Vertical for Top/Bottom).
   - Places the target window into one child tile and the dragged window into the opposite child tile.

---

## Troubleshooting

### KWin Bridge Issues (Plasma 6 / Wayland)
If KWin script execution is blocked, check user journal logs:
```bash
journalctl --user --since "2 minutes ago" | grep "js:"
```

To manually reset KWin script states:
```bash
qdbus org.kde.KWin /KWin reconfigure
```

### Running with Sudo Warning
Kinetix does **not** require root privileges. Running with `sudo` will break D-Bus environment variables (`DBUS_SESSION_BUS_ADDRESS`). If execution as root is intentionally required, pass `--force-sudo`.

---

## License

Kinetix is released under the GNU General Public License v3.0 (GPL-3.0). See the [LICENSE](LICENSE) file for details.
