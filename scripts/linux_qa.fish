#!/usr/bin/env fish
# Linux-only native release smoke checks. Requires Mathematica and WSTP.
# Run from any directory: fish /path/to/mathematica-mcp/scripts/linux_qa.fish

set -l root (realpath (dirname (status --current-filename))/..)
cd "$root"
or exit 1

echo "=== Mathematica MCP: native Linux QA ==="
echo "Repository: $root"
echo "Revision: "(git rev-parse --short HEAD 2>/dev/null)

if test (uname -s) != Linux
    echo "ERROR: this QA script is Linux-only." >&2
    exit 2
end

for exe in cargo rustc python3
    if not command -sq $exe
        echo "ERROR: required executable not found: $exe" >&2
        exit 2
    end
end

if set -q WOLFRAM_KERNEL_PATH
    echo "WOLFRAM_KERNEL_PATH is explicitly set."
    if not test -x "$WOLFRAM_KERNEL_PATH"
        echo "ERROR: configured Wolfram kernel is missing or not executable." >&2
        exit 2
    end
else if command -sq WolframKernel
    echo "WolframKernel is available on PATH."
else if command -sq MathKernel
    echo "MathKernel is available on PATH."
else
    echo "NOTE: no kernel on PATH; native Wolfram discovery may still find it."
end

echo "Rust compiler: "(rustc --version)
echo "Cargo: "(cargo --version)
echo "Using the current system Rust toolchain (no repo-pinned nightly)."

if set -q WAYLAND_DISPLAY
    echo "Graphical environment: Wayland display present."
else if set -q DISPLAY
    echo "Graphical environment: X11 display present."
else
    echo "WARNING: neither WAYLAND_DISPLAY nor DISPLAY is present."
    echo "WARNING: Wolfram PNG export may require a graphical front end."
end

echo "=== Native WSTP build ==="
cargo build --locked
or exit 1

echo "=== Rust unit tests (native WSTP link) ==="
cargo test --locked
or exit 1

echo "=== Lint (native WSTP link) ==="
cargo clippy --locked --all-targets -- -D warnings
or exit 1

echo "=== Real-kernel MCP protocol smoke tests ==="
python3 -u scripts/mcp_smoke.py
or exit 1

echo "=== Shared HTTP daemon: native tenant isolation smoke ==="
python3 -u scripts/http_smoke.py
or exit 1

echo "=== Independent Rust library consumer (native WSTP link) ==="
# This is a separate Cargo package resolution, not a crate-local example.
cargo run --manifest-path tests/consumer/Cargo.toml
or exit 1

echo "PASS: Linux native WSTP build, unit tests, lint, stdio and HTTP MCP smoke, and Rust library consumer"
