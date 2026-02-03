# Port Tray

A high-performance, minimalist desktop app to visualize and kill system ports.

Built with Tauri v2, Rust, and vanilla JavaScript. No frameworks, no bloat.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/try-samuel/port-tray/main/install.sh | bash
```

Then run:

```bash
findports
```

> **Note (macOS):** This app is unsigned. The installer automatically bypasses Gatekeeper, but if you download manually, run: `xattr -cr /Applications/Port\ Tray.app`

## Features

- View all listening TCP ports in a clean grid layout
- Search/filter by port number, process name, PID, or user
- Kill processes with a single click (with confirmation)
- System tray integration
- Auto-refresh every 5 seconds
- Keyboard shortcuts (Cmd+R to refresh, / to search, Esc to clear)
- Dark theme

## Platforms

- macOS (10.15+)
- Linux (deb, rpm)

Windows is not supported.

## Prerequisites

- [Rust](https://rustup.rs/) (1.70+)
- [Node.js](https://nodejs.org/) (18+)
- `lsof` command (pre-installed on macOS, `apt install lsof` on Debian/Ubuntu)

## Development

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev
```

## Build

```bash
# Build for production
npm run tauri build
```

Build outputs will be in `src-tauri/target/release/bundle/`:

- macOS: `.app` and `.dmg`
- Linux: `.deb` and `.rpm`

## Project Structure

```
port-tray/
├── src/
│   └── index.html        # Frontend (HTML + CSS + JS)
├── src-tauri/
│   ├── src/
│   │   ├── main.rs       # Entry point
│   │   └── lib.rs        # Core logic (lsof, kill)
│   ├── capabilities/
│   │   └── default.json  # Shell permissions
│   ├── icons/            # App icons
│   ├── Cargo.toml        # Rust dependencies
│   └── tauri.conf.json   # Tauri configuration
├── package.json
└── README.md
```

## How It Works

1. **Backend (Rust)**: Runs `lsof -iTCP -sTCP:LISTEN -n -P` to get listening ports
2. **Frontend (JS)**: Displays ports in a grid, handles search/filter
3. **Kill**: Sends `kill -9 <PID>` to terminate processes

## Keyboard Shortcuts

| Key            | Action                     |
| -------------- | -------------------------- |
| `Cmd/Ctrl + R` | Refresh port list          |
| `/`            | Focus search input         |
| `Escape`       | Clear search / Close modal |

## License

MIT
