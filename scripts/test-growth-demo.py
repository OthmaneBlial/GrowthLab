#!/usr/bin/env python3
"""Exercise the real bundled demo command, engine, HTTP records and shutdown."""
import json
import hashlib
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from urllib.error import URLError
from urllib.request import urlopen


def main():
    binary = Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug/growthlab").resolve()
    root = Path(tempfile.mkdtemp(prefix="growthlab-demo-cli-"))
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
        environment = dict(os.environ, GROWTHLAB_DATA_DIR=str(root / "lab"),
                           ORX_CACHE_DIR=str(root / "cache"), XDG_CONFIG_HOME=str(root / "config"),
                           PATH=str(wrappers) + os.pathsep + os.environ.get("PATH", ""))
        for key in list(environment):
            if key == "ORX_DATA_DIR" or key.startswith(("GIT_", "ANTHROPIC_", "OPENAI_", "OPENROUTER_")):
                environment.pop(key)
        log_file = root / "server.log"
        with log_file.open("w") as log:
            process = subprocess.Popen([str(binary), "--no-telemetry", "demo", "--no-browser"],
                                       env=environment, stdout=log, stderr=log, start_new_session=True)
            deadline = time.monotonic() + 60
            address = None
            while time.monotonic() < deadline:
                assert process.poll() is None, "Demo exited before serving its battle; inspect the owned test log."
                match = re.search(r"growthlab demo: battle ([0-9a-f-]+) at (http://127\.0\.0\.1:\d+)/growth/", log_file.read_text())
                if match:
                    battle, address = match.groups()
                    break
                time.sleep(0.05)
            assert address, "Demo did not publish its local battle address."

            def get(route):
                with urlopen(address + route, timeout=10) as response:
                    return response.read()

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
            assert record["battle"]["status"] == "failed"
            assert not record["selections"], "The demo must not automatically select or apply."
            comparison = json.loads(get(f"/api/growth/battles/{battle}/compare"))
            assert len(comparison["recommendedCandidates"]) == 2
            for index, row in enumerate(comparison["rows"]):
                assert row["implementationProvenance"] == "SIMULATED"
                assert row["checkProvenance"] == "OBSERVED" and row["outcomeProvenance"] == "UNTESTED"
                assert row["eligible"] == (index != 1)
                assert [check["exitCode"] for check in row["checks"]] == ([2, 0, 0] if index == 1 else [0, 0, 0])
                rubric = row.get("rubric")
                assert rubric and rubric["id"] == "seo-page-hygiene-v1"
                assert rubric["provenance"] == "ESTIMATED" and rubric["maxScore"] == 100
                assert len(rubric["dimensions"]) == 8
                assert isinstance(rubric["recommendations"], list)
                if index == 1:
                    assert any("<h1>" in recommendation for recommendation in rubric["recommendations"])
                assert all(0 <= dimension["score"] <= dimension["maxScore"] for dimension in rubric["dimensions"])
                variant = row["variantId"]
                artifacts = json.loads(get(f"/api/growth/variants/{variant}/artifacts"))
                assert artifacts["sealed"]
                preview = json.loads(get(f"/api/growth/variants/{variant}/static-preview"))
                assert preview["sealed"] and preview["archiveDigest"] == artifacts["archiveDigest"]
                assert preview["record"]["status"] == "ready" and preview["record"]["blockedResources"] == 0
                assert preview["record"]["documentDigest"] == hashlib.sha256(preview["html"].encode()).hexdigest()
                assert "script-src 'none'" in preview["html"] and "data:text/css;charset=utf-8;base64," in preview["html"]
                assert len(preview["record"]["sources"]) == 2
                screenshots = preview["record"].get("screenshots", [])
                if os.environ.get("GROWTHLAB_REQUIRE_SCREENSHOT") == "1":
                    render = row.get("render")
                    assert render and render["id"] == "static-render-hints-v1"
                    assert render["provenance"] == "OBSERVED" and render["maxScore"] == 20
                    assert len(render["dimensions"]) == 4
                    assert screenshots, "A local Chromium capture was required but no PNG was archived."
                    assert {screenshot["path"] for screenshot in screenshots} == {
                        "preview/screenshot-desktop.png",
                        "preview/screenshot-phone.png",
                    }, "Both desktop and phone captures are required for the browser smoke."
                    render_checks = preview["record"].get("renderChecks", [])
                    assert {check["viewport"] for check in render_checks} == {"desktop", "phone"}
                    assert all(check["provenance"] == "OBSERVED" for check in render_checks)
                    assert all(check["viewportMatches"] for check in render_checks)
                    assert all(not check["horizontalOverflow"] for check in render_checks)
                for screenshot in screenshots:
                    viewport = "desktop" if screenshot["path"].endswith("screenshot-desktop.png") else "phone"
                    image = get(f"/api/growth/variants/{variant}/static-preview/{viewport}")
                    assert len(image) == screenshot["size"]
                    assert hashlib.sha256(image).hexdigest() == screenshot["digest"]
                    assert image.startswith(b"\x89PNG\r\n\x1a\n")
                log = json.loads(get(f"/api/growth/variants/{variant}/artifact?name=validation-0.log"))["text"]
                assert "exactlyOnePrimaryHeading" in log
            html = get(f"/growth/{battle}").decode()
            assert "<title>GrowthLab</title>" in html and 'type="module"' in html
            module = re.search(r'src="(/assets/[^\"]+\.js)"', html)
            assert module and "Run bundled demo" in get(module.group(1)).decode(), "The actual dashboard bundle must serve the demo control."
            ui_root = Path(__file__).resolve().parents[1] / "ui/dist"
            current_module = re.search(r'src="(/assets/[^\"]+\.js)"', (ui_root / "index.html").read_text())
            assert current_module and module.group(1) == current_module.group(1), "The dashboard must serve the latest built asset path."
            assert get(module.group(1)) == (ui_root / module.group(1).lstrip("/")).read_bytes(), "The dashboard server must serve the current built asset."
            report = get(f"/api/growth/battles/{battle}/report?format=html").decode()
            assert all(label in report for label in ["SIMULATED", "OBSERVED", "UNTESTED", "SEO page hygiene"])
            assert "PatchKit" not in report and str(root) not in report
            products = list((root / "lab/growth-demo").glob("*/product"))
            assert len(products) == 1
            product = products[0]
            for args in [["status", "--porcelain"], ["remote"]]:
                result = subprocess.run(["git", "-C", str(product), *args], env=environment,
                                        capture_output=True, text=True, timeout=10, check=True)
                assert not result.stdout.strip()
            assert (product / "website/index.html").read_bytes() == (Path(__file__).resolve().parents[1] / "demo/patchkit/website/index.html").read_bytes()
            assert not marker.exists(), "The bundled command unexpectedly invoked a provider CLI."
            process.send_signal(signal.SIGINT)
            assert process.wait(timeout=15) == 0
            assert not marker.exists()
            outside = root / "outside-sentinel"
            outside.write_bytes(b"Owned unrelated fixture must remain unchanged")
            redirected = dict(environment, GIT_DIR=str(root / "outside.git"))
            refused = subprocess.run([str(binary), "--no-telemetry", "demo", "--no-browser"],
                                     env=redirected, capture_output=True, timeout=30)
            assert refused.returncode != 0 and b"repository-scoped Git environment overrides" in refused.stderr
            assert len(list((root / "lab/growth-demo").glob("*/product"))) == 1
            assert outside.read_bytes() == b"Owned unrelated fixture must remain unchanged"
            assert not (root / "outside.git").exists() and not marker.exists()
            safely_stopped = True
        print("Bundled demo smoke passed: real command and HTTP engine; 3 sealed attempts; exits 0/0/0, 2/0/0, 0/0/0; no provider, selection or apply; private report; clean fictional baseline; graceful shutdown.")
    finally:
        if process is not None and process.poll() is None:
            process.send_signal(signal.SIGINT)
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGTERM)
                process.wait(timeout=10)
        # An unsuccessful check retains only its owned root for job/log inspection.
        if safely_stopped:
            shutil.rmtree(root)
        else:
            print(f"Owned demo test evidence retained at {root}", file=sys.stderr)


if __name__ == "__main__":
    main()
