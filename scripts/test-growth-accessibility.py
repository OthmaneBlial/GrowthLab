#!/usr/bin/env python3
"""Run a local Chromium accessibility-tree and keyboard smoke against GrowthLab.

This deliberately bounded check inspects the real bundled demo dashboard with
Chromium's DOM and accessibility tree, then exercises keyboard focus. It is a
local structure/interaction smoke, not a WCAG audit or screen-reader test.
"""
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
import urllib.request


def http_json(url, timeout=10):
    with urllib.request.urlopen(url, timeout=timeout) as response:
        return json.loads(response.read())


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


class DevTools:
    """Minimal synchronous WebSocket client for the Chrome DevTools Protocol."""

    def __init__(self, url):
        parsed = urllib.parse.urlparse(url)
        self.sock = socket.create_connection((parsed.hostname, parsed.port), timeout=10)
        self.sock.settimeout(10)
        key = base64.b64encode(os.urandom(16)).decode()
        path = parsed.path or "/"
        request = (
            f"GET {path} HTTP/1.1\r\nHost: {parsed.hostname}:{parsed.port}\r\n"
            "Upgrade: websocket\r\nConnection: Upgrade\r\n"
            f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
        )
        self.sock.sendall(request.encode())
        headers = b""
        while b"\r\n\r\n" not in headers:
            headers += self.sock.recv(4096)
        if b" 101 " not in headers.split(b"\r\n", 1)[0]:
            raise RuntimeError(f"DevTools websocket upgrade failed: {headers[:200]!r}")
        self.message_id = 0

    def close(self):
        try:
            self.sock.close()
        except OSError:
            pass

    def _send_frame(self, payload, opcode=1):
        data = payload.encode() if isinstance(payload, str) else payload
        first = 0x80 | opcode
        size = len(data)
        if size < 126:
            header = bytes([first, 0x80 | size])
        elif size < 65536:
            header = bytes([first, 0x80 | 126]) + struct.pack("!H", size)
        else:
            header = bytes([first, 0x80 | 127]) + struct.pack("!Q", size)
        mask = os.urandom(4)
        body = bytes(byte ^ mask[index % 4] for index, byte in enumerate(data))
        self.sock.sendall(header + mask + body)

    def _recv_frame(self):
        header = self.sock.recv(2)
        if len(header) != 2:
            raise RuntimeError("DevTools websocket closed")
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
            data += self.sock.recv(size - len(data))
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
                raise RuntimeError("DevTools websocket closed")
            if opcode != 1:
                continue
            message = json.loads(data)
            if message.get("id") == ident:
                if "error" in message:
                    raise RuntimeError(f"{method}: {message['error']}")
                return message.get("result", {})


def stop_process(process, name):
    if process is None or process.poll() is not None:
        return
    process.send_signal(signal.SIGINT)
    try:
        process.wait(timeout=15)
    except subprocess.TimeoutExpired:
        if process.args and name == "demo":
            os.killpg(process.pid, signal.SIGTERM)
        else:
            process.kill()
        process.wait(timeout=10)


