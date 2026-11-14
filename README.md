# Dock

A lumabar-style taskbar replacement for Windows 11, built with Tauri 2, React 19 and TypeScript (with a Rust backend on the `windows` crate).

Dock hides the Windows taskbar and puts a glass dock in its place. It shows your pinned and open apps, widgets and quick settings, and it can put the taskbar back with one click.

> **Windows 11 only.** Dock calls Win32 APIs directly and doesn't build or run on macOS or Linux.

## Features

- **Apps:** pinned taskbar apps, open windows, folders of apps, jump lists, window hover previews, a Windows-style Alt+Tab switcher, and Win+1..9 to open dock apps
- **Glass look:** a liquid-glass dock with wallpaper refraction and several styles, sizes and layouts (including a split dock and auto-hide)
- **Widgets:** clock, weather, calendar, music, system stats, Wi-Fi, Bluetooth, brightness, focus timer, notes, to-dos, kanban, links, dev servers, card widgets and more, as dock items, floating widgets or a dashboard
- **Command bar:** search apps, run calculations, switch workspaces and trigger actions from the keyboard
- **Automations:** meeting reminders, focus music and sunset dark mode
- **Multi-monitor:** a dock on every screen, or just the one you choose
- **Safe exit:** Dock-only mode, a quick switch back to the Windows taskbar, and an installer that restores the taskbar when Dock is uninstalled

API keys for account widgets are stored in Windows Credential Manager, never in files.

## Build

You need [Node.js](https://nodejs.org), [pnpm](https://pnpm.io), [Rust](https://rustup.rs) and the [Tauri prerequisites for Windows](https://tauri.app/start/prerequisites/) (WebView2 and the MSVC build tools).

```bash
pnpm install
```

Run it in development mode:

```bash
pnpm tauri dev
```

Build the installer (the output goes to `src-tauri/target/release/bundle/`):

```bash
pnpm tauri build
```

Other scripts: `pnpm test` (Vitest), `pnpm test:rust` (cargo tests), `pnpm check` (types, lint, format and tests) and `pnpm gen:types` (regenerates the TypeScript types from the Rust ones with ts-rs).

## Settings

Settings live in `%APPDATA%\com.dhyey.dock`, outside the repo.

## License

MIT. See [LICENSE](LICENSE).
