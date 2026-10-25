---
name: WebKit virtual displays
description: EGL and software-rendering setup for installed GTK/WebKit apps in this Replit runner.
---

The default EGL manifest can select Mesa 26 even when a packaged WebKitGTK app uses glibc 2.40; Mesa 26's driver needs glibc 2.42 and WebKit aborts with `EGL_BAD_PARAMETER`. Mesa 25 from the project's Nix channel works with this app.

For a visible installed UI on the provided X11 display, point `__EGL_VENDOR_LIBRARY_FILENAMES` and `LIBGL_DRIVERS_PATH` to the same Mesa 25 store path, set `LIBGL_ALWAYS_SOFTWARE=1` and `GALLIUM_DRIVER=llvmpipe`, and launch with `GDK_BACKEND=x11`, `EGL_PLATFORM=x11`, `WEBKIT_DISABLE_COMPOSITING_MODE=1`, `WEBKIT_DISABLE_DMABUF_RENDERER=1`, and `WEBKIT_HARDWARE_ACCELERATION_POLICY=never`. Resolve the Mesa path from the current Nix store; do not copy its hash into scripts.

Nested Weston Wayland produced a negative WebKit viewport for this app, and its fullscreen shell did not provide the shell interface GTK requires; prefer direct X11 for visual checks in this runner.

**Why:** the system's default EGL driver is newer than the packaged WebKit runtime, and merely having a virtual display does not ensure the app is rendered correctly.

**How to apply:** when validating an installed UI, select the compatible Mesa 25 manifest and software driver explicitly, then confirm the actual screenshot and app viewport before relying on the display setup. Native GTK folder dialogs also require `GSETTINGS_SCHEMA_DIR` to point at the installed GTK 3 package's compiled schemas, plus `GSETTINGS_BACKEND=memory`; otherwise opening the picker aborts with a missing `org.gtk.Settings.FileChooser` schema error. Resolve the schema directory from the current Nix store rather than hardcoding its path.
