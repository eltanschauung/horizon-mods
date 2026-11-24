#!/usr/bin/env python3
"""Build one SDK package from the maintained native source and content snapshot."""

import argparse
from pathlib import Path
import shutil
import subprocess
from tempfile import TemporaryDirectory

ROOT = Path(__file__).resolve().parents[1]
MOD_ID = "horizon_gensokyo_legacy"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--assemble", action="store_true", help="Regenerate the content snapshot first")
    parser.add_argument("--tool", default="cargo-horizon-mod", help="Path to the SDK packaging tool")
    args = parser.parse_args()

    tool = shutil.which(args.tool)
    if tool is None:
        parser.error(f"SDK tool not found: {args.tool}")
    tool = Path(tool).resolve()

    if args.assemble:
        subprocess.run(["python3", ROOT / "tools" / "assemble.py"], check=True)

    output = ROOT / "target" / "horizon-mod"
    output.mkdir(parents=True, exist_ok=True)
    with TemporaryDirectory(prefix="gensokyo-sdk-", dir=output.parent) as temp:
        project = Path(temp)
        shutil.copytree(ROOT / "sdk", project, dirs_exist_ok=True, ignore=shutil.ignore_patterns(".horizon", "target"))
        for folder in ("assets", "data"):
            shutil.copytree(ROOT / "content" / folder, project / folder)
        subprocess.run([tool, "build"], cwd=project, check=True)
        package = project / "target" / "horizon-mod" / f"{MOD_ID}.hmod"
        subprocess.run([tool, "check", package, "--json"], check=True)
        destination = output / package.name
        package.replace(destination)
    print(destination)


if __name__ == "__main__":
    main()
