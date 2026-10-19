---
name: Rust test linker on Replit
description: Workaround for Rust test binaries failing to link against zlib in this workspace environment.
---

When `cargo test` fails at the final link step with `cannot find -lz`, a zlib library can exist but not be visible to the linker's search paths. The Nix store may contain both 32-bit and 64-bit copies, so confirm the selected library is x86-64 before setting `LIBRARY_PATH`. A usable library may have only a versioned filename (`libz.so.1.x`) and no linker-name `libz.so`; in that case, expose it through a temporary directory with `libz.so` and `libz.so.1` symlinks.

**Why:** The Rust test binary may directly link native Tauri dependencies, and this workspace's native library search path can omit zlib.

**How to apply:** Locate and verify an x86-64 zlib shared library, make linker-name symlinks in a temporary directory if needed, then include that directory in `LIBRARY_PATH`. Add `LD_LIBRARY_PATH` only if the test executable cannot load zlib at runtime and the override is compatible with the Nix toolchain; overriding it with system paths can interfere with Nix tools. This is a local verification workaround; do not hardcode a Nix store hash in project configuration.
