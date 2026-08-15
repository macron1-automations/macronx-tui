# macronx-tui

A terminal user interface counterpart to [macronx](https://github.com/ninja-in-brazil/macronx).

`macronx-tui` is designed for fast, keyboard-driven work against Macronx. The initial focus is inbox workflows: listing inboxes, inspecting inbox details, and refreshing the inbox list.

## Status

Early, focused, and intentionally small. The current TUI is useful for processing inbox workflows and will grow alongside the Macronx API surface.

## Requirements

- Rust 2021 toolchain
- A running Macronx API
- `MACRONX_API_TOKEN` set in your environment

Optional:

- `MACRONX_API_URL`, if the API is not running at `http://localhost:5000`

## Usage

```sh
export MACRONX_API_URL=http://localhost:5000
export MACRONX_API_TOKEN=your-token

cargo run
```

## Controls

| Key | Action |
| --- | --- |
| `j` / `Down` | Move down |
| `k` / `Up` | Move up |
| `g` | Jump to first inbox |
| `G` | Jump to last inbox |
| `Shift+J` | Filter by next tag |
| `Shift+K` | Filter by previous tag |
| `Enter` | Open selected inbox |
| `r` | Refresh inboxes |
| `Esc` / `Backspace` | Return from detail screen |
| `q` | Quit |

## Terminal Performance

The interface is built to feel fast. For the best experience, use a modern GPU-rendered terminal such as [Ghostty](https://ghostty.org/), especially when working with large inbox lists or dense JSON payloads.

## Configuration

`macronx-tui` reads configuration from environment variables:

| Variable | Required | Default | Description |
| --- | --- | --- | --- |
| `MACRONX_API_TOKEN` | Yes | none | Bearer token sent to the Macronx API |
| `MACRONX_API_URL` | No | `http://localhost:5000` | Base URL for the Macronx API |