def main():
    repo = Path(__file__).resolve().parents[1]
    binary = repo / "target/debug/growthlab"
    if not binary.is_file():
        raise RuntimeError(f"Build the local debug binary first: {binary}")
    root = Path(tempfile.mkdtemp(prefix="growthlab-a11y-smoke-"))
    demo = None
    chrome = None
    devtools = None
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

        browser = os.environ.get("GROWTHLAB_CHROME") or "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
        if not Path(browser).is_file():
            raise RuntimeError(f"Chromium executable unavailable: {browser}")
        port = free_port()
        profile = root / "chrome-profile"
        chrome = subprocess.Popen(
            [
                browser,
                "--headless=new",
                f"--remote-debugging-port={port}",
                "--remote-allow-origins=*",
                f"--user-data-dir={profile}",
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-gpu",
                "--disable-extensions",
                "--disable-background-networking",
                "--disable-sync",
                "--hide-scrollbars",
                "--window-size=1280,900",
                "about:blank",
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        target = None
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            try:
                targets = http_json(f"http://127.0.0.1:{port}/json/list")
                target = next((item for item in targets if item.get("type") == "page" and item.get("webSocketDebuggerUrl")), None)
                if target:
                    break
            except Exception:
                pass
            time.sleep(0.1)
        if not target:
            raise RuntimeError("Chrome did not expose a DevTools page")
        devtools = DevTools(target["webSocketDebuggerUrl"])
        devtools.command("Runtime.enable")
        devtools.command("Page.enable")
        devtools.command("Accessibility.enable")
        devtools.command("Page.navigate", {"url": f"{address}/growth/{battle}"})

        def evaluate(expression):
            result = devtools.command(
                "Runtime.evaluate",
                {"expression": expression, "returnByValue": True, "awaitPromise": True},
            )
            exception = result.get("exceptionDetails")
            if exception:
                raise RuntimeError(f"Runtime.evaluate failed: {exception}")
            return result.get("result", {}).get("value")

        ready = False
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            state = evaluate("({buttons:document.querySelectorAll('button').length})")
            if state and state["buttons"] > 0:
                ready = True
                break
            time.sleep(0.2)
        if not ready:
            raise RuntimeError("dashboard did not render its controls")

        dom = evaluate(
            r'''(() => {
              const visible = (element) => { const style = getComputedStyle(element); const rect = element.getBoundingClientRect(); return style.display !== "none" && style.visibility !== "hidden" && rect.width > 0 && rect.height > 0; };
              const text = (element) => (element.textContent || "").replace(/\s+/g, " ").trim();
              const label = (element) => {
                const labelled = element.getAttribute("aria-label");
                if (labelled && labelled.trim()) return labelled.trim();
                const ids = (element.getAttribute("aria-labelledby") || "").split(/\s+/).filter(Boolean);
                const labelledBy = ids.map((id) => document.getElementById(id)).filter(Boolean).map(text).join(" ").trim();
                if (labelledBy) return labelledBy;
                if (element.labels && element.labels.length) return [...element.labels].map(text).join(" ").trim();
                if (element.tagName === "IMG" && element.hasAttribute("alt")) return element.alt.trim();
                return text(element) || element.getAttribute("title")?.trim() || element.getAttribute("placeholder")?.trim() || "";
              };
              const controls = [...document.querySelectorAll("a[href], button, input, select, textarea, [role='button'], [role='link'], [role='tab'], [role='checkbox'], [role='switch'], [role='slider']")].filter(visible);
              const headings = [...document.querySelectorAll("h1,h2,h3,h4,h5,h6")].map((element) => Number(element.tagName.slice(1)));
              const duplicateIds = [...document.querySelectorAll("[id]")].map((element) => element.id).filter((id, index, ids) => ids.indexOf(id) !== index);
              const images = [...document.querySelectorAll("img")].filter(visible);
              const forms = [...document.querySelectorAll("input,select,textarea")].filter(visible);
              return {
                controls: controls.length,
                unnamedControls: controls.filter((element) => !label(element)).length,
                headings,
                h1: headings.filter((level) => level === 1).length,
                duplicateIds: [...new Set(duplicateIds)],
                imagesMissingAlt: images.filter((element) => !element.hasAttribute("alt")).length,
                unlabeledForms: forms.filter((element) => !label(element)).length,
                landmarks: [...document.querySelectorAll("main,nav,header,footer,aside")].filter(visible).length,
                scrollWidth: document.documentElement.scrollWidth,
                viewportWidth: window.innerWidth,
              };
            })()'''
        )
        ax = devtools.command("Accessibility.getFullAXTree").get("nodes", [])
        interactive_roles = {"button", "link", "checkbox", "combobox", "listbox", "menuitem", "radio", "slider", "spinbutton", "switch", "tab", "textbox"}
        ax_interactive = [node for node in ax if node.get("role", {}).get("value") in interactive_roles]
        unnamed_ax = [node for node in ax_interactive if not (node.get("name", {}).get("value") or "").strip()]
        ax_headings = [node for node in ax if node.get("role", {}).get("value") == "heading"]
        if dom["h1"] != 1 or any(b > a + 1 for a, b in zip(dom["headings"], dom["headings"][1:])):
            raise AssertionError(f"heading hierarchy failed: {dom}")
        if dom["duplicateIds"] or dom["imagesMissingAlt"] or dom["unnamedControls"] or dom["unlabeledForms"]:
            raise AssertionError(f"DOM accessibility names failed: {dom}")
        if dom["scrollWidth"] > dom["viewportWidth"] + 1:
            raise AssertionError(f"horizontal overflow detected: {dom}")
        if not ax_interactive or unnamed_ax or not ax_headings:
            raise AssertionError(f"AX tree names failed: {len(ax_interactive)} interactive, {len(unnamed_ax)} unnamed, {len(ax_headings)} headings")

        evaluate("document.body.focus()")
        focus = []
        for _ in range(30):
            devtools.command("Input.dispatchKeyEvent", {"type": "keyDown", "key": "Tab", "code": "Tab", "windowsVirtualKeyCode": 9, "nativeVirtualKeyCode": 9})
            devtools.command("Input.dispatchKeyEvent", {"type": "keyUp", "key": "Tab", "code": "Tab", "windowsVirtualKeyCode": 9, "nativeVirtualKeyCode": 9})
            focus.append(evaluate("(() => { const e=document.activeElement; if (!e) return null; const r=e.getBoundingClientRect(); return {tag:e.tagName,id:e.id,visible:r.width>0 && r.height>0}; })()"))
        bad_focus = [item for item in focus if not item or item["tag"] == "BODY" or not item["visible"]]
        if len(focus) < 5 or bad_focus:
            raise AssertionError(f"keyboard focus smoke failed: {focus}")
        if marker.exists():
            raise AssertionError("a provider CLI was invoked")
        print(json.dumps({"dom": dom, "axInteractive": len(ax_interactive), "axHeadings": len(ax_headings), "tabStops": len(focus), "browser": browser, "browserSha256": hashlib.sha256(Path(browser).read_bytes()).hexdigest()[:16]}))
    finally:
        if devtools:
            devtools.close()
        stop_process(chrome, "chrome")
        stop_process(demo, "demo")
        shutil.rmtree(root, ignore_errors=True)


if __name__ == "__main__":
    main()
