#!/usr/bin/env python3
"""Exercise the native battle adapter with a disposable, key-free Claude fixture.

This is a harness-boundary regression, not evidence from a real model provider.
The temporary ``claude`` executable returns one valid implementation document;
GrowthLab still creates the real isolated worktrees, runs the configured check
in each worktree and seals the resulting archives.
"""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile


def main() -> None:
    binary = Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug/growthlab").resolve()
    if not binary.is_file():
        raise SystemExit(f"Build the local binary first: {binary}")

    with tempfile.TemporaryDirectory(prefix="growthlab-native-fixture-") as temporary:
        root = Path(temporary)
        product = root / "product"
        (product / "website").mkdir(parents=True)
        (product / "website/index.html").write_text(
            "<!doctype html><h1>Native fixture baseline</h1>", encoding="utf-8"
        )
        (product / "website/check.mjs").write_text(
            "import {readFileSync} from 'node:fs'; "
            "if (!readFileSync('website/index.html', 'utf8').includes('<h1>')) process.exit(2);",
            encoding="utf-8",
        )

        provider_bin = root / "provider-bin"
        provider_bin.mkdir()
        fake_claude = provider_bin / "claude"
        fake_claude.write_text(
            "#!/usr/bin/env python3\n"
            "import json\n"
            "print(json.dumps({\n"
            "  'summary': 'Local native proposal fixture',\n"
            "  'files': [{'path': 'website/index.html', 'contents': '<!doctype html><h1>Native fixture proposal</h1>'}],\n"
            "  'risks': ['Fixture has no real growth data']\n"
            "}))\n",
            encoding="utf-8",
        )
        fake_claude.chmod(0o700)

        environment = dict(
            os.environ,
            GROWTHLAB_DATA_DIR=str(root / "lab"),
            ORX_CACHE_DIR=str(root / "cache"),
            XDG_CONFIG_HOME=str(root / "config"),
            PATH=str(provider_bin) + os.pathsep + os.environ.get("PATH", ""),
            ORX_NO_UPDATE_CHECK="1",
            DO_NOT_TRACK="1",
        )
        for key in list(environment):
            if key == "ORX_DATA_DIR" or key.startswith(
                ("GIT_", "ANTHROPIC_", "OPENAI_", "OPENROUTER_")
            ):
                environment.pop(key, None)

        def run(*args: str, succeeds: bool = True) -> str:
            result = subprocess.run(
                [str(binary), "--no-telemetry", *args],
                env=environment,
                text=True,
                capture_output=True,
                timeout=120,
            )
            if (result.returncode == 0) != succeeds:
                raise AssertionError(
                    f"Fixture CLI result was unexpected for {args}: "
                    f"rc={result.returncode}\nstdout={result.stdout}\nstderr={result.stderr}"
                )
            return result.stdout

        def git(*args: str) -> str:
            result = subprocess.run(
                ["git", "-C", str(product), *args],
                env=environment,
                text=True,
                capture_output=True,
                timeout=30,
            )
            if result.returncode:
                raise AssertionError(result.stderr)
            return result.stdout.strip()

        run(
            "init",
            "--path",
            str(product),
            "--name",
            "Native fixture product",
            "--audience",
            "SEO teams",
            "--goal",
            "Improve qualified discovery",
            "--mode",
            "implementation",
            "--allow",
            "website",
            "--validate",
            "node website/check.mjs",
        )
        git("init", "-b", "main")
        git("add", "growthlab.yaml", "website")
        git(
            "-c",
            "user.name=GrowthLab fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=.disabled-fixture-hooks",
            "commit",
            "-m",
            "Native fixture baseline",
        )
        source_commit = git("rev-parse", "HEAD")
        project_id = json.loads(run("workspace", "import", "--path", str(product)))["projectId"]
        prepared = json.loads(
            run(
                "battle",
                "Improve qualified discovery",
                "--project",
                project_id,
                "--prepare-only",
            )
        )
        battle_id = prepared["battle"]["id"]
        assert len(prepared["variants"]) == 3
        result = json.loads(
            run(
                "run",
                battle_id,
                "--harness",
                "claude-code",
                "--agent-timeout-seconds",
                "30",
            )
        )
        assert result["status"] == "completed", result
        status = json.loads(run("battle-status", battle_id))
        assert len(status["runs"]) == 3, status
        for sealed in status["runs"]:
            record = sealed["run"]
            assert record["agent"]["harness"] == "claude-code", record["agent"]
            assert record["agent"]["mode"] == "isolated_tools_disabled_proposal"
            assert record["provenance"] == "UNTESTED"
            assert record["implementation"]["files"][0]["path"] == "website/index.html"
            assert record["validations"][0]["exitCode"] == 0

        assert git("rev-parse", "HEAD") == source_commit
        assert git("status", "--porcelain") == ""
        assert git("remote") == ""
        assert (product / "website/index.html").read_text(encoding="utf-8") == (
            "<!doctype html><h1>Native fixture baseline</h1>"
        )
        print(
            "Native harness fixture passed: fake Claude proposal, three isolated "
            "worktrees, observed checks and sealed UNTESTED runs; product checkout "
            "and remotes stayed unchanged."
        )


if __name__ == "__main__":
    main()
