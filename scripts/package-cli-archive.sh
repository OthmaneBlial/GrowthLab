#!/usr/bin/env bash
# Build a deterministic, portable CLI archive for the current Rust target.
#
# This is intentionally a local packaging helper. It never uploads a release,
# signs a binary, or contacts a provider. Cross-platform release jobs use the
# same file layout through cargo-dist; this helper is useful for reproducing
# and inspecting one target on the machine where it is run.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="$ROOT/dist/archives"
TARGET=""
SKIP_BUILD=0

usage() {
  cat <<'USAGE'
Usage: scripts/package-cli-archive.sh [--target <rust-target>] [--output-dir <dir>] [--skip-build]

Build a release growthlab binary and package it with the README, demo guide,
MIT license, attribution notice, and every checked-in dependency notice.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target)
      [[ $# -ge 2 ]] || { echo "--target needs a value" >&2; exit 2; }
      TARGET="$2"
      shift 2
      ;;
    --output-dir)
      [[ $# -ge 2 ]] || { echo "--output-dir needs a value" >&2; exit 2; }
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --skip-build)
      SKIP_BUILD=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

VERSION="$(sed -n '/^\[package\]/,/^\[/ { s/^version = "\(.*\)"/\1/p; }' "$ROOT/Cargo.toml" | head -1)"
[[ -n "$VERSION" ]] || { echo "Could not read package version" >&2; exit 1; }

if [[ -z "$TARGET" ]]; then
  TARGET="$(rustc -vV | sed -n 's/^host: //p')"
fi

case "$TARGET" in
  aarch64-apple-darwin) PLATFORM="macos-arm64"; BIN_NAME="growthlab" ;;
  x86_64-apple-darwin) PLATFORM="macos-x86_64"; BIN_NAME="growthlab" ;;
  x86_64-unknown-linux-musl) PLATFORM="linux-x86_64-musl"; BIN_NAME="growthlab" ;;
  aarch64-unknown-linux-musl) PLATFORM="linux-arm64-musl"; BIN_NAME="growthlab" ;;
  x86_64-pc-windows-msvc) PLATFORM="windows-x86_64"; BIN_NAME="growthlab.exe" ;;
  *)
    echo "Unsupported packaging target '$TARGET'. Use one of the documented release targets." >&2
    exit 2
    ;;
esac

HOST_TARGET="$(rustc -vV | sed -n 's/^host: //p')"
if [[ "$TARGET" == "$HOST_TARGET" ]]; then
  BINARY="$ROOT/target/release/$BIN_NAME"
else
  BINARY="$ROOT/target/$TARGET/release/$BIN_NAME"
fi

if [[ "$SKIP_BUILD" != 1 ]]; then
  if [[ "$TARGET" == "$HOST_TARGET" ]]; then
    cargo build --release --locked --bin growthlab
  elif [[ "$TARGET" == *-unknown-linux-musl && -n "$(command -v cargo-zigbuild 2>/dev/null || true)" ]]; then
    # cargo-zigbuild supplies a reproducible Zig linker for cross-target musl
    # builds. Keep plain Cargo as the fallback so hosts without Zig get the
    # native toolchain error instead of a hidden dependency installation.
    cargo zigbuild --release --locked --target "$TARGET" --bin growthlab
  else
    cargo build --release --locked --target "$TARGET" --bin growthlab
  fi
fi

[[ -f "$BINARY" && ! -L "$BINARY" ]] || {
  echo "Release binary not found at $BINARY" >&2
  exit 1
}

mkdir -p "$OUTPUT_DIR"
ARCHIVE="$OUTPUT_DIR/growthlab-v${VERSION}-${PLATFORM}.tar.gz"
python3 - "$ROOT" "$BINARY" "$ARCHIVE" "$VERSION" "$PLATFORM" <<'PY'
from __future__ import annotations

import gzip
import pathlib
import sys
import tarfile

root, binary, archive, version, platform = map(pathlib.Path, sys.argv[1:])
archive.parent.mkdir(parents=True, exist_ok=True)
root_name = f"growthlab-v{version}-{platform}"
files = [
    (binary, "bin/" + binary.name),
    (root / "LICENSE", "LICENSE"),
    (root / "NOTICE.md", "NOTICE.md"),
    (root / "README.md", "README.md"),
    (root / "docs" / "demo.md", "docs/demo.md"),
    (root / "docs" / "distribution.md", "docs/distribution.md"),
]
files.extend((path, f"licenses/{path.name}") for path in sorted((root / "licenses").glob("*")) if path.is_file())
for source, _ in files:
    if not source.is_file() or source.is_symlink():
        raise SystemExit(f"required distribution file is missing or symlinked: {source}")

with archive.open("wb") as output:
    with gzip.GzipFile(fileobj=output, mode="wb", mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode="w") as bundle:
            for source, relative in files:
                info = bundle.gettarinfo(str(source), arcname=f"{root_name}/{relative}")
                info.mtime = 0
                info.uid = info.gid = 0
                info.uname = info.gname = ""
                info.mode = 0o755 if relative.startswith("bin/") else 0o644
                with source.open("rb") as stream:
                    bundle.addfile(info, stream)
PY

CHECKSUM="$ARCHIVE.sha256"
DIGEST="$(shasum -a 256 "$ARCHIVE" | awk '{print $1}')"
printf '%s  %s\n' "$DIGEST" "$(basename "$ARCHIVE")" > "$CHECKSUM"

echo "Archive: $ARCHIVE"
echo "SHA-256: $DIGEST"
