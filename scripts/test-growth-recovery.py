#!/usr/bin/env python3
"""Hard-kill a real CLI controller, retain its jobs, and recover sealed evidence.

An optional older binary also verifies actual legacy archive compatibility.
All repositories, inputs, jobs and reports are synthetic and stay in a temporary root.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time


def fixture(root, binary, gated):
    product = root / "product"
    (product / "website").mkdir(parents=True)
    (product / "website/index.html").write_text("<!doctype html><h1>Synthetic baseline</h1>")
    check = "import {existsSync,readFileSync,writeFileSync} from 'node:fs';"
    if gated:
        check += "writeFileSync('ready','started');const timer=setInterval(()=>{if(existsSync('release')){clearInterval(timer);process.exit(readFileSync('website/index.html','utf8').includes('<h1>')?0:2)}},20);"
    else:
        check += "if(!readFileSync('website/index.html','utf8').includes('<h1>'))process.exit(2);"
    (product / "website/check.mjs").write_text(check)
    environment = dict(os.environ, GROWTHLAB_DATA_DIR=str(root / "lab"), ORX_CACHE_DIR=str(root / "cache"), XDG_CONFIG_HOME=str(root / "config"))
    environment.pop("ORX_DATA_DIR", None)

    def run(*args, selected_binary=binary, succeeds=True):
        result = subprocess.run([str(selected_binary), *args], env=environment, text=True, capture_output=True, timeout=60)
        assert (result.returncode == 0) == succeeds, f"Synthetic CLI result was unexpected for {args[0]}"
        return result.stdout

    def git(*args):
        result = subprocess.run(["git", "-C", str(product), *args], env=environment, text=True, capture_output=True, timeout=30)
        assert result.returncode == 0, "Synthetic Git fixture operation failed"
        return result.stdout.strip()

    run("init", "--path", str(product), "--name", "PrivateSyntheticRecoveryName", "--audience", "Developers", "--goal", "PrivateSyntheticRecoveryGoal", "--mode", "implementation", "--allow", "website", "--validate", "node website/check.mjs")
    git("init", "-b", "main")
    git("add", "growthlab.yaml", "website")
    git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false", "-c", "core.hooksPath=.disabled-fixture-hooks", "commit", "-m", "Synthetic recovery product")
    project = json.loads(run("workspace", "import", "--path", str(product)))["projectId"]
    prepared = json.loads(run("battle", "PrivateSyntheticRecoveryGoal", "--project", project, "--prepare-only"))
    plan = root / "replay.json"
    plan.write_text(json.dumps({"version": 1, "implementations": [{"summary": "Declared synthetic replay", "files": [{"path": "website/index.html", "contents": html}], "risks": ["No real growth data"]} for html in ["<h1>Outcome-first</h1>", "Intentional heading failure", "<h1>First success</h1>"]]}))
    return product, environment, run, git, prepared, plan


def release_jobs(root):
    directories = list((root / "lab/growth-jobs").glob("*"))
    for directory in directories:
        # Safe even if snapshot staging is still finishing: it preserves this
        # test-only untracked marker, then the gated Node command can exit.
        (directory / "repo").mkdir(exist_ok=True)
        (directory / "repo/release").write_text("synthetic release")
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        if all((directory / "exit_code").exists() for directory in directories):
            return
        time.sleep(0.05)
    raise AssertionError("Synthetic job controllers did not record terminal exits")


def main():
    binary = Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug/growthlab").resolve()
    legacy = Path(sys.argv[2]).resolve() if len(sys.argv) > 2 else None
    with tempfile.TemporaryDirectory(prefix="growthlab-recovery-cli-") as temporary:
        root = Path(temporary)
        current = root / "current"
        product, environment, run, git, prepared, plan = fixture(current, binary, True)
        battle = prepared["battle"]["id"]
        commit = git("rev-parse", "HEAD")
        controller = subprocess.Popen([str(binary), "run", battle, "--replay", str(plan)], env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            deadline = time.monotonic() + 30
            while time.monotonic() < deadline:
                assert controller.poll() is None, "Synthetic CLI controller ended before interruption"
                status = json.loads(run("battle-status", battle))
                attempts = status["attempts"]
                if len(attempts) == 3 and all(attempt["run"].get("activeValidation") and (current / "lab/growth-jobs" / attempt["run"]["activeValidation"]["runId"] / "repo/ready").exists() for attempt in attempts):
                    break
                time.sleep(0.05)
            else:
                raise AssertionError("Synthetic commands never reached their live gates")
            run("recover", battle, succeeds=False)
            controller.kill()
            controller.communicate(timeout=10)
            waiting = json.loads(run("recover", battle))
            assert waiting["status"] == "waiting" and len(waiting["waitingJobs"]) == 3
            assert not waiting["recoveredAttempts"] and not json.loads(run("battle-status", battle))["runs"]
            release_jobs(current)
            moved = current / "unavailable-product"
            product.rename(moved)
            recovered = json.loads(run("recover", battle))
            assert recovered["status"] == "recovered" and len(recovered["recoveredAttempts"]) == 3
            assert recovered["battle"]["status"] == "failed"
            comparison = json.loads(run("compare", battle))
            assert not comparison["recommendedCandidates"], "Interrupted commands cannot create successful candidates"
            assert comparison["rows"][1]["checks"][0]["exitCode"] == 2
            assert all(row["status"] == "failed" and row["implementationProvenance"] == "SIMULATED" and row["checkProvenance"] == "OBSERVED" and row["outcomeProvenance"] == "UNTESTED" for row in comparison["rows"])
            for row in comparison["rows"]:
                isolation = row["checks"][0]["confinement"]
                assert isolation["backend"] in ["macos-seatbelt-v1", "linux-bubblewrap-v1"]
                archive = current / "lab/growth-archives" / row["archiveDigest"]
                assert hashlib.sha256((archive / "validation-0.policy.json").read_bytes()).hexdigest() == isolation["policyDigest"]
            status = json.loads(run("battle-status", battle))
            for seal in status["runs"]:
                archive = current / "lab/growth-archives" / seal["archiveDigest"]
                assert all((archive / name).exists() for name in ["proposal-input.json", "implementation.diff", "files/website/index.html", "validation-0.log", "recovery.json"])
            run("report", battle, "--output", str(current / "recovered.md"), "--format", "markdown")
            report = (current / "recovered.md").read_text()
            assert "PrivateSyntheticRecoveryName" not in report and "PrivateSyntheticRecoveryGoal" not in report and str(product) not in report
            assert str(current / "lab") not in report and "Policy SHA-256" in report
            assert json.loads(run("recover", battle))["status"] == "unchanged"
            assert json.loads(run("battle-status", battle))["runs"] == status["runs"]
            assert len(list((current / "lab/growth-jobs").glob("*"))) == 3
            moved.rename(product)
            assert git("rev-parse", "HEAD") == commit and git("status", "--porcelain") == "" and git("remote") == ""
        finally:
            if controller.poll() is None:
                controller.kill()
            controller.communicate(timeout=10)
            release_jobs(current)
        if legacy:
            old_root = root / "legacy"
            _, _, old_run, old_git, old_prepared, old_plan = fixture(old_root, legacy, False)
            old_battle = old_prepared["battle"]["id"]
            old_run("run", old_battle, "--replay", str(old_plan))
            before = json.loads(old_run("compare", old_battle))
            after = json.loads(old_run("compare", old_battle, selected_binary=binary))
            assert after == before, "Current binary must preserve actual legacy sealed archives and evaluation"
            assert json.loads(old_run("recover", old_battle, selected_binary=binary))["status"] == "unchanged"
            variant = old_prepared["variants"][0]["id"]
            old_run("select", variant, selected_binary=binary)
            old_run("export", variant, "--output", str(old_root / "legacy.patch"), selected_binary=binary)
            assert "+<h1>Outcome-first</h1>" in (old_root / "legacy.patch").read_text()
            assert old_git("status", "--porcelain") == ""
        print("Growth recovery CLI smoke passed: hard-killed owner; live jobs retained; sealed context/logs recovered; no reruns or successful-candidate promotion; missing product supported; repeated recovery preserved seals." + (" Actual legacy archives and patch delivery remain compatible." if legacy else ""))


if __name__ == "__main__":
    main()
