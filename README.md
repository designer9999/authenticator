# Authenticator

A modern TOTP authenticator desktop app built with **Tauri v2**, **Svelte 5**, and **Rust**. Designed with Material Design 3 Expressive guidelines.

## Features

- **TOTP code generation** — RFC 6238 compliant (HMAC-SHA1, 6-digit, 30s period)
- **20+ brand icons** — Snapchat, Google, GitHub, Discord, Facebook, Instagram, and more (Simple Icons, MIT)
- **Quick paste** — paste `name:password:secret` to auto-fill all fields
- **Bulk import** — import multiple accounts from a `.txt` file (one per line)
- **Drag-and-drop** — drop a `.txt` file onto the app to import accounts
- **Edit accounts** — change issuer, name, or secret key after adding
- **Issuer picker** — tap brand chips or avatar to quickly assign/change issuer
- **Filter and search** — filter by issuer chips, search by name or issuer
- **Copy to clipboard** — click any account row to copy the current code
- **Custom titlebar** — drag to move, double-click to maximize, native window controls
- **Settings** — version info, account count, data location, change storage path, check for updates
- **Fully offline** — Roboto Flex + Material Symbols fonts bundled locally
- **M3 Expressive design** — dark theme, proper color tokens, motion springs, accessibility

## Tech Stack

| Layer    | Technology                                         |
| -------- | -------------------------------------------------- |
| Backend  | Rust + Tauri v2                                    |
| Frontend | Svelte 5 (runes) + SvelteKit                       |
| Styling  | Tailwind CSS v4 + M3 design tokens                 |
| Fonts    | Roboto Flex (variable) + Material Symbols Outlined |
| Icons    | Simple Icons (simpleicons.org)                     |
| Linting  | ESLint + Prettier                                  |

## Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/) (v20+)
- [Rust](https://rustup.rs/) (latest stable)
- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

### Development

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Or on Windows, double-click:
dev.bat
```

### Build for production

```bash
npm run tauri build
```

The installer will be in `src-tauri/target/release/bundle/`.

## Account Format

### Single account (quick paste)

```
username:password:SECRETKEY
```

### Bulk import file (.txt)

```
Alice:pass123:JBSWY3DPEHPK3PXP
Bob:pass456:KRMVATZTJFZUC4BY
Charlie:pass789:GEZDGNBVGY3TQOJQ
```

The password field is ignored — only the name and base32 secret are used.

## Data Storage

Accounts are stored in a local JSON file:

- **Windows:** `%APPDATA%/com.mjau.authenticator/accounts.json`
- **macOS:** `~/Library/Application Support/com.mjau.authenticator/accounts.json`
- **Linux:** `~/.config/com.mjau.authenticator/accounts.json`

You can change the storage location in Settings.

## License

MIT

## Credits

- [Tauri](https://tauri.app/) — desktop app framework
- [Svelte](https://svelte.dev/) — UI framework
- [Simple Icons](https://simpleicons.org/) — brand SVG icons (MIT license)
- [Material Design 3](https://m3.material.io/) — design system
- [Roboto Flex](https://fonts.google.com/specimen/Roboto+Flex) — variable font
