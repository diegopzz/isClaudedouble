# isClaudeDouble

System tray app that tracks Claude's 2x usage rate promotion periods.

Shows a tray icon (orange when 2x is active, black otherwise) and a popup with:
- Live countdown to the next rate change
- Progress bar showing time remaining
- Windows toast notifications before transitions (customizable timing)
- 4 color themes (Dark, Light, Night, Sunset)

## Usage

Left-click the tray icon to open the popup. Right-click for the native menu with Quit.

## Schedule

- **Promotion period:** Mar 13 - Mar 28, 2026 (UTC)
- **2x active:** Weekends (all day) + weekdays outside 8 AM - 2 PM EDT
- **Standard rate:** Weekdays 8 AM - 2 PM EDT

## Build

```
cargo build --release
```

The binary is at `target/release/isclaude2x-tray.exe` (Windows) or `target/release/isclaude2x-tray` (macOS).

## Config

Settings are saved to `%APPDATA%/isclaude2x-tray/config.toml` (Windows) or `~/Library/Application Support/isclaude2x-tray/config.toml` (macOS).

## License

MIT
