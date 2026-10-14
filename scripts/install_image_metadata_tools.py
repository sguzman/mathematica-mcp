#!/usr/bin/env python3
"""Explicit, one-time installation of Wolfram 15's ImageMetadataTools paclet.

Normal builds and smoke tests never invoke this installer. A --check run is
read-only. --install downloads the official 6.9 MiB release to the user's
persistent XDG cache, supports resume, verifies its SHA-256, and installs it
into Wolfram's own paclet storage.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import time
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

PACLET_NAME = "ImageMetadataTools-1.0.0.paclet"
DOWNLOAD_URL = (
    "https://github.com/WolframResearch/ImageMetadataTools/releases/"
    "download/v1.0.0/" + PACLET_NAME
)
EXPECTED_BYTES = 7_225_598
EXPECTED_SHA256 = "ae672f03c44a4720cd9b360736bb032e2f908e0ca09c062fc844554f6c94c928"


def kernel_eval(kernel: str, expression: str, seconds: int = 30) -> str:
    proc = subprocess.Popen(
        [kernel, "-noprompt", "-run", expression],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        start_new_session=True,
        text=True,
        errors="replace",
    )
    try:
        output, _ = proc.communicate(timeout=seconds)
    except subprocess.TimeoutExpired as exc:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        output, _ = proc.communicate()
        raise RuntimeError(f"WolframKernel timed out after {seconds}s: {output[-2000:]}") from exc
    print(output[-3000:].strip(), flush=True)
    if proc.returncode != 0:
        raise RuntimeError(f"WolframKernel exited with status {proc.returncode}")
    return output


def paclet_is_installed(kernel: str) -> bool:
    output = kernel_eval(
        kernel,
        'If[PacletFind["ImageMetadataTools"] === {}, '
        'Print["MCP_PACLET_MISSING"], Print["MCP_PACLET_PRESENT"]]; Quit[]',
        20,
    )
    if "MCP_PACLET_PRESENT" in output:
        return True
    if "MCP_PACLET_MISSING" in output:
        return False
    raise RuntimeError("Could not determine ImageMetadataTools installation state")


def cache_path() -> Path:
    root = Path(os.environ.get("XDG_CACHE_HOME") or Path.home() / ".cache")
    return root / "mathematica-mcp" / PACLET_NAME


def digest_ok(path: Path) -> bool:
    if not path.is_file() or path.stat().st_size != EXPECTED_BYTES:
        return False
    digest = hashlib.sha256()
    with path.open("rb") as infile:
        while chunk := infile.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest() == EXPECTED_SHA256


def download(path: Path) -> None:
    if digest_ok(path):
        print(f"Verified cached paclet: {path}", flush=True)
        return
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    partial = path.with_name(path.name + ".part")
    if partial.exists() and partial.stat().st_size > EXPECTED_BYTES:
        partial.unlink()
    print(f"Official WolframResearch release: {DOWNLOAD_URL}", flush=True)
    print(f"Expected download: {EXPECTED_BYTES:,} bytes (6.9 MiB)", flush=True)
    print(f"Persistent cache: {path}", flush=True)

    for attempt in range(1, 7):
        start = partial.stat().st_size if partial.exists() else 0
        if start == EXPECTED_BYTES:
            if digest_ok(partial):
                partial.replace(path)
                return
            print("Checksum mismatch in cached partial file; restarting.", flush=True)
            partial.unlink()
            start = 0

        request = Request(
            DOWNLOAD_URL,
            headers={
                "User-Agent": "mathematica-mcp-linux-installer/1.0",
                **({"Range": f"bytes={start}-"} if start else {}),
            },
        )
        print(f"Attempt {attempt}/6: starting at {start:,} bytes", flush=True)
        try:
            with urlopen(request, timeout=40) as response:
                status = response.status
                if start and status != 206:
                    # A server that ignored Range sends the entire object.
                    print("Server did not resume; restarting from byte zero.", flush=True)
                    start = 0
                mode = "ab" if start else "wb"
                done = start
                last_log = 0.0
                with partial.open(mode) as outfile:
                    while True:
                        block = response.read(64 * 1024)
                        if not block:
                            break
                        outfile.write(block)
                        done += len(block)
                        now = time.monotonic()
                        if now - last_log >= 3 or done >= EXPECTED_BYTES:
                            print(
                                f"Downloaded {done:,}/{EXPECTED_BYTES:,} "
                                f"({done * 100 / EXPECTED_BYTES:.1f}%)",
                                flush=True,
                            )
                            last_log = now
            if digest_ok(partial):
                partial.replace(path)
                print("SHA-256 verified; download complete.", flush=True)
                return
            if partial.stat().st_size >= EXPECTED_BYTES:
                print("Downloaded object failed checksum; retrying from zero.", flush=True)
                partial.unlink()
                continue
            print("Connection closed before download completed; will resume.", flush=True)
        except (OSError, URLError, HTTPError) as exc:
            print(f"Download interrupted: {exc}", flush=True)
        if attempt < 6:
            time.sleep(min(2 * attempt, 10))
    raise RuntimeError(
        f"Download incomplete after six attempts. Resume later by running "
        f"the same --install command; partial bytes remain in {partial}."
    )


def install(kernel: str, path: Path) -> None:
    # json.dumps creates a valid quoted Wolfram string for ordinary POSIX paths.
    path_argument = json.dumps(str(path))
    print("Installing checksum-verified local paclet into Wolfram...", flush=True)
    kernel_eval(
        kernel,
        f'PacletInstall[{path_argument}]; '
        'If[PacletFind["ImageMetadataTools"] === {}, '
        'Print["MCP_INSTALL_FAILED"]; Quit[3], '
        'Print["MCP_INSTALL_OK"]; Quit[]]',
        75,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_mutually_exclusive_group(required=True)
    actions.add_argument("--check", action="store_true", help="Check installed paclet (no writes or downloads)")
    actions.add_argument("--install", action="store_true", help="Explicitly download and permanently install the official paclet if absent")
    args = parser.parse_args()
    kernel = shutil.which("WolframKernel") or shutil.which("MathKernel")
    if kernel is None:
        print("ERROR: WolframKernel/MathKernel not on PATH.", flush=True)
        return 2
    print(f"Using Wolfram kernel: {kernel}", flush=True)
    try:
        if paclet_is_installed(kernel):
            print("PASS: ImageMetadataTools is already installed; no download.", flush=True)
            return 0
        print("MISSING: ImageMetadataTools is not installed.", flush=True)
        if args.check:
            print("Install once using --install; normal builds never download paclets.", flush=True)
            return 3
        location = cache_path()
        download(location)
        install(kernel, location)
        if not paclet_is_installed(kernel):
            raise RuntimeError("Wolfram did not register ImageMetadataTools after installation")
        print("PASS: ImageMetadataTools installed and verified. No further download needed.", flush=True)
        return 0
    except Exception as exc:
        print(f"ERROR: {exc}", file=sys.stderr, flush=True)
        return 1


if __name__ == "__main__":
    sys.exit(main())
