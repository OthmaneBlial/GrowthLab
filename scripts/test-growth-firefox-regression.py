#!/usr/bin/env python3
"""Render the bundled GrowthLab preview in Firefox and pin local baselines.

The existing visual smoke covers the Chromium renderer used by the preview
archiver. This companion check exercises the same sealed HTML in a second
locally installed browser. It compares deterministic PNG bytes for the two
supported viewports and checks the layout invariants that should hold across
renderers. It is still a local browser smoke, not a claim of every browser or
assistive-technology conformance.

Use ``--update-baselines`` only when the bundled fixture or the pinned Firefox
build intentionally changes. The command writes the six PNGs and prints the
digests needed to review the fixture update.
"""

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import time
import urllib.parse
from urllib.error import HTTPError, URLError
from urllib.request import urlopen


VIEWPORTS = {
    "desktop": (1280, 900),
    "phone": (390, 844),
}

# Filled by the first intentional --update-baselines run. Keeping the browser
# digest in the output makes a renderer change reviewable without pretending
# that Firefox pixels are interchangeable with Chromium pixels.
BASELINES = {
    "variant-1-desktop-firefox.png": "8e9b5058908d040fb8af75898def4efa4b9f5982b875093147038be54866bfc9",
    "variant-1-phone-firefox.png": "a7501efa475da07bf2c83398bed057f5d1d9c7eadb886b1bf037b27b32bfbed3",
    "variant-2-desktop-firefox.png": "0a7428ecd75a445c5b1626a7598a75ba091e606eac500aa59bd92b4b9b5b1cb9",
    "variant-2-phone-firefox.png": "ba6bad508096fb1025a28b2e606c3c2ca94918054612ec2ec240deacdded5a7a",
    "variant-3-desktop-firefox.png": "8c44f3512c27f1721ec17fea7b21a4c40ccd0440c786610bd9b9f6064a21536e",
    "variant-3-phone-firefox.png": "b2fcfe02cdec177e9f13c75f7617ea37a26bc57b718af267c5f661690dec02ab",
}


def http_json(url, timeout=10):
    try:
        with urlopen(url, timeout=timeout) as response:
            return json.loads(response.read())
    except HTTPError as error:
        detail = error.read().decode("utf-8", errors="replace")
        raise AssertionError(f"HTTP {error.code} for {url}: {detail}") from error


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


class BiDi:
    """Minimal synchronous WebSocket client for Firefox WebDriver BiDi."""

    def __init__(self, url):
        parsed = urllib.parse.urlparse(url)
        self.sock = socket.create_connection((parsed.hostname, parsed.port), timeout=10)
        self.sock.settimeout(20)
        key = base64.b64encode(os.urandom(16)).decode()
        path = parsed.path or "/session"
        request = (
            f"GET {path} HTTP/1.1\r\nHost: {parsed.hostname}:{parsed.port}\r\n"
            "Upgrade: websocket\r\nConnection: Upgrade\r\n"
            f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
        )
        self.sock.sendall(request.encode())
        headers = b""
        while b"\r\n\r\n" not in headers:
            chunk = self.sock.recv(4096)
            if not chunk:
                raise RuntimeError("Firefox BiDi websocket closed during upgrade")
            headers += chunk
        if b" 101 " not in headers.split(b"\r\n", 1)[0]:
            raise RuntimeError(f"Firefox BiDi websocket upgrade failed: {headers[:200]!r}")
        self.message_id = 0

    def close(self):
        try:
            self.sock.close()
        except OSError:
            pass

    def _send_frame(self, payload, opcode=1):
        data = payload.encode() if isinstance(payload, str) else payload
        size = len(data)
        if size < 126:
            header = bytes([0x80 | opcode, 0x80 | size])
        elif size < 65536:
            header = bytes([0x80 | opcode, 0x80 | 126]) + struct.pack("!H", size)
        else:
            header = bytes([0x80 | opcode, 0x80 | 127]) + struct.pack("!Q", size)
        mask = os.urandom(4)
        body = bytes(byte ^ mask[index % 4] for index, byte in enumerate(data))
        self.sock.sendall(header + mask + body)

    def _recv_frame(self):
        header = self.sock.recv(2)
        if len(header) != 2:
            raise RuntimeError("Firefox BiDi websocket closed")
        first, second = header
        opcode = first & 0x0F
        size = second & 0x7F
        if size == 126:
            size = struct.unpack("!H", self.sock.recv(2))[0]
        elif size == 127:
            size = struct.unpack("!Q", self.sock.recv(8))[0]
        masked = second & 0x80
        mask = self.sock.recv(4) if masked else None
        data = b""
        while len(data) < size:
            chunk = self.sock.recv(size - len(data))
            if not chunk:
                raise RuntimeError("Firefox BiDi websocket closed mid-frame")
            data += chunk
        if mask:
            data = bytes(byte ^ mask[index % 4] for index, byte in enumerate(data))
        return opcode, data

    def command(self, method, params=None):
        self.message_id += 1
        ident = self.message_id
        self._send_frame(json.dumps({"id": ident, "method": method, "params": params or {}}))
        while True:
            opcode, data = self._recv_frame()
            if opcode == 9:
                self._send_frame(data, opcode=10)
                continue
            if opcode == 8:
                raise RuntimeError("Firefox BiDi websocket closed")
            if opcode != 1:
                continue
            message = json.loads(data)
            if message.get("id") != ident:
                continue
            if message.get("type") == "error":
                raise RuntimeError(f"{method}: {message.get('error')}: {message.get('message')}")
            return message.get("result", {})


