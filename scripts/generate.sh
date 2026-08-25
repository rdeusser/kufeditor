#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd -- "${script_dir}/.." && pwd)"
cleave_command="${CLEAVE:-cleave}"
cpp_output="${project_root}/src/parsers"
rust_output="${project_root}/crates/kufeditor-formats/src/generated"

mkdir -p "${cpp_output}" "${rust_output}"

for schema in "${project_root}"/schemas/*.clv; do
    "${cleave_command}" generate --lang cpp --out "${cpp_output}" "${schema}"
done

"${cleave_command}" generate \
    --lang rust \
    --no-cargo \
    --out "${rust_output}" \
    "${project_root}/schemas/sox_troop_info.clv"

"${cleave_command}" generate \
    --lang rust \
    --no-cargo \
    --out "${rust_output}" \
    "${project_root}/schemas/sox_skill_info.clv"
