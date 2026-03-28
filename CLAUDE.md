# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build --release       # build
cargo run --release         # run (requires a running Qtile-Wayland session)
cargo clippy                # lint
```

No tests exist in this codebase.

## Architecture

qalttab is a Qtile alt-tab switcher overlay built with egui/eframe, running on Wayland. It renders a floating window listing open Qtile windows and lets the user switch focus with Alt+Tab.

**Data flow:**

1. **`ipc.rs`** — Unix socket server at `$XDG_CACHE_HOME/qtile/qalttab.$WAYLAND_DISPLAY`. Receives JSON messages from Qtile hooks with `message_type` (`cycle_windows` or `client_focus`) and a `windows` array. Each window entry is a `HashMap<String, String>` with keys `id`, `name`, `class`, `group_name`, `group_label`.

2. **`qaltd.rs`** — Spawns `libinput debug-events` and monitors stdout for Alt key release events. On release, sends `AppEvent::AltReleased` and calls `qtile.fire_user_hook("alt_release")` via `qtile-cmd-client`.

3. **`ui.rs`** — Main egui app (`AsyncApp`). Both listeners above send `AppEvent` variants over an `UnboundedSender`. The `update()` loop drains the channel and either shows/hides the overlay or updates the window list. Also spawns a background task to discover qalttab's own Qtile window ID (needed for `hide`/`place` calls). Window management (show, hide, resize, center, focus, kill) is done via `InteractiveCommandClient::call` (qtile-cmd-client IPC).

4. **`config.rs`** — YAML config loaded via `confy` from the default config dir (`qalttab/config.yaml`). Covers fonts, colors, icon themes, sizes, and which UI items to show (`icon`, `name`, `group_name`, `group_label`) and layout orientation.

**Qtile side:** Requires an `alttab_hooks` module in the Qtile config that sends IPC messages to the socket and handles the `alt_release` user hook. See the README link for the reference implementation.

**Single-instance guard:** `run_ui()` checks running `qalttab` processes via `sysinfo` and exits if 4+ instances are found (accounts for the process tree depth).