def stop_process(process, name):
    if process is None or process.poll() is not None:
        return
    process.send_signal(signal.SIGTERM)
    try:
        process.wait(timeout=15)
    except subprocess.TimeoutExpired:
        if name == "demo":
            os.killpg(process.pid, signal.SIGTERM)
        else:
            process.kill()
        process.wait(timeout=10)


def bidi_value(result):
    """Extract a primitive BiDi result, rejecting thrown script exceptions."""
    if result.get("type") != "success":
        raise RuntimeError(f"Firefox script evaluation failed: {result}")
    value = result.get("result", {}).get("value")
    if isinstance(value, dict) and "type" in value and "value" in value:
        return value["value"]
    return value


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--update-baselines", action="store_true")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    binary = repo / "target/debug/growthlab"
    firefox = os.environ.get("GROWTHLAB_FIREFOX") or "/Applications/Firefox.app/Contents/MacOS/firefox"
    if not binary.is_file():
        raise RuntimeError(f"Build the local debug binary first: {binary}")
    if not Path(firefox).is_file():
        raise RuntimeError(f"Firefox executable unavailable: {firefox}")
    baseline_dir = repo / "scripts/fixtures/visual-regression"
    root = Path(tempfile.mkdtemp(prefix="growthlab-firefox-regression-"))
    demo = http = firefox_process = None
    bidi = None
    try:
        wrappers = root / "provider-probes"
        wrappers.mkdir()
        marker = root / "unexpected-provider-invocation"
        for name in ["claude", "codex", "opencode", "cursor-agent"]:
            probe = wrappers / name
            probe.write_text(
                f"#!{sys.executable}\nfrom pathlib import Path\n"
                f"Path({str(marker)!r}).write_text('unexpected provider probe')\n"
                "raise SystemExit(70)\n"
            )
            probe.chmod(0o700)
        environment = dict(
            os.environ,
            GROWTHLAB_DATA_DIR=str(root / "lab"),
            ORX_CACHE_DIR=str(root / "cache"),
            XDG_CONFIG_HOME=str(root / "config"),
            PATH=str(wrappers) + os.pathsep + os.environ.get("PATH", ""),
        )
        for key in list(environment):
            if key == "ORX_DATA_DIR" or key.startswith(("GIT_", "ANTHROPIC_", "OPENAI_", "OPENROUTER_")):
                environment.pop(key)
        log_path = root / "demo.log"
        with log_path.open("w") as log:
            demo = subprocess.Popen(
                [str(binary), "--no-telemetry", "demo", "--no-browser"],
                env=environment,
                stdout=log,
                stderr=log,
                start_new_session=True,
            )
            address = battle = None
            deadline = time.monotonic() + 60
            while time.monotonic() < deadline:
                match = re.search(
                    r"growthlab demo: battle ([0-9a-f-]+) at (http://127\.0\.0\.1:\d+)/growth/",
                    log_path.read_text(),
                )
                if match:
                    battle, address = match.groups()
                    break
                if demo.poll() is not None:
                    raise RuntimeError(f"demo exited before startup: {log_path.read_text()}")
                time.sleep(0.05)
            if not address:
                raise RuntimeError("demo did not start")
            deadline = time.monotonic() + 60
            while time.monotonic() < deadline:
                record = http_json(f"{address}/api/growth/battles/{battle}")
                if not record["controller"]["running"] and len(record["runs"]) == 3:
                    break
                time.sleep(0.1)
            else:
                raise RuntimeError("demo did not finish")
            comparison = http_json(f"{address}/api/growth/battles/{battle}/compare")
            if len(comparison["rows"]) != 3:
                raise AssertionError("The bundled Firefox fixture must contain exactly three variants.")
            preview_html = []
            for row in comparison["rows"]:
                preview = http_json(f"{address}/api/growth/variants/{row['variantId']}/static-preview")
                if not preview.get("sealed") or preview.get("record", {}).get("status") != "ready":
                    raise AssertionError("Firefox must render a sealed ready preview.")
                preview_html.append(preview["html"])
        source = root / "preview-source"
        source.mkdir()
        for index, html in enumerate(preview_html, 1):
            (source / f"variant-{index}.html").write_text(html)
        http_port = free_port()
        http = subprocess.Popen(
            [sys.executable, "-m", "http.server", str(http_port), "--bind", "127.0.0.1", "--directory", str(source)],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        source_url = f"http://127.0.0.1:{http_port}"
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            try:
                if urlopen(f"{source_url}/variant-1.html", timeout=1).status == 200:
                    break
            except URLError:
                time.sleep(0.05)
        else:
            raise RuntimeError("preview source server did not start")

        port = free_port()
        profile = root / "firefox-profile"
        firefox_process = subprocess.Popen(
            [
                firefox,
                "--headless",
                "--no-remote",
                "--profile",
                str(profile),
                f"--remote-debugging-port={port}",
                "--remote-allow-hosts",
                "127.0.0.1",
                "about:blank",
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            try:
                with socket.create_connection(("127.0.0.1", port), timeout=1):
                    break
            except OSError:
                time.sleep(0.1)
        else:
            raise RuntimeError("Firefox did not expose WebDriver BiDi")
        bidi = BiDi(f"ws://127.0.0.1:{port}/session")
        capabilities = bidi.command("session.new", {"capabilities": {}})["capabilities"]
        tree = bidi.command("browsingContext.getTree")
        context = tree["contexts"][0]["context"]

        def evaluate(expression):
            result = bidi.command(
                "script.evaluate",
                {
                    "expression": f"JSON.stringify(({expression}))",
                    "target": {"context": context},
                    "awaitPromise": True,
                    "resultOwnership": "root",
                },
            )
            value = bidi_value(result)
            return json.loads(value)

        observed = {}
        for index in range(1, 4):
            for viewport, (width, height) in VIEWPORTS.items():
                bidi.command(
                    "browsingContext.setViewport",
                    {"context": context, "viewport": {"width": width, "height": height}, "devicePixelRatio": 1},
                )
                bidi.command(
                    "browsingContext.navigate",
                    {"context": context, "url": f"{source_url}/variant-{index}.html", "wait": "complete"},
                )
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline:
                    state = evaluate(
                        "({ready:document.readyState, fonts:document.fonts ? document.fonts.status : 'unavailable', body:!!document.body})"
                    )
                    if state["ready"] == "complete" and state["body"] and state["fonts"] in ("loaded", "unavailable"):
                        break
                    time.sleep(0.05)
                else:
                    raise RuntimeError(f"Firefox did not finish rendering variant {index} {viewport}")
                checks = evaluate(
                    """(() => {
                      const visible = (e) => { const s=getComputedStyle(e), r=e.getBoundingClientRect(); return s.display!=='none' && s.visibility!=='hidden' && r.width>0 && r.height>0; };
                      const text = (e) => (e.textContent || '').replace(/\\s+/g, ' ').trim();
                      const label = (e) => e.getAttribute('aria-label')?.trim() ||
                        (e.getAttribute('aria-labelledby') || '').split(/\\s+/).map(id => document.getElementById(id)).filter(Boolean).map(text).join(' ').trim() ||
                        [...(e.labels || [])].map(text).join(' ').trim() || text(e) || e.getAttribute('title')?.trim() || e.getAttribute('placeholder')?.trim() || '';
                      const controls=[...document.querySelectorAll('a[href],button,input,select,textarea,[role="button"],[role="link"],[role="tab"],[role="checkbox"],[role="switch"],[role="slider"]')].filter(visible);
                      const headings=[...document.querySelectorAll('h1,h2,h3,h4,h5,h6')].map(e=>Number(e.tagName.slice(1)));
                      const duplicateIds=[...document.querySelectorAll('[id]')].map(e=>e.id).filter((id,i,all)=>all.indexOf(id)!==i);
                      const images=[...document.querySelectorAll('img')].filter(visible);
                      return {controls:controls.length, unnamedControls:controls.filter(e=>!label(e)).length, headings, h1:headings.filter(n=>n===1).length,
                        duplicateIds:[...new Set(duplicateIds)], imagesMissingAlt:images.filter(e=>!e.hasAttribute('alt')).length,
                        overflow:document.documentElement.scrollWidth>window.innerWidth+1, width:window.innerWidth, height:window.innerHeight};
                    })()"""
                )
                # Static previews are user-owned documents; unlike the GrowthLab
                # dashboard they may intentionally start at h2 or contain no
                # h1. Preserve the useful no-skipped-level check without
                # imposing the dashboard shell's heading contract on them.
                if checks["headings"] and any(b > a + 1 for a, b in zip(checks["headings"], checks["headings"][1:])):
                    raise AssertionError(f"Firefox heading hierarchy failed for variant {index} {viewport}: {checks}")
                if checks["unnamedControls"] or checks["duplicateIds"] or checks["imagesMissingAlt"] or checks["overflow"]:
                    raise AssertionError(f"Firefox layout/accessibility invariants failed for variant {index} {viewport}: {checks}")
                shot = bidi.command(
                    "browsingContext.captureScreenshot",
                    {"context": context, "origin": "viewport", "format": {"type": "image/png"}},
                )
                image = base64.b64decode(shot["data"])
                if len(image) < 24 or image[:8] != b"\x89PNG\r\n\x1a\n" or image[12:16] != b"IHDR":
                    raise AssertionError(f"Firefox did not return a PNG for variant {index} {viewport}")
                actual_width = int.from_bytes(image[16:20], "big")
                actual_height = int.from_bytes(image[20:24], "big")
                if (actual_width, actual_height) != (width, height):
                    raise AssertionError(f"Firefox screenshot dimensions {(actual_width, actual_height)} != {(width, height)}")
                name = f"variant-{index}-{viewport}-firefox.png"
                digest = hashlib.sha256(image).hexdigest()
                observed[name] = digest
                destination = baseline_dir / name
                if args.update_baselines:
                    destination.write_bytes(image)
                elif BASELINES.get(name) != digest or not destination.is_file() or destination.read_bytes() != image:
                    expected = BASELINES.get(name, "missing baseline")
                    raise AssertionError(f"Firefox visual regression in {name}: got {digest}, expected {expected}")
        if marker.exists():
            raise AssertionError("The bundled Firefox smoke unexpectedly invoked a provider CLI.")
        print(json.dumps({"browser": "Firefox", "version": capabilities.get("browserVersion"), "baselines": observed, "updated": args.update_baselines}))
    finally:
        if bidi:
            bidi.close()
        stop_process(firefox_process, "firefox")
        stop_process(http, "http")
        stop_process(demo, "demo")
        shutil.rmtree(root, ignore_errors=True)


if __name__ == "__main__":
    main()
