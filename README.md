# Kinetix

Kinetix is an ultra-fast, zero-installation, single-binary automatic tiling window manager engine designed for Linux desktops (KDE Plasma 6 / Wayland & X11).

Built with Rust, Kinetix bridges directly into KWin and X11 display environments to deliver dynamic binary space partitioning (BSP / Dwindle), drag-and-drop window swapping & splitting, aspect-ratio-aware tile tree management, and zero-footprint lifecycle operation.

---

## Features

- **Zero Installation / Single Binary**: Distributed as a standalone compiled Rust binary with no system daemons or complex dependency trees required.
- **Wayland & X11 Dual Support**: Native dynamic bridge for **KDE Plasma 6 (Wayland)** via D-Bus KWin Scripting and **X11** via `x11rb`.
- **Aspect-Ratio-Aware Dwindle BSP**: Automatically chooses optimal horizontal or vertical split axes to preserve comfortable 16:9 / 4:3 window proportions.
- **Interactive Drag-and-Drop Tiling**:
  - **Center Drop Zone (Swap)**: Drag a window to the center of another tile to swap their positions.
  - **Multi-Directional Edge Splits (Left / Right / Top / Bottom)**: Drag a window to any edge of a tile to split that tile and insert the window on that side.
  - **Smooth Unconstrained Dragging**: Windows detach seamlessly from tile boundaries while being dragged.
- **Configurable Constraints**:
  - Set window caps (`--max-windows`).
  - Customizable inner/outer gaps (`--gaps`, `--inner-gaps`).
  - Configurable center swap threshold (`--swap-zone-ratio`).
- **Clean Restorative Exit**: Gracefully untiles windows and restores original geometry upon shutdown (`Ctrl+C` / SIGINT).

---

## Installation & Requirements

### Requirements
- **OS**: Linux (x86_64)
- **Display Server**: Wayland (KDE Plasma 6) or X11
- **Toolchain (building from source)**: Rust 1.75+ / `cargo`

### Building from Source

```bash
git clone https://github.com/your-username/kinetix.git
cd kinetix
cargo build --release
```

The compiled binary will be available at `./target/release/kinetix`.

---

## Quick Start

Run Kinetix directly from the command line:

```bash
./target/release/kinetix
```

### CLI Command Options

```bash
kinetix [OPTIONS]
```

| Option | Default | Description |
|---|---|---|
| `--layout <LAYOUT>` | `dwindle` | Layout algorithm (`dwindle`, `master-stack`, `floating`) |
| `--max-windows <N>` | `0` | Maximum number of tiled windows on screen (`0` = unlimited) |
| `--gaps <PX>` | `0` | Outer screen margin / padding in pixels |
| `--inner-gaps <PX>` | `0` | Inner gap between adjacent tiles in pixels |
| `--swap-zone-ratio <RATIO>` | `0.4` | Inner center area fraction (`0.0`-`1.0`) reserved for drag-to-swap |
| `--drag-threshold <PX>` | `20` | Drag distance threshold in pixels |
| `--restore-on-exit <BOOL>` | `true` | Restore original window geometries on exit |
| `--debug` | `false` | Enable verbose file logging (`./.kinetix/kinetix.log`) |
| `--help` | - | Print help information |

### Examples

**Run with 8px outer gaps and 4px inner gaps:**
```bash
./kinetix --gaps 8 --inner-gaps 4
```

**Limit tiling to 4 windows maximum and expand center swap area:**
```bash
./kinetix --max-windows 4 --swap-zone-ratio 0.5
```

---

## Drag-and-Drop Mechanics

When moving a window with the mouse:

```text
+------------------------------------+
|             TOP SPLIT              |
|   +----------------------------+   |
|   |                            |   |
| L |                            | R |
| E |         SWAP ZONE          | I |
| F |         (Center)           | G |
| T |                            | H |
|   |                            | T |
|   +----------------------------+   |
|            BOTTOM SPLIT            |
+------------------------------------+
```

- **Center Zone**: Swaps window positions.
- **Edges**: Splits the target tile in the drag direction and places the dragged window into the new tile cell.

---

## License

Kinetix is released under the **GNU General Public License v3.0 (GPL-3.0)**.

```text
Kinetix - Zero-installation tiling engine for Wayland and X11
Copyright (C) 2026

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
GNU General Public License for more details.
```

See the [LICENSE](LICENSE) file for details.
