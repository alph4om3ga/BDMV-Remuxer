---
name: Rust test linker on Replit
description: Workaround for Rust test binaries failing to link against zlib in this workspace environment.
---

When `cargo test` fails at the final link step with `cannot find -lz`, a zlib library can exist in the Nix store but not be visible to the linker's search paths. The store may contain both 32-bit and 64-bit copies, so confirm the selected `libz.so` is x86-64 before setting `LIBRARY_PATH`.

**Why:** The Rust test binary may directly link native Tauri dependencies, and this workspace's native library search path can omit zlib.

**How to apply:** Locate the installed zlib `lib` directory and include it in `LIBRARY_PATH` for linking and `LD_LIBRARY_PATH` when running tests. This is a local verification workaround; do not hardcode a Nix store hash in project configuration.
