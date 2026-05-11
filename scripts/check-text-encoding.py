#!/usr/bin/env python3
"""Fail if tracked text-like files contain NUL bytes.

This catches accidental UTF-16LE writes on Windows before rustc/bash/Markdown
consumers see the files as binary text.
"""

from __future__ import annotations

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]

TEXT_SUFFIXES = {
    ".md",
    ".rs",
    ".sh",
    ".ps1",
    ".py",
    ".mjs",
    ".toml",
    ".yaml",
    ".yml",
    ".json",
}

SKIP_DIRS = {
    ".git",
    "target",
    "artifacts",
    ".idea",
    ".vscode",
    "node_modules",
}


def iter_text_candidates() -> list[Path]:
    out: list[Path] = []
    for path in ROOT.rglob("*"):
        if not path.is_file():
            continue
        rel = path.relative_to(ROOT)
        if any(part in SKIP_DIRS for part in rel.parts):
            continue
        if path.suffix.lower() in TEXT_SUFFIXES:
            out.append(path)
    return out


def main() -> int:
    offenders: list[Path] = []
    for path in iter_text_candidates():
        data = path.read_bytes()
        if b"\x00" in data:
            offenders.append(path.relative_to(ROOT))

    if offenders:
        print("NUL bytes found in text-like files (possible UTF-16LE):", file=sys.stderr)
        for path in offenders:
            print(f"  - {path}", file=sys.stderr)
        return 1

    print("text encoding check OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
