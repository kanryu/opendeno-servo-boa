# OpenDeno Servo Boa

An experimental Rust browser project demonstrating the technical feasibility
of OpenDeno: a browser environment that combines Servo, SpiderMonkey, and Boa
with a small Deno-style permission model.

The GTK browser component is based on Servo GTK. Ordinary page JavaScript is
still executed by Servo's SpiderMonkey runtime, while special DenoScript blocks
are evaluated by an application-owned Boa runtime.

## What is OpenDeno?

OpenDeno is the project name for an experimental, JavaScript-engine-independent
approach to Deno-style scripting. Unlike the traditional Deno runtime, which is
tightly integrated with the V8 engine, OpenDeno is intended to combine Deno's
secure execution model with multiple JavaScript engines, including Boa,
SpiderMonkey, and other standards-compliant engines.

The broader motivation for this experiment is discussed in the following
article:

- [Why Deno's Future Feels Stalled—and the Case for "OpenDeno" (Reddit)](https://www.reddit.com/r/Deno/comments/1xpx0bl/why_denos_future_feels_stalledand_the_case_for/)

The article is background material and does not define the implementation or
security guarantees of this repository.

The goal is to define the security and capability model independently from the
JavaScript engine. This makes the model applicable not only to a conventional
JavaScript command-line interpreter, but also to:

- web browsers and browser components;
- desktop applications;
- smartphone applications; and
- embedded application scripting environments.

In a conventional embedded JavaScript engine, scripts often execute with the
same operating-system privileges as the host application. This makes arbitrary
script execution difficult to adopt safely: a script may gain access to local
files, networks, processes, or other application capabilities without a clear
permission boundary.

OpenDeno addresses this problem by allowing each DenoScript execution context
to declare or receive only the capabilities it needs. The host application
controls which capabilities are exposed, while the JavaScript engine remains
responsible for language execution. A host can therefore provide a read-only
filesystem capability, for example, without granting write, network, or process
access.

This repository is an early proof of that concept. Boa is embedded as a second
engine in a Servo GTK browser, and DenoScript is given a deliberately limited
local filesystem capability: reading below the current working directory is
allowed, while writing is denied. The experiment demonstrates that the
security model can be applied independently of the page JavaScript engine.

## Features

- GTK4-based web browser widget
- Servo web engine integration
- OpenGL-accelerated rendering
- Async event handling
- HiDPI rendering at device resolution (correct `window.devicePixelRatio`)
- Page zoom (`zoom-level` property; Ctrl+/Ctrl- style)
- Navigation: `load_url`, `load_html`, reload, back/forward
- Observable `uri`, `title`, `is-loading` properties and a `load-changed` signal
- Full keyboard mapping, including function keys and Insert
- User script/style injection and a page-to-native message channel
- Script dialogs (`alert`, `confirm`, `prompt`) with native GTK dialogs
- `<input type=file>` backed by a native file chooser
- HTTP authentication and permission prompts
- Popups (`window.open` / `target=_blank`) via a `create-web-view` signal
- Native context menus on web content

## OpenDeno experiment

HTML pages can contain both ordinary page JavaScript and DenoScript:

```html
<script>
  // Executed by Servo's SpiderMonkey page runtime.
</script>

<script type="application/deno">
  const text = Deno.readTextFileSync("deno-read-test.txt");
  let writeDenied = false;
  try {
    Deno.writeTextFileSync("deno-write-test.txt", "must fail");
  } catch {
    writeDenied = true;
  }
  "read=" + text.trim() + "; writeDenied=" + writeDenied;
</script>
```

The current proof of concept provides these Deno-style capabilities:

- `Deno.readTextFileSync(path)` can read files below the browser process's
  current working directory.
- Absolute paths, `..` traversal, and paths escaping the canonical root are
  rejected.
- `Deno.writeTextFileSync(...)` is always denied.
- No unrestricted filesystem, network, subprocess, or full Deno runtime API is
  exposed.

The demonstration page reads `deno-read-test.txt` successfully and verifies
that writing is denied. The expected result is:

```text
read=This file is readable by the Boa DenoScript sandbox.; writeDenied=true
```

This is an experimental security and multi-engine demonstration, not a
production browser or a complete Deno implementation.

## Building

```bash
cargo build
```

### Windows build requirements

Building Servo on Windows is considerably more involved than building a
small standalone Rust application. The Servo dependency includes SpiderMonkey
and native graphics, font, and media components, so the host toolchain must be
assembled carefully.

The following prerequisites are required or strongly recommended:

1. Install the official MozillaBuild environment. It provides the Mozilla
   build tools and the native dependencies expected by SpiderMonkey.
2. Ensure that `mozmake.exe` is available. Depending on the MozillaBuild
   installation, it may be necessary to copy or manually add
   `C:\mozilla-build\bin\mozmake.exe` to the executable search path.
3. Use the MSVC Rust toolchain and target. SpiderMonkey is built with MSVC on
   this Windows configuration, so the Rust target must also be MSVC:

   ```powershell
   rustup toolchain install stable-x86_64-pc-windows-msvc
   rustup target add x86_64-pc-windows-msvc
   cargo build --example browser --target x86_64-pc-windows-msvc
   ```

   Do not use the GNU target for this build. Mixing a MinGW/GNU Rust target
   with the MSVC-built SpiderMonkey and native libraries can result in linker
   failures or incompatible binaries.

4. Put the rustup binaries, MozillaBuild tools, and the required native
   library tools on `PATH`. A typical setup also defines `LIBCLANG_PATH` and
   the `pkg-config` paths used by GTK and the graphics stack.
5. Enable Git's long-path support before fetching Servo. Servo's repository
   contains deeply nested Web Platform Tests paths:

   ```powershell
   git config --global core.longpaths true
   ```

6. Use a short, writable Cargo home and enable the Git command-line fetcher if
   the default Windows Cargo cache causes path or checkout problems:

   ```powershell
   $env:CARGO_HOME = "C:\cargo"
   $env:CARGO_NET_GIT_FETCH_WITH_CLI = "true"
   ```

The first build can download and compile a very large Servo dependency graph,
including SpiderMonkey. It may take a long time and can consume substantial
disk space. Errors mentioning Servo's `python/tidy/tests` fixture Cargo.toml
are emitted while Cargo inspects the repository and are not necessarily the
root cause; the useful diagnostic is usually the final dependency, linker, or
native-tool error.

After a successful build, the browser example is produced at:

```text
target\x86_64-pc-windows-msvc\debug\examples\browser.exe
```

## Running the Example

```bash
cargo run --example browser
```

The browser example opens the local DenoScript test page by default. Start its
HTTP server first:

```powershell
cd E:\Home\src\rust\opendeno-servo-boa\test-pages
python -m http.server 8000 --bind 127.0.0.1
```

Then run the browser from the project root:

```powershell
cd E:\Home\src\rust\opendeno-servo-boa
cargo run --example browser
```

The test page is available at
`http://127.0.0.1:8000/deno-script.html`. The initial URL can be overridden
with the `SERVO_GTK_HOME_URL` environment variable.

## Using as a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
servo-gtk = { path = "path/to/servo-gtk" }
```

Then use in your code. You **must** call `run_as_runner_if_requested()` as the
very first thing in `main()`. The library runs Servo in a subprocess by
re-executing your own binary; this call hands off to the Servo runner when the
process was spawned as one, and returns immediately otherwise. No separate
binary needs to be installed.

```rust
use servo_gtk::WebView;

fn main() {
    servo_gtk::run_as_runner_if_requested();

    // ... your normal application startup ...
    let webview = WebView::new();
    webview.load_url("https://example.com");
}
```

## Using from C and other languages (GObject Introspection)

In addition to the Rust crate, `servo-gtk` can be built as a C-ABI shared
library (`libservo-gtk.so`) that exposes its GObjects — `ServoGtkWebView` (a
`GtkWidget` subclass) and `ServoGtkUserContentManager` — to C and to any
language with GObject-Introspection bindings (Python, JavaScript, Vala, ...).
The Meson build also produces a pkg-config file (`servo-gtk.pc`) and
introspection data (`ServoGtk-0.1.gir` and `ServoGtk-0.1.typelib`).

### Building with Meson

```bash
meson setup builddir
meson compile -C builddir
meson test -C builddir      # headless C API smoke test
meson install -C builddir   # installs the .so, headers, servo-gtk.pc, .gir and .typelib
```

The Meson build wraps `cargo` to produce a static library and links it into the
shared library, so a Rust toolchain is still required. GObject Introspection
generation is Linux-only and requires `g-ir-scanner` (the `gobject-introspection`
package) and a display (`$DISPLAY`/`$WAYLAND_DISPLAY`); it can be toggled with
`-Dintrospection=enabled|disabled|auto`.

### Using the C API

Include the umbrella header and link with pkg-config:

```c
#include <servo-gtk/servo-gtk.h>

int main(int argc, char **argv) {
    /* MUST be the very first statement (see the runner note above). */
    servo_gtk_run_as_runner_if_requested();
    servo_gtk_init();  /* set up the GL loader */

    /* ... gtk_init(); create a ServoGtkWebView; run your application ... */
    GtkWidget *view = servo_gtk_web_view_new();
    servo_gtk_web_view_load_url(SERVO_GTK_WEB_VIEW(view), "https://example.com");
}
```

```bash
cc $(pkg-config --cflags servo-gtk) browser.c $(pkg-config --libs servo-gtk)
```

A complete C browser lives in [`examples-c/browser.c`](examples-c/browser.c).

### Using from Python (PyGObject)

```python
import gi
gi.require_version("ServoGtk", "0.1")
from gi.repository import ServoGtk

ServoGtk.run_as_runner_if_requested()
ServoGtk.init()
view = ServoGtk.WebView.new()
view.load_url("https://example.com")
```

> Note: because the shared library statically links the entire Servo engine,
> loading it lazily via `dlopen` (as PyGObject does on first use) may emit a
> `cannot allocate memory in static TLS block` warning. The introspection
> metadata still loads correctly, and C consumers that link the library at
> startup are unaffected.

## Dependencies

- GTK4
- OpenGL
- Servo web engine
- Rust toolchain
- For the C library / introspection: Meson, pkg-config, and
  `gobject-introspection` (`g-ir-scanner`)
