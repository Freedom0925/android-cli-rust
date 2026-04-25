# Android CLI (Rust)

Pure Rust implementation of Android development tools CLI.

## Features

- **SDK Management**: Install, list, update, and remove Android SDK packages
- **Emulator Control**: Create, start, stop, and manage Android Virtual Devices (AVDs)
- **Device Operations**: Install APKs, run shell commands, port forwarding via ADB
- **Screen Capture**: Screenshot with UI element annotation and coordinate resolution
- **UI Hierarchy**: Dump and diff UI layout trees from connected devices
- **Project Creation**: Create new Android projects from templates
- **AI Agent Skills**: Install and manage skills for 42+ AI coding assistants
- **Documentation Search**: Search and fetch Android developer documentation

## Installation

```bash
cargo build --release
```

The binary will be at `target/release/android`.

## Quick Start

```bash
# Initialize environment
android init

# Install SDK packages
android sdk install "build-tools;34.0.0" "platforms;34" "platform-tools"

# List available packages
android sdk list --all

# Create and start emulator
android emulator create --profile pixel_6
android emulator start pixel_6_api34

# Run app on device
android run --apks app-debug.apk

# Capture annotated screenshot
android screen capture --annotate -o screenshot.png

# Dump UI hierarchy
android layout --pretty

# Create new project
android create AndroidCompose --name "My App" --minSdk 24

# Search documentation
android docs search "RecyclerView"
```

## Commands

| Command | Description |
|---------|-------------|
| `sdk` | SDK package management |
| `emulator` | AVD management |
| `run` | Install and run APK on device |
| `device` | Device operations (hidden) |
| `screen` | Screenshot with annotation |
| `layout` | UI hierarchy dump |
| `create` | Create new project |
| `describe` | Analyze project or SDK |
| `docs` | Documentation search |
| `skills` | AI agent skills management |
| `init` | Initialize environment |
| `info` | Print environment info |
| `update` | Self-update CLI |

## AI Agent Skills Support

Supports skills installation for 42 AI coding assistants including:

- Claude Code (`claude-code`)
- Cursor (`cursor`)
- Gemini CLI (`gemini`)
- GitHub Copilot (`github-copilot`)
- Windsurf (`windsurf`)
- Continue (`continue`)
- And many more...

```bash
# List installed skills
android skills list

# Install skill for specific agent
android skills add --skill android-cli --agent claude-code,cursor

# Remove skill
android skills remove --skill android-cli --agent cursor
```

## Architecture

### SDK Storage (Git-like CAS)

- **Objects**: `~/.android/cli/storage/objects/<sha>` - Package metadata
- **Archives**: `~/.android/cli/storage/archives/<sha>.zip` - Downloaded packages
- **Refs**: `~/.android/cli/refs/` - References (head, remote, channels)

### Vision Processing Pipeline

```
Screenshot → Grayscale → Sobel Edges → Otsu Threshold → Clustering → Region Groups → Annotation
```

### Screen Annotation Format

PNG file with JSON metadata appended after IEND chunk. Use `android screen resolve` to extract coordinates.

## Testing

```bash
cargo test --lib
```

Currently 356+ tests passing.

## Comparison with Kotlin Original

| Feature | Kotlin | Rust |
|---------|--------|------|
| CLI Commands | 17 | 17 (100% aligned) |
| Core Algorithms | ✓ | ✓ (100% aligned) |
| Vision/Layout/Screen | ✓ | ✓ |
| SDK/Skills/Metrics | ✓ | ✓ |
| Async Operations | Coroutines | Synchronous |
| Metrics Upload | HTTP POST | Local files |

## License

MIT