#!/usr/bin/env python3
"""Bring Touhou Mobs fairy Extend Item drops in line with Arcadian Dream."""
import argparse
import json
from pathlib import Path
from zipfile import ZipFile

SOURCE = "mixins/thmobs_native/api/src/lib.rs"
OLD = '''        if roll(10) == 0 {
            drops.push(("arcadiandream:extend_item", 1));
        }'''
NEW = OLD.replace("roll(10)", "roll(40)")

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.source.resolve() == args.output.resolve():
        parser.error("choose a separate output")
    with ZipFile(args.source) as original:
        if json.loads(original.read("horizon-native.json"))["id"] != "thmobs_native":
            parser.error("expected the Touhou Mobs native package")
        text = original.read(SOURCE).decode()
        if text.count(OLD) == 1:
            text = text.replace(OLD, NEW).replace("10 times in 100 each, a star 15", "10% and 2.5% respectively, a star 15%")
        elif text.count(NEW) != 1:
            parser.error("the expected fairy loot rule changed")
        with ZipFile(args.output, "w") as output:
            for entry in original.infolist():
                output.writestr(entry, text.encode() if entry.filename == SOURCE else original.read(entry))

if __name__ == "__main__":
    main()
