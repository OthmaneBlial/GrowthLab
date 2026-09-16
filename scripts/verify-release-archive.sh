#!/usr/bin/env bash
# Verify the structure, notices, checksum, and (when compatible) runtime of a
# GrowthLab release archive. This is a local, offline check.
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: scripts/verify-release-archive.sh <archive.tar.gz|archive.tar.xz|archive.zip>" >&2
  exit 2
fi

ARCHIVE="$1"
[[ -f "$ARCHIVE" && ! -L "$ARCHIVE" ]] || { echo "Archive is not a regular file: $ARCHIVE" >&2; exit 1; }

python3 - "$ARCHIVE" <<'PY'
from __future__ import annotations

import hashlib
import pathlib
import platform
import subprocess
import sys
import tarfile
import tempfile
import zipfile

archive = pathlib.Path(sys.argv[1]).resolve()
sidecar = pathlib.Path(f"{archive}.sha256")
if sidecar.is_file():
    token = sidecar.read_text(encoding="utf-8").split()[0]
    if len(token) != 64 or any(c not in "0123456789abcdefABCDEF" for c in token):
        raise SystemExit(f"invalid SHA-256 sidecar: {sidecar}")
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    if digest.lower() != token.lower():
        raise SystemExit(f"SHA-256 mismatch: expected {token}, got {digest}")
    print(f"checksum: passed ({digest})")
else:
    print("checksum: unverified (no adjacent .sha256 file)")

def safe_name(name: str) -> bool:
    path = pathlib.PurePosixPath(name)
    return not path.is_absolute() and ".." not in path.parts and "\\" not in name and name != "."

with tempfile.TemporaryDirectory(prefix="growthlab-archive-") as temporary:
    extraction = pathlib.Path(temporary)
    if archive.name.endswith(".zip"):
        with zipfile.ZipFile(archive) as bundle:
            names = bundle.namelist()
            if any(not safe_name(name) for name in names):
                raise SystemExit("archive contains an absolute or traversal path")
            bundle.extractall(extraction)
    else:
        with tarfile.open(archive, mode="r:*") as bundle:
            members = bundle.getmembers()
            names = [member.name for member in members]
            if any(not safe_name(name) for name in names):
                raise SystemExit("archive contains an absolute or traversal path")
            if any(member.issym() or member.islnk() for member in members):
                raise SystemExit("archive contains an unexpected link")
            bundle.extractall(extraction, members=members)

    roots = {pathlib.PurePosixPath(name).parts[0] for name in names if name}
    if len(roots) != 1:
        raise SystemExit("archive must contain exactly one top-level directory")
    root = extraction / next(iter(roots))
    required = [root / "LICENSE", root / "NOTICE.md", root / "README.md", root / "docs" / "demo.md", root / "docs" / "distribution.md"]
    required.extend(sorted((root / "licenses").glob("*")))
    missing = [str(path.relative_to(root)) for path in required if not path.is_file()]
    if missing:
        raise SystemExit("missing distribution files: " + ", ".join(missing))
    binaries = list((root / "bin").glob("growthlab*"))
    binaries = [path for path in binaries if path.is_file() and not path.is_symlink()]
    if len(binaries) != 1:
        raise SystemExit("archive must contain exactly one bin/growthlab executable")
    binary = binaries[0]
    print(f"contents: passed ({len(names)} entries, {len(list((root / 'licenses').glob('*')))} notices)")

    archive_name = archive.name
    host = platform.system().lower()
    machine = platform.machine().lower()
    runnable = (("macos-arm64" in archive_name and host == "darwin" and machine in {"arm64", "aarch64"})
                or ("macos-x86_64" in archive_name and host == "darwin" and machine in {"x86_64", "amd64"})
                or ("linux-x86_64" in archive_name and host == "linux" and machine in {"x86_64", "amd64"})
                or ("linux-arm64" in archive_name and host == "linux" and machine in {"arm64", "aarch64"})
                or ("windows-x86_64" in archive_name and host == "windows"))
    if not runnable:
        print(f"runtime: skipped (archive target does not match {host}/{machine})")
    else:
        binary.chmod(binary.stat().st_mode | 0o111)
        version = subprocess.run([str(binary), "--no-telemetry", "version"], check=True, text=True, capture_output=True).stdout.strip()
        channel = subprocess.run([str(binary), "--no-telemetry", "version", "--build-channel"], check=True, text=True, capture_output=True).stdout.strip()
        if not version.startswith("growthlab ") or channel not in {"development", "production"}:
            raise SystemExit(f"unexpected runtime output: {version!r}, {channel!r}")
        print(f"runtime: passed ({version}; build channel {channel})")
PY
