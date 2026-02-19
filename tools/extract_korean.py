#!/usr/bin/env python3
"""Extract unique Korean strings from KUF game files.

Scans .stg and .sox files for CP949-encoded Korean text, decodes it to
UTF-8, and prints deduplicated strings one per line -- ready for pasting
into Google Translate.

Only matches bytes in the standard CP949 Hangul range (lead 0xB0-0xC8,
trail 0xA1-0xFE) to avoid false positives from binary data.

Skips non-Korean localization directories (CHS, ENG, FRA, GER, ITA, JAP)
since those contain Chinese/Japanese/Latin text whose bytes produce
nonsensical Korean when decoded as CP949.

Usage:
    python3 tools/extract_korean.py <game_directory>

    # With source file context:
    python3 tools/extract_korean.py --verbose <game_directory>
"""

import sys
from pathlib import Path

# Localization directories that contain non-Korean text.
SKIP_DIRS = {"CHS", "CHT", "ENG", "FRA", "GER", "ITA", "JAP", "KOR", "SPA", "RUS"}


def is_hex_encoded(data: bytes) -> bool:
    if len(data) < 16:
        return False
    return all(b in b"0123456789abcdefABCDEF\r\n " for b in data[:64])


def hex_decode(data: bytes) -> bytes:
    try:
        return bytes.fromhex(data.decode("ascii").strip())
    except (ValueError, UnicodeDecodeError):
        return data


def is_cp949_hangul(b0: int, b1: int) -> bool:
    """Standard CP949 Hangul syllable range (EUC-KR compatible)."""
    return 0xB0 <= b0 <= 0xC8 and 0xA1 <= b1 <= 0xFE


def extract_korean(data: bytes) -> set[str]:
    strings: set[str] = set()
    i = 0
    end = len(data) - 1

    while i < end:
        if is_cp949_hangul(data[i], data[i + 1]):
            start = i
            last_hangul_end = i
            while i < end:
                if is_cp949_hangul(data[i], data[i + 1]):
                    i += 2
                    last_hangul_end = i
                elif (
                    data[i] == 0x20
                    and i + 2 < len(data)
                    and is_cp949_hangul(data[i + 1], data[i + 2])
                ):
                    i += 1
                else:
                    break
            i = last_hangul_end
            chunk = data[start:i]
            try:
                text = chunk.decode("cp949")
                if len(text) >= 2:
                    strings.add(text)
            except (UnicodeDecodeError, ValueError):
                pass
        else:
            i += 1

    return strings


def in_skip_dir(filepath: Path, game_dir: Path) -> bool:
    rel = filepath.relative_to(game_dir)
    return any(part in SKIP_DIRS for part in rel.parts)


def scan_file(filepath: Path) -> set[str]:
    try:
        data = filepath.read_bytes()
    except OSError:
        return set()

    if is_hex_encoded(data):
        data = hex_decode(data)

    return extract_korean(data)


def main():
    verbose = "--verbose" in sys.argv or "-v" in sys.argv
    args = [a for a in sys.argv[1:] if not a.startswith("-")]

    if not args:
        print(f"Usage: {sys.argv[0]} [-v|--verbose] <game_directory>", file=sys.stderr)
        sys.exit(1)

    game_dir = Path(args[0])
    if not game_dir.is_dir():
        print(f"Error: {game_dir} is not a directory", file=sys.stderr)
        sys.exit(1)

    extensions = {".stg", ".sox"}
    all_strings: set[str] = set()
    string_sources: dict[str, list[str]] = {}

    for filepath in sorted(game_dir.rglob("*")):
        if not filepath.is_file():
            continue
        if filepath.suffix.lower() not in extensions:
            continue
        if in_skip_dir(filepath, game_dir):
            continue

        strings = scan_file(filepath)
        all_strings.update(strings)
        if verbose:
            relpath = str(filepath.relative_to(game_dir))
            for s in strings:
                string_sources.setdefault(s, []).append(relpath)

    for s in sorted(all_strings):
        if verbose:
            sources = ", ".join(sorted(set(string_sources.get(s, []))))
            print(f"{s}\t# {sources}")
        else:
            print(s)

    print(f"\n# {len(all_strings)} unique Korean strings", file=sys.stderr)


if __name__ == "__main__":
    main()
