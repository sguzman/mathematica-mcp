#!/usr/bin/env python3
"""Short Linux Wolfram 15 graphics diagnostic without WSTP or a Rust rebuild.

Each kernel runs in its own process group and is killed on timeout. The probe
never writes test graphics to the filesystem; all exports stay in memory.
"""
from __future__ import annotations

import os
import shutil
import signal
import subprocess
import sys
from pathlib import Path

from install_image_metadata_tools import paclet_is_installed

KERNEL = shutil.which("WolframKernel") or shutil.which("MathKernel")

CASES = [
    (
        "kernel arithmetic",
        'Print["MCP_PROBE=" <> ToString[1 + 1, InputForm]]; Quit[]',
        8,
        {},
    ),
    (
        "symbolic Plot (no rasterizer)",
        'Print["MCP_PROBE=" <> ToString[Head[Plot[x, {x, 0, 1}]], InputForm]]; Quit[]',
        8,
        {},
    ),
    (
        "direct PNG export with inherited environment",
        'Print["MCP_PROBE=" <> ToString[Head[ExportByteArray[Plot[x, {x, 0, 1}], "PNG"]], InputForm]]; Quit[]',
        15,
        {},
    ),
    (
        "direct PNG export with offscreen Wolfram front end",
        'Print["MCP_PROBE=" <> ToString[Head[ExportByteArray[Plot[x, {x, 0, 1}], "PNG"]], InputForm]]; Quit[]',
        20,
        {"WOLFRAMNB_INIT": "-platform offscreen"},
    ),
]


def attempt(label: str, expression: str, seconds: int, environment: dict[str, str]) -> tuple[str, str]:
    print(f"=== {label} (max {seconds}s) ===", flush=True)
    env = os.environ.copy()
    env.update(environment)
    process = subprocess.Popen(
        [KERNEL, "-noprompt", "-run", expression],
        cwd=Path(__file__).resolve().parents[1],
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        start_new_session=True,
        text=True,
        errors="replace",
    )
    try:
        output, _ = process.communicate(timeout=seconds)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        output, _ = process.communicate()
        print(output[-4000:], flush=True)
        print(f"TIMEOUT: {label} ({seconds}s); process group killed", flush=True)
        return "TIMEOUT", output

    print(output[-4000:], flush=True)
    print(f"Exit status: {process.returncode}", flush=True)
    if process.returncode != 0:
        return "FAILED", output
    if label == "kernel arithmetic":
        return ("PASS" if "MCP_PROBE=2" in output else "FAILED"), output
    if label.startswith("symbolic Plot"):
        return ("PASS" if "MCP_PROBE=Graphics" in output else "FAILED"), output
    return ("PASS" if "MCP_PROBE=ByteArray" in output else "FAILED"), output


def main() -> int:
    print(f"Wolfram kernel: {KERNEL or '(not on PATH)'}", flush=True)
    print(f"DISPLAY present: {bool(os.environ.get('DISPLAY'))}", flush=True)
    print(f"WAYLAND_DISPLAY present: {bool(os.environ.get('WAYLAND_DISPLAY'))}", flush=True)
    print(f"QT_QPA_PLATFORM: {os.environ.get('QT_QPA_PLATFORM', '(unset)')}", flush=True)
    if KERNEL is None:
        print("ERROR: WolframKernel and MathKernel are absent from PATH", flush=True)
        return 2

    print("Checking required Wolfram image metadata paclet (no network access)...", flush=True)
    try:
        installed = paclet_is_installed(KERNEL)
    except Exception as exc:
        print(f"ERROR: ImageMetadataTools check failed: {exc}", flush=True)
        return 2
    if not installed:
        print("BLOCKED: ImageMetadataTools is absent; Wolfram 15 would auto-download it during PNG export.", flush=True)
        print("Install explicitly once: python3 -u scripts/install_image_metadata_tools.py --install", flush=True)
        print("Skipping PNG probes to avoid repeated implicit GitHub downloads.", flush=True)
        return 3

    results: list[tuple[str, str]] = []
    for label, expression, timeout, environment in CASES:
        outcome, _ = attempt(label, expression, timeout, environment)
        results.append((label, outcome))
        if outcome != "PASS" and label in ("kernel arithmetic", "symbolic Plot (no rasterizer)"):
            print("Basic kernel operation failed; stopping further probes.", flush=True)
            break

    print("=== Probe summary ===", flush=True)
    for label, outcome in results:
        print(f"{outcome}: {label}", flush=True)

    print("This probes Wolfram standalone rendering, not MCP/WSTP.", flush=True)
    states = dict(results)
    basic_ok = (
        states.get("kernel arithmetic") == "PASS"
        and states.get("symbolic Plot (no rasterizer)") == "PASS"
    )
    png_ok = any(
        outcome == "PASS"
        for label, outcome in results
        if label.startswith("direct PNG export")
    )
    if basic_ok and png_ok:
        print("PASS: at least one standalone graphics export method works", flush=True)
        return 0
    print("FAIL: no working standalone graphics export method found", flush=True)
    return 1


if __name__ == "__main__":
    sys.exit(main())
