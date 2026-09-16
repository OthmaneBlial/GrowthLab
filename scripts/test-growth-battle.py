#!/usr/bin/env python3
"""Run a real three-worktree replay battle with an intentional validation failure."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import shutil


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
        for row in comparison["rows"]:
            isolation = row["checks"][0]["confinement"]
            assert isolation["backend"] in ["macos-seatbelt-v1", "linux-bubblewrap-v1"]
            archive = root / "lab/growth-archives" / row["archiveDigest"]
            policy = (archive / "validation-0.policy.json").read_bytes()
            assert hashlib.sha256(policy).hexdigest() == isolation["policyDigest"]
        run("run", battle, "--replay", str(plan), succeeds=False)
        assert len(json.loads(run("experiments", "--project", project))) == 1
        assert git("rev-parse", "HEAD") == commit and git("status", "--porcelain") == "" and git("remote") == ""
        variant = prepared["variants"][0]["id"]
        invalid = prepared["variants"][1]["id"]
        run("select", invalid, succeeds=False)
        run("apply", invalid, succeeds=False)
        selected = json.loads(run("select", variant))
        assert selected["decision"] == "candidate"
        patch = root / "selected.patch"
        exported = json.loads(run("export", variant, "--output", str(patch)))
        assert exported["status"] == "done" and patch.exists()
        run("export", variant, "--output", str(patch), succeeds=False)
        preview = json.loads(run("apply", variant, "--check"))
        assert preview["changedFiles"] == ["website/index.html"]
        assert git("status", "--porcelain") == ""
        report = root / "report.html"
        markdown = root / "report.md"
        run("report", battle, "--output", str(report), "--public-goal", "Compare three declared landing-page strategies")
        run("report", battle, "--output", str(markdown), "--format", "markdown")
        for text in [report.read_text(), markdown.read_text()]:
            assert all(label in text for label in ["SIMULATED", "OBSERVED", "UNTESTED"])
            assert "Synthetic developer tool" not in text and "website/index.html" not in text and str(product) not in text
            assert str(root / "lab") not in text and "Policy SHA-256" in text
            assert all(row["checks"][0]["confinement"]["policyDigest"] in text for row in comparison["rows"])
        run("report", battle, "--output", str(report), succeeds=False)
        applied = json.loads(run("apply", variant))
        assert applied["status"] == "done" and applied["decision"] == "ship"
        assert (product / "website/index.html").read_text() == "<h1>Outcome-first</h1>"
        assert git("rev-parse", "HEAD") == commit and git("diff", "--cached", "--name-only") == "" and git("remote") == ""
        run("apply", variant, succeeds=False)
        if len(sys.argv) > 2:
            artifacts = Path(sys.argv[2])
            artifacts.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(report, artifacts / "battle-report.html")
            shutil.copyfile(markdown, artifacts / "battle-report.md")
        archive = root / "lab/growth-archives" / comparison["rows"][0]["archiveDigest"]
        assert (archive / "validation-0.log").exists() and (archive / "proposal-input.json").exists()
        target = archive / "implementation.diff"
        target.chmod(0o600)
        target.write_text("Tampered synthetic fixture")
        run("compare", battle, succeeds=False)
        run("report", battle, "--output", str(root / "tampered.html"), succeeds=False)
        run("export", variant, "--output", str(root / "tampered.patch"), succeeds=False)
        print("Growth Battle CLI smoke passed: 3 worktrees, observed failure, archived isolation policies, explicit selection/export/apply, private-context omission, overwrite/tamper refusal; HEAD/index/remotes preserved.")


if __name__ == "__main__":
    main()
