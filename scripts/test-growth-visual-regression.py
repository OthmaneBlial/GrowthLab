#!/usr/bin/env python3
"""Compare the bundled demo's sealed Chromium captures with local baselines.

This is a deliberately narrow visual-regression smoke: it pins the bundled
fixture, two CSS viewports and the locally installed Chromium renderer. It is
not a cross-browser or assistive-technology certification.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from urllib.error import HTTPError, URLError
from urllib.request import urlopen


BASELINES = {
    "variant-1-desktop.png": (1280, 900, "1c739577a5fa1042c852917cd93009dd4ca9af765a9ef8e906763499b111a1f9"),
    "variant-1-phone.png": (390, 844, "aa57f91378483db2e929271ac42a3d2c33fb2bad58b644584b240945d61c2b89"),
    "variant-2-desktop.png": (1280, 900, "2adfa41e8e2c9088f497ee5112d3b5d3edb37f7dddc433f14eec263abeb01956"),
    "variant-2-phone.png": (390, 844, "e6a98c09c71a2fa44085f2d527002782dca2e62df177912247d3760b629e1507"),
    "variant-3-desktop.png": (1280, 900, "1c6dc91b522695272a005fe1f275f911d1beaf724cbea92f13c2b674d5c0f4ee"),
    "variant-3-phone.png": (390, 844, "1f2ec0e75eb5c8ba1b53ab99f866c34c9bf0dbda804ed190efea1672e4796353"),
}


def png_size(data):
    if len(data) < 24 or data[:8] != b"\x89PNG\r\n\x1a\n" or data[12:16] != b"IHDR":
        return None
    return int.from_bytes(data[16:20], "big"), int.from_bytes(data[20:24], "big")


def main():
    repo = Path(__file__).resolve().parents[1]
    baseline_dir = repo / "scripts" / "fixtures" / "visual-regression"
    missing = [name for name in BASELINES if not (baseline_dir / name).is_file()]
    if missing:
        raise AssertionError(f"Missing checked-in visual baselines: {', '.join(missing)}")
    for name, (_, _, expected) in BASELINES.items():
        actual = hashlib.sha256((baseline_dir / name).read_bytes()).hexdigest()
        if actual != expected:
            raise AssertionError(f"Baseline {name} has digest {actual}; expected the recorded fixture {expected}.")

    root = Path(tempfile.mkdtemp(prefix="growthlab-visual-regression-"))
    process = None
    safely_stopped = False
    try:
        wrappers = root / "provider-probes"
        wrappers.mkdir()
        marker = root / "unexpected-provider-invocation"
        for name in ["claude", "codex", "opencode", "cursor-agent"]:
            probe = wrappers / name
            probe.write_text(f"#!{sys.executable}\nfrom pathlib import Path\nPath({str(marker)!r}).write_text('unexpected provider probe')\nraise SystemExit(70)\n")
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
        log_file = root / "server.log"
        with log_file.open("w") as log:
            process = subprocess.Popen(
                [str(repo / "target/debug/growthlab"), "--no-telemetry", "demo", "--no-browser"],
                env=environment,
                stdout=log,
                stderr=log,
                start_new_session=True,
            )
            deadline = time.monotonic() + 60
            address = None
            battle = None
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise AssertionError("Demo exited before serving its battle; inspect the owned test log.")
                match = re.search(r"growthlab demo: battle ([0-9a-f-]+) at (http://127\.0\.0\.1:\d+)/growth/", log_file.read_text())
                if match:
                    battle, address = match.groups()
                    break
                time.sleep(0.05)
            if not address:
                raise AssertionError("Demo did not publish its local battle address.")

            def get(route):
                try:
                    with urlopen(address + route, timeout=10) as response:
                        return response.read()
                except HTTPError as error:
                    detail = error.read().decode("utf-8", errors="replace")
                    raise AssertionError(f"HTTP {error.code} for {route}: {detail}") from error

            deadline = time.monotonic() + 60
            while time.monotonic() < deadline:
                try:
                    record = json.loads(get(f"/api/growth/battles/{battle}"))
                except URLError:
                    time.sleep(0.05)
                    continue
                if not record["controller"]["running"] and len(record["runs"]) == 3:
                    break
                time.sleep(0.1)
            else:
                raise AssertionError("Demo did not finish its three actual attempts.")

            comparison = json.loads(get(f"/api/growth/battles/{battle}/compare"))
            if len(comparison["rows"]) != 3:
                raise AssertionError("The bundled visual fixture must contain exactly three variants.")
            for index, row in enumerate(comparison["rows"], start=1):
                variant = row["variantId"]
                preview = json.loads(get(f"/api/growth/variants/{variant}/static-preview"))
                screenshots = {item["path"]: item for item in preview["record"].get("screenshots", [])}
                for viewport in ("desktop", "phone"):
                    key = f"variant-{index}-{viewport}.png"
                    expected_width, expected_height, expected_digest = BASELINES[key]
                    path = f"preview/screenshot-{viewport}.png"
                    metadata = screenshots.get(path)
                    if not metadata:
                        raise AssertionError(f"Missing archived {path} for variant {index}.")
                    image = get(f"/api/growth/variants/{variant}/static-preview/{viewport}")
                    actual_digest = hashlib.sha256(image).hexdigest()
                    if actual_digest != expected_digest:
                        raise AssertionError(f"Visual regression in {key}: got {actual_digest}, expected {expected_digest}.")
                    if image != (baseline_dir / key).read_bytes():
                        raise AssertionError(f"Visual regression in {key}: PNG bytes differ from the checked-in baseline.")
                    if png_size(image) != (expected_width, expected_height):
                        raise AssertionError(f"Unexpected {key} dimensions: {png_size(image)}")
                    if metadata["digest"] != actual_digest or metadata["width"] != expected_width or metadata["height"] != expected_height:
                        raise AssertionError(f"Archive metadata does not match {key}.")
            if marker.exists():
                raise AssertionError("The bundled visual smoke unexpectedly invoked a provider CLI.")
            process.send_signal(signal.SIGINT)
            if process.wait(timeout=15) != 0:
                raise AssertionError("Demo did not shut down cleanly after visual verification.")
            safely_stopped = True
        print("Visual regression smoke passed: three sealed demo variants match six checked-in local Chromium PNG baselines (desktop 1280x900 and phone 390x844); archive metadata and provider boundary verified.")
    finally:
        if process is not None and process.poll() is None:
            process.send_signal(signal.SIGINT)
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGTERM)
                process.wait(timeout=10)
        if safely_stopped:
            shutil.rmtree(root)
        else:
            print(f"Owned visual-regression evidence retained at {root}", file=sys.stderr)


if __name__ == "__main__":
    main()
