"""Package the website's exact three CLI assets from one CI revision."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil

ASSETS = ("snapshotdb-darwin-amd64", "snapshotdb-darwin-arm64", "snapshotdb-linux-amd64")


def package(source, destination, revision):
    source, destination = Path(source), Path(destination)
    if not re.fullmatch(r"[a-f0-9]{40}", revision):
        raise ValueError("A full lowercase Git revision is required")
    # Check the complete set before writing anything; never publish mixed builds.
    for name in ASSETS:
        if not (source / name).is_file() or (source / name).stat().st_size == 0:
            raise ValueError(f"Missing or empty asset: {name}")
    if destination.exists():
        raise ValueError("Destination must be new; refusing to mix release assets")
    destination.mkdir(parents=True)
    sums = {}
    for name in ASSETS:
        path = destination / name
        shutil.copyfile(source / name, path)
        path.chmod(0o755)
        sums[name] = hashlib.sha256(path.read_bytes()).hexdigest()
    (destination / "SHA256SUMS.txt").write_text("".join(f"{digest}  {name}\n" for name, digest in sums.items()))
    (destination / "BUILD.json").write_text(json.dumps({"revision": revision, "sha256": sums}, indent=2) + "\n")


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("source")
    p.add_argument("destination")
    p.add_argument("--revision", required=True)
    a = p.parse_args()
    package(a.source, a.destination, a.revision)
