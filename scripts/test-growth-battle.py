#!/usr/bin/env python3
"""Run a real three-worktree replay battle with an intentional validation failure."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile


def main():
    binary = Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug/growthlab").resolve()
    with tempfile.TemporaryDirectory(prefix="growthlab-battle-cli-") as temporary:
        root = Path(temporary)
        product = root / "product"
        (product / "website").mkdir(parents=True)
        (product / "website/index.html").write_text("<!doctype html><h1>Synthetic baseline</h1>")
        (product / "website/check.mjs").write_text("import {readFileSync} from 'node:fs'; if (!readFileSync('website/index.html','utf8').includes('<h1>')) process.exit(2); console.log('Observed heading check passed');")
        environment = dict(os.environ, GROWTHLAB_DATA_DIR=str(root / "lab"), ORX_CACHE_DIR=str(root / "cache"), XDG_CONFIG_HOME=str(root / "config"))
        environment.pop("ORX_DATA_DIR", None)

        def run(*args, succeeds=True):
            result = subprocess.run([str(binary), *args], env=environment, text=True, capture_output=True, timeout=60)
            assert (result.returncode == 0) == succeeds, f"Fixture CLI result was unexpected for {args[0]}"
            return result.stdout

        def git(*args):
            result = subprocess.run(["git", "-C", str(product), *args], env=environment, text=True, capture_output=True, timeout=30)
            assert result.returncode == 0, "Synthetic fixture Git operation failed"
            return result.stdout.strip()

        run("init", "--path", str(product), "--name", "Synthetic developer tool", "--audience", "Developers", "--goal", "Improve qualified activation", "--mode", "implementation", "--allow", "website", "--validate", "node website/check.mjs")
        git("init", "-b", "main")
        git("add", "growthlab.yaml", "website")
        git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false", "-c", "core.hooksPath=.disabled-fixture-hooks", "commit", "-m", "Synthetic public product")
        commit = git("rev-parse", "HEAD")
        project = json.loads(run("workspace", "import", "--path", str(product)))["projectId"]
        prepared = json.loads(run("battle", "Improve qualified activation", "--project", project, "--prepare-only"))
        battle = prepared["battle"]["id"]
        assert prepared["battle"]["contract"]["sourceSnapshotCommit"] == commit
        assert len(prepared["variants"]) == 3
        for variant in prepared["variants"]:
            result = subprocess.run(["git", "-C", variant["worktree"], "rev-parse", "HEAD"], text=True, capture_output=True, timeout=30)
            assert result.returncode == 0 and result.stdout.strip() == commit
        assert not json.loads(run("compare", battle))["recommendedCandidates"]
        plan = root / "replay.json"
        plan.write_text(json.dumps({"version": 1, "implementations": [{"summary": "Declared replay strategy", "files": [{"path": "website/index.html", "contents": html}], "risks": ["No real growth data"]} for html in ["<h1>Outcome-first</h1>", "Deliberate invalid heading fixture", "<h1>Faster first success</h1>"]]}))
        result = json.loads(run("run", battle, "--replay", str(plan)))
        assert result["status"] == "failed", "the intentional validation failure must be visible"
        comparison = json.loads(run("compare", battle))
        assert comparison["label"] == "Recommended candidates"
        assert len(comparison["recommendedCandidates"]) == 2
        assert comparison["rows"][1]["checks"][0]["exitCode"] == 2
        assert all(row["implementationProvenance"] == "SIMULATED" and row["checkProvenance"] == "OBSERVED" and row["outcomeProvenance"] == "UNTESTED" for row in comparison["rows"])
        status = json.loads(run("battle-status", battle))
        assert len(status["runs"]) == 3
        run("run", battle, "--replay", str(plan), succeeds=False)
        assert len(json.loads(run("experiments", "--project", project))) == 1
        assert git("rev-parse", "HEAD") == commit and git("status", "--porcelain") == "" and git("remote") == ""
        archive = root / "lab/growth-archives" / comparison["rows"][0]["archiveDigest"]
        assert (archive / "validation-0.log").exists() and (archive / "proposal-input.json").exists()
        target = archive / "implementation.diff"
        target.chmod(0o600)
        target.write_text("Tampered synthetic fixture")
        run("compare", battle, succeeds=False)
        print("Growth Battle CLI smoke passed: 3 real worktrees, 2 eligible candidates, 1 observed failure, immutable-evidence tamper refusal; product HEAD/files/remotes preserved.")


if __name__ == "__main__":
    main()
