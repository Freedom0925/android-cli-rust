# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Test Commands

```bash
cargo build --release        # Build release binary (outputs target/release/android)
cargo test --lib             # Run all unit tests (~356 tests)
cargo test <module>:: --lib  # Run tests for specific module (e.g., sdk::, vision::)
cargo test <test_name> --lib # Run specific test by name
```

## Architecture Overview

This is a pure Rust implementation of Android CLI tools, originally implemented in Kotlin. The project provides SDK management, emulator control, device interaction, and AI agent skills management.

### CLI Entry Point

`src/main.rs` contains all CLI command definitions using clap derive macros. Commands are organized as:
- Visible: `sdk`, `emulator`, `run`, `skills`, `init`, `describe`, `docs`, `update`, `info`, `create`, `screen`, `layout`, `help`
- Hidden: `device`, `template`, `upload-metrics`, `test-metrics`

### Core Modules

**SDK Management (`src/sdk/`):**
- `storage.rs` - Git-like Content-Addressable Storage (CAS): objects stored by SHA-1 hash in `storage/objects/<sha>`, archives in `storage/archives/<sha>.zip`, refs in `refs/`
- `model.rs` - Protobuf-based SDK package index (generated from `protos/android.sdk.proto`)
- `manager.rs` - High-level SDK operations (install, list, update, remove)
- `repository.rs` - Remote repository interaction (fetch index, download packages)

**Vision Processing (`src/vision/`):**
- `edges.rs` - Sobel edge detection with Otsu automatic threshold
- `cluster.rs` - Union-Find algorithm for connected component clustering, PixelCluster struct
- `image_utils.rs` - Image manipulation (copy, grayscale, draw rect/number)
- `digits.rs` - 5x3 pixel digit rendering for annotation labels

**UI Hierarchy (`src/layout/`):**
- `mod.rs` - build_tree stack algorithm for XML parsing, compute_key with sibling_index for duplicate resource_id handling
- `serializer.rs` - JSON serialization with ElementSerializer/ElementDiffSerializer
- `key.rs` - Key struct with hash_code method

**Screen Operations (`src/screen/`):**
- Screen capture via ADB
- Annotation: overlays labeled bounding boxes on detected features
- PNG+JSON format: JSON metadata appended after PNG IEND chunk
- `resolve` command substitutes `#N` coordinates from annotated screenshots

**Skills Management (`src/skills/`):**
- `location.rs` - SkillsInstallLocation enum with 42 AI agent installation paths (claude-code, cursor, gemini, copilot, windsurf, etc.)
- `manager.rs` - Install/remove skills to agent-specific directories

**Metrics (`src/metrics/`):**
- InvocationRecord and CrashRecord tracking
- Writes to local JSON files in `.android/cli/metrics/` (no network upload)

### Key Data Flows

1. **SDK Install Flow:** `manager.install()` → `repository.fetch_index()` (protobuf) → `storage.write_object()` → `download_archive()` → unzip to SDK path

2. **Screen Annotation Flow:** `capture()` → `to_grayscale()` → `sobel_edges_with_threshold()` → `find_clusters()` → `group_regions()` (depth-based filtering) → `draw_labeled_regions()` → PNG+JSON output

3. **Layout Dump Flow:** `adb shell uiautomator dump` → parse XML → `build_tree()` → `compute_key()` per node → JSON output with diff support

### Protobuf Schema

SDK package index uses protobuf defined in `protos/android.sdk.proto`. Build.rs generates Rust types via prost-build. Key types: `SdkPackage`, `RemoteSdk`, `Archive`.

### Dependencies

Key crates:
- `clap` - CLI parsing (derive macros)
- `prost` - Protobuf
- `image/imageproc` - Vision processing
- `tantivy` - Full-text search (docs module)
- `sha1/sha2` - CAS hashing
- `reqwest` - HTTP downloads

## Testing Patterns

Tests are inline in module files under `#[cfg(test)] mod tests`. Use `tempfile` for filesystem tests. Vision tests verify algorithm outputs against expected patterns.

## Kotlin-to-Rust Alignment

The implementation aligns 100% with the original Kotlin version except:
- Async operations: Rust uses synchronous calls (tokio available but not used for core flow)
- Metrics: writes local files instead of HTTP upload to analytics server

See `DIFF_REPORT.md` for detailed feature comparison.