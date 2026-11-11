# clippy

A lightweight, Outplayed-style game clipper, Valorant first. Records gameplay with the GPU encoder, detects kills from the kill feed, and (soon) clips and shares to Discord.

**Status:** milestone 1 (record + kill detection), untested on real footage.

## Requirements
- Windows, AMD GPU (AMF encoder), Valorant in borderless/windowed fullscreen
- Rust (rustup) and a full FFmpeg build on PATH

## Usage
```
cargo run --release -- --calibrate   # dump kill feed crops to tune crop/colors
cargo run --release                  # record + detect, writes recordings/session_*/events.json
```

## Roadmap
- [x] M1: capture + segmented recording + kill detection scaffold
- [ ] M1b: calibrate crop and highlight color on real footage
- [ ] M2: clip command (`ffmpeg -c copy` around events)
- [ ] M3: upload to Cloudflare R2 + Discord embed page (Worker with og:video)
- [ ] M4: tray daemon, auto-start with Valorant
- [ ] M5: Tauri editor UI (timeline, trim, upload)

## Anti-cheat note
clippy only uses OS-level screen capture and analyzes pixels, like OBS. It never injects into, hooks, or reads memory of the game.
