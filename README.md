# Speak Up

Development paused.

This project was an attempt to build a system-wide AI speech assistant using Rust and Tauri.
I chose to pause development and focus on smaller projects while gaining more experience with desktop systems programming.

## Overview

Speak Up is an experimental desktop dictation assistant. The intended workflow was:

1. Start recording from a global hotkey or tray menu.
2. Capture microphone audio.
3. Send audio to a local backend over a WebSocket IPC channel.
4. Transcribe speech with Whisper or a cloud ASR provider.
5. Clean the transcript with local rules or an AI cleaner.
6. Inject the final text into the active application.

The codebase is not production-ready. It contains a working Rust workspace structure and several implemented building blocks, but platform support and provider behavior are incomplete.

## Workspace Layout

- `speak-up-core`: shared data types, settings structs, IPC messages, and error types.
- `speak-up-client`: Tauri 2 desktop client with tray integration, settings UI, global hotkeys, audio capture, overlay state, and text injection modules.
- `speak-up-backend`: local backend process with a WebSocket server, session management, ASR provider abstraction, transcript cleanup, model download metadata, and SQLite-backed dictation history.

## Current Capabilities

- Rust workspace with `cargo` and `make` commands for build, check, test, lint, and formatting.
- Tauri client shell with first-run wizard and settings UI files.
- Global hotkey parsing and registration through `global-hotkey`.
- Microphone capture through `cpal`.
- Tray state for idle, recording, and processing states.
- WebSocket communication between client and backend using bincode-encoded IPC messages.
- Local Whisper path through `whisper-rs` behind the `local-asr` feature.
- OpenAI Whisper transcription path through the audio transcription API.
- Rule-based local transcript cleanup with filler-word removal, dictionary replacements, punctuation, and capitalization.
- OpenAI and Anthropic transcript cleaner implementations.
- SQLite history storage for dictation results.
- Platform-specific text injection modules for Linux, macOS, and Windows.

## Known Limitations

- The project is paused and should be treated as an experiment.
- Default transcription may fall back to a mock Whisper implementation when local ASR is unavailable or not configured.
- Deepgram ASR is declared in the shared provider types but is not implemented.
- Auto-mute is implemented for Linux only; macOS and Windows currently return unsupported behavior.
- Desktop permissions, packaging, and platform-specific reliability need more work.
- The system has not been hardened for production use, long-running background operation, or user-facing distribution.

## Development

Requirements depend on the target platform and enabled features. At minimum, this is a Rust workspace with a Tauri 2 client.

Common commands:

```sh
make check
make test
make lint
make build
```

Run individual components:

```sh
make run-backend
make run-client
```

The backend listens on `127.0.0.1:9876` by default. The client can override the port with the `SPEAK_UP_PORT` environment variable.

Local Whisper support uses the default `local-asr` feature in `speak-up-backend` and depends on `whisper-rs` and a compatible Whisper model file. Without a configured model, the backend can fall back to mock transcription behavior.
