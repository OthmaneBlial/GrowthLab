#!/usr/bin/env python3
"""Exercise actual GrowthLab CLI onboarding on a synthetic local Git product."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile


def main():
    binary = Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug/growthlab").resolve()
    with tempfile.TemporaryDirectory(prefix="growthlab-cli-") as temporary:
        root = Path(temporary)
        product = root / "product"
        (product / "website").mkdir(parents=True)
        (product / "website" / "index.html").write_text("<!doctype html><title>Synthetic fixture</title>")
        cache = root / "cache"
        (cache / "repos").mkdir(parents=True)
        retained = cache / "repos" / "keep.txt"
        retained.write_text("Synthetic cache must survive a refused update")
        environment = dict(os.environ, GROWTHLAB_DATA_DIR=str(root / "lab"), ORX_CACHE_DIR=str(cache), XDG_CONFIG_HOME=str(root / "config"))
        environment.pop("ORX_DATA_DIR", None)

        def run(*args, succeeds=True):
            result = subprocess.run([str(binary), *args], env=environment, text=True, capture_output=True, timeout=30)
            assert (result.returncode == 0) == succeeds, f"Unexpected result for fixture command {args[0]}: {result.returncode}"
            return result.stdout

        def git(*args):
            result = subprocess.run(["git", "-C", str(product), *args], env=environment, text=True, capture_output=True, timeout=30)
            assert result.returncode == 0, "Synthetic fixture Git command failed"
            return result.stdout.strip()

        assert run("--version").startswith("growthlab ")
        run("init", "--path", str(product), "--name", "Fixture", "--audience", "Developers", "--goal", "Improve qualified activation", "--mode", "implementation", "--allow", "website/", "--deny", "website/private", "--validate", "node website/check.mjs")
        original = (product / "growthlab.yaml").read_bytes()
        run("init", "--path", str(product), "--name", "Other", "--audience", "Developers", "--goal", "Other goal", succeeds=False)
        assert (product / "growthlab.yaml").read_bytes() == original
        run("config", "check", "--path", str(product))
        run("config", "check-path", "--path", str(product), "website/index.html")
        for denied in ["website/private/file", "website/PRIVATE/file", "website/private./file", "website/.env", "website-other/file", "../outside"]:
            run("config", "check-path", "--path", str(product), denied, succeeds=False)
        run("workspace", "import", "--path", str(product), succeeds=False)
        git("init", "-b", "main")
        run("workspace", "import", "--path", str(product), succeeds=False)
        git("add", "growthlab.yaml", "website/index.html")
        git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false", "-c", "core.hooksPath=.disabled-fixture-hooks", "commit", "-m", "Synthetic public fixture")
        commit = git("rev-parse", "HEAD")
        workspace = json.loads(run("workspace", "import", "--path", str(product)))
        identifier = workspace["projectId"]
        assert workspace["sourceSnapshotCommit"] == commit
        assert len(json.loads(run("workspace", "list"))) == 1
        assert json.loads(run("workspace", "view", identifier)) == workspace
        hypotheses = json.loads(run("hypotheses", identifier))
        assert len(hypotheses) == 3
        assert len({entry["id"] for entry in hypotheses}) == 3
        for entry in hypotheses:
            assert entry["provenance"] == "UNTESTED"
            assert entry["sourceSnapshotCommit"] == commit
            assert entry["confidence"]["label"] == "low"
            assert entry["decision"] == "inconclusive"
            assert entry["successThreshold"] is None
            assert len(entry["evidence"]) == 1
            assert entry["evidence"][0]["evidenceType"] == "user_input"
            assert entry["evidence"][0]["provenance"] == "OBSERVED"
        assert json.loads(run("hypotheses", identifier, "--list")) == hypotheses
        run("workspace", "import", "--path", str(product), succeeds=False)
        assert git("rev-parse", "HEAD") == commit
        assert git("status", "--porcelain") == ""
        assert git("remote") == ""
        version = json.loads(run("version", "--json"))
        assert version["latest"] is None and version["releaseLookup"] == "unavailable"
        assert version["autoUpdate"] is False
        run("update", succeeds=False)
        assert retained.exists(), "a refused update must not migrate or remove existing cache files"
        print("Growth CLI smoke passed: 3 untested hypotheses; product files, HEAD and remotes preserved.")


if __name__ == "__main__":
    main()
