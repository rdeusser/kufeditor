#!/bin/sh
set -eu

violations="$(
    rg --no-filename --only-matching --pcre2 \
        '\b(?:[A-Z][A-Za-z0-9]*?(?:Id|Api|Http)(?:[A-Z0-9][A-Za-z0-9]*)?|[A-Z][A-Za-z0-9]*?(?:Sox|Utf8|Ascii)(?:[A-Z0-9][A-Za-z0-9]*)?|(?:Sox|Utf8|Ascii)[A-Za-z0-9]+|MacOs|DefaultUnitHp|UnitHpLevelUp|UnitUvInfo|UnitUvid)\b' \
        crates -g '*.rs' -g '!**/generated/**' |
        sort -u |
        awk '$0 != "ElementId" && $0 != "GlobalElementId" && $0 != "InspectorElementId" && $0 != "LayoutId" && $0 != "UnexpectedEof" && $0 != "Utf8Error"'
)"

if [ -n "$violations" ]; then
    printf '%s\n' 'project-owned Rust identifiers contain mixed-case acronyms:' >&2
    printf '%s\n' "$violations" >&2
    exit 1
fi
