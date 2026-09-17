#!/usr/bin/env python3
"""Verify the public GrowthLab release assets without provider access.

This is an explicit, read-only network check for release review. It confirms
the tag, downloads the expected archive/checksum pairs, compares GitHub's
asset digest with the downloaded bytes, validates each checksum sidecar, and
passes each archive through the local safety/notices verifier. It does not
claim runtime support for a target that cannot run on this host.
"""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
from urllib.request import Request, urlopen


DEFAULT_REPOSITORY = "OthmaneBlial/GrowthLab"
DEFAULT_TAG = "v0.1.0-alpha.20"
MAX_DOWNLOAD_BYTES = 64 * 1024 * 1024
ASSET_PAIRS = (
    (
        "growthlab-v0.1.0-alpha.20-macos-arm64.tar.gz",
        "growthlab-v0.1.0-alpha.20-macos-arm64.tar.gz.sha256",
    ),
    (
        "growthlab-v0.1.0-alpha.20-linux-x86_64-musl.tar.gz",
        "growthlab-v0.1.0-alpha.20-linux-x86_64-musl.tar.gz.sha256",
    ),
    (
        "growthlab-v0.1.0-alpha.20-windows-x86_64-gnu.zip",
        "growthlab-v0.1.0-alpha.20-windows-x86_64-gnu.zip.sha256",
    ),
    (
        "growthlab-v0.1.0-alpha.20-macos-x86_64.tar.gz",
        "growthlab-v0.1.0-alpha.20-macos-x86_64.tar.gz.sha256",
    ),
    (
        "growthlab-v0.1.0-alpha.20-linux-arm64-musl.tar.gz",
        "growthlab-v0.1.0-alpha.20-linux-arm64-musl.tar.gz.sha256",
    ),
)


def fetch(url, limit=MAX_DOWNLOAD_BYTES):
    request = Request(url, headers={"User-Agent": "GrowthLab-release-verifier/1"})
    with urlopen(request, timeout=30) as response:
        content_length = response.headers.get("Content-Length")
        if content_length and int(content_length) > limit:
            raise RuntimeError(f"Refusing oversized release asset ({content_length} bytes): {url}")
        data = response.read(limit + 1)
    if len(data) > limit:
        raise RuntimeError(f"Release asset exceeded {limit} bytes: {url}")
    return data


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description="Verify public GrowthLab release archive assets.")
    parser.add_argument("--repo", default=DEFAULT_REPOSITORY, help="Public GitHub repository owner/name.")
    parser.add_argument("--tag", default=DEFAULT_TAG, help="Release tag to verify.")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    api_url = f"https://api.github.com/repos/{args.repo}/releases/tags/{args.tag}"
    release = json.loads(fetch(api_url, limit=2 * 1024 * 1024))
    if release.get("tag_name") != args.tag or release.get("draft"):
        raise RuntimeError(f"Unexpected or draft release response for {args.repo}@{args.tag}")
    assets = {asset.get("name"): asset for asset in release.get("assets", [])}
    missing = [name for pair in ASSET_PAIRS for name in pair if name not in assets]
    if missing:
        raise RuntimeError("Release is missing expected assets: " + ", ".join(missing))

    with tempfile.TemporaryDirectory(prefix="growthlab-release-assets-") as temporary:
        root = Path(temporary)
        verified = []
        for archive_name, checksum_name in ASSET_PAIRS:
            archive_asset = assets[archive_name]
            checksum_asset = assets[checksum_name]
            archive = fetch(archive_asset["browser_download_url"])
            checksum_file = fetch(checksum_asset["browser_download_url"], limit=4096)
            archive_digest = sha256(archive)
            github_digest = (archive_asset.get("digest") or "").removeprefix("sha256:")
            if github_digest and archive_digest != github_digest:
                raise RuntimeError(
                    f"GitHub digest mismatch for {archive_name}: {archive_digest} != {github_digest}"
                )
            fields = checksum_file.decode("utf-8").split()
            if len(fields) < 2 or fields[1] != archive_name or fields[0].lower() != archive_digest.lower():
                raise RuntimeError(f"Checksum sidecar mismatch for {archive_name}")
            archive_path = root / archive_name
            archive_path.write_bytes(archive)
            (root / checksum_name).write_bytes(checksum_file)
            subprocess.run(
                [str(repo / "scripts/verify-release-archive.sh"), str(archive_path)],
                check=True,
                cwd=repo,
            )
            verified.append({"name": archive_name, "bytes": len(archive), "sha256": archive_digest})
    print(json.dumps({"repository": args.repo, "tag": args.tag, "draft": release.get("draft"), "assets": verified}))


if __name__ == "__main__":
    main()
