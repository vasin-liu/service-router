#!/usr/bin/env python3
"""Fail if tracked text-like files contain NUL bytes.

This catches accidental UTF-16LE writes on Windows before rustc/bash/Markdown
consumers see the files as binary text.
"""

from __future__ import annotations

from pathlib import Path
import subprocess
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
    tracked = git_tracked_files()
    if tracked is not None:
        return [p for p in tracked if p.suffix.lower() in TEXT_SUFFIXES]

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


def git_tracked_files() -> list[Path] | None:
    try:
        proc = subprocess.run(
            ["git", "ls-files", "-z"],
            cwd=ROOT,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
        )
    except (FileNotFoundError, subprocess.CalledProcessError):
        return None

    files = []
    for raw in proc.stdout.split(b"\0"):
        if raw:
            files.append(ROOT / raw.decode("utf-8"))
    return files


def looks_utf16le(data: bytes) -> bool:
    """Heuristic for UTF-16LE ASCII-ish text: many odd bytes are NUL."""
    if len(data) < 4:
        return False
    odd = data[1::2]
    return odd.count(0) >= max(2, len(odd) // 2)


def main() -> int:
    offenders: list[Path] = []
    utf16le_like: list[Path] = []
    for path in iter_text_candidates():
        data = path.read_bytes()
        if b"\x00" in data:
            rel = path.relative_to(ROOT)
            offenders.append(rel)
            if looks_utf16le(data):
                utf16le_like.append(rel)

    if offenders:
        print("NUL bytes found in text-like files (possible UTF-16LE):", file=sys.stderr)
        for path in offenders:
            print(f"  - {path}", file=sys.stderr)
        if utf16le_like:
            print("", file=sys.stderr)
            print("If these files are UTF-16LE, re-save them as UTF-8.", file=sys.stderr)
            print("One-file repair example:", file=sys.stderr)
            print(
                "  python -c \"from pathlib import Path; "
                "p=Path('FILE'); "
                "p.write_text(p.read_bytes().decode('utf-16le'), "
                "encoding='utf-8', newline='\\\\n')\"",
                file=sys.stderr,
            )
        return 1

    print("text encoding check OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
