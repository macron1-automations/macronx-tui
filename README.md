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
| `j` / `Down` | Scroll body down (or move attachment selection when sidebar is focused) |
| `k` / `Up` | Scroll body up (or move attachment selection when sidebar is focused) |
| `PgUp` / `PgDn` | Scroll body by page |
| `g` | Scroll body to top |
| `G` | Scroll body to bottom |
| `Tab` | Toggle focus between body and sidebar |
| `Shift+Tab` / `h` / `l` | Switch sidebar tab (Metadata / Attachments) |
| `f` | Open fullscreen image viewer for the attached image |
| `o` | Open selected attachment with the system's default app (also works in the image viewer) |
| `space` | Play / pause attached audio |
| `←` / `→` | Seek audio ±5s (or switch sidebar tab when no audio is loaded) |
| `+` / `-` | Audio volume |
| `q` | Quit |

## Detail View Sidebar

The detail view splits the body area into a main scrollable column (~80%) and a sidebar (~20%). The sidebar has two tabs:

- **Metadata** — id, source, tag, creation time, attachment count, and summary.
- **Attachments** — list of files with type tag (`[IMG]`, `[AUD]`, `[FILE]`) and size. Selecting an image lazily downloads and previews it inline; selecting an audio file shows an inline mini-player with elapsed/total time, a waveform overview with playhead, and a live level visualization.

### Image Viewer

Pressing `f` anywhere in the detail view opens the attached image fullscreen (fit to screen). Then:

- `o` opens the original file with the system's default image app
- `f` or `Esc` closes

Images render through native terminal graphics protocols (Kitty, iTerm2, Sixel) via `ratatui-image`, falling back to unicode halfblocks on terminals without graphics support.

### Inline Audio Playback

Pressing `space` anywhere in the detail view plays/pauses the attached audio via `rodio`. While playing, `←`/`→` seek ±5 seconds and `+`/`-` adjust volume. Playback continues while browsing tabs and stops when leaving the inbox.

## Body Markdown Rendering

The detail screen renders the inbox body with a small, dependency-free markdown parser (`src/ui/inbox_show.rs`). It is intentionally light and covers the common cases:

- **Headings** — lines starting with `#` (optionally indented) render bold and yellow.
- **Fenced code blocks** — lines between ` ``` ` or `~~~` fences render green. `**` inside code is treated literally, so markup stays readable.
- **Blockquotes** — lines starting with `>` render blue.
- **Inline bold** — text wrapped in `**…**` renders bold and the `**` markers are removed, e.g. `this is **important**` shows as: this is **important**.
- **Everything else** — plain white text.

Inline `**` is parsed before word-wrapping, so an emphasized phrase that wraps across two rows stays bold with no markers reappearing at the seam. An unmatched `**` (no closing pair on the same line) is shown literally. The raw `#`, `>`, and fence markers stay visible; only `**` is stripped.

Word-wrapping is width-aware (via `unicode-width`), and the body scrolls vertically with `j`/`k`, `PgUp`/`PgDn`, and `g`/`G`.

## Terminal Performance

The interface is built to feel fast. For the best experience, use a modern GPU-rendered terminal such as [Ghostty](https://ghostty.org/), especially when working with large inbox lists or dense JSON payloads.

## Configuration

`macronx-tui` reads configuration from environment variables:

| Variable | Required | Default | Description |
| --- | --- | --- | --- |
| `MACRONX_API_TOKEN` | Yes | none | Bearer token sent to the Macronx API |
| `MACRONX_API_URL` | No | `http://localhost:5000` | Base URL for the Macronx API |
