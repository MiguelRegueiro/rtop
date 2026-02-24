# rtop

`rtop` is a fast, terminal-based system monitor written in Rust, inspired by `btop` and `htop`.

It provides real-time monitoring for CPU, GPU, memory, network, disks, and processes with polished bars, smooth graphs, and keyboard-first process controls.


## Highlights

- **CPU panel**
  - Global CPU usage
  - Per-core usage and frequency
  - CPU temperature and power when available
- **GPU panel**
  - NVIDIA telemetry via NVML (usage, temperature, memory, power)
  - Intel iGPU telemetry on Linux with layered fallbacks
- **Memory panel**
  - RAM used/cached/total stacked bar
  - SWAP usage bar
  - Smoothed memory history graph
- **Network panel**
  - RX/TX live rates and totals
  - Smoothed history chart with numeric axes
  - Interface cycling
- **Disk panel**
  - Deduplicated mounted volume view
  - Used/total summary
- **Process panel**
  - Sorting: CPU, memory, PID, name
  - Tree mode
  - Search/filter (`Shift+S`)
  - Safe terminate flow (`k` -> confirm dialog)
- **UI/UX**
  - Smooth visual updates and polished usage bars
  - Multiple themes
  - Bottom key-hint bar and top status bar

## Platform Support

- **Linux**: best support, including most advanced sensor and GPU telemetry paths.
- **macOS/Windows**: core monitoring works via `sysinfo`; some sensor/telemetry fields may be unavailable.

## Requirements

- Rust stable toolchain
- Cargo
- A terminal with UTF-8 and Unicode drawing support

Optional:
- NVIDIA driver with NVML available for NVIDIA GPU telemetry

## Installation

Clone and build:

```bash
git clone https://github.com/MiguelRegueiro/rtop.git
cd rtop
cargo build --release
```

Run:

```bash
cargo run --release
```

Or run the built binary:

```bash
./target/release/rtop
```

## Usage

The UI layout is:

- **Top bar**: host, uptime, load average, clock, active theme
- **Left column**: CPU, GPU, memory
- **Center column**: process list/tree
- **Right column**: network, disk
- **Bottom bar**: key hints

## Keybindings

### Global

| Key | Action |
|---|---|
| `q` | Quit |
| `Esc` | Quit when no modal is open |
| `Up` / `Down` | Move process selection |
| `s` | Cycle process sort mode |
| `Shift+S` | Start process search/filter |
| `k` | Open terminate confirmation for selected process |
| `T` | Toggle process tree/list view |
| `i` | Cycle network interface |
| `t` | Cycle theme |
| `w` | Save current theme setting |

### Search Mode

| Key | Action |
|---|---|
| `Enter` | Apply search |
| `Esc` | Cancel search |
| `Backspace` | Delete character |
| Printable keys | Update filter text |

### Kill Confirmation

| Key | Action |
|---|---|
| `Left` / `Right` | Toggle `Yes`/`No` |
| `Tab` / `Shift+Tab` | Toggle `Yes`/`No` |
| `Enter` | Confirm selected option |
| `Esc` | Cancel dialog |

`Yes` is preselected.

## Configuration

The theme is persisted when you press `w`.

Config path:
- Linux/macOS: `$XDG_CONFIG_HOME/rtop/config.toml` or `~/.config/rtop/config.toml`
- Windows: `%APPDATA%\\rtop\\config.toml`

Current saved field:
- `color_scheme`

## Intel iGPU Notes (Linux)

`rtop` collects Intel iGPU data from multiple sources and falls back when direct metrics are unavailable:

- DRM sysfs/hwmon
- i915 debugfs (if enabled and accessible)
- thermal zones
- `/proc/meminfo` proxy for shared-memory estimates

Depending on your distro and setup, temperature/power/memory sources may require elevated permissions or specific kernel interfaces.

## Troubleshooting

- **No NVIDIA metrics**: confirm NVIDIA driver/NVML is installed and accessible.
- **Missing Intel iGPU fields**: check debugfs availability and permissions; some fields are kernel/platform dependent.
- **No temperature or power values**: not all systems expose these sensors through standard interfaces.

## Development

```bash
cargo fmt
cargo check
cargo test
```

## License

MIT
