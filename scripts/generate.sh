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

rust_staging="$(mktemp -d)"
cleanup_rust_staging() {
    rm -rf -- "${rust_staging}"
}
trap cleanup_rust_staging EXIT

shopt -s nullglob
rust_schemas=("${project_root}"/schemas/sox_*.clv)
if ((${#rust_schemas[@]} == 0)); then
    printf 'error: no Rust SOX schemas match %s\n' "${project_root}/schemas/sox_*.clv" >&2
    exit 1
fi

for schema in "${rust_schemas[@]}"; do
    "${cleave_command}" generate \
        --lang rust \
        --no-cargo \
        --out "${rust_staging}" \
        "${schema}"
done

staged_modules=("${rust_staging}"/sox_*.rs)
if ((${#staged_modules[@]} == 0)); then
    printf 'error: Cleave generated no Rust SOX modules\n' >&2
    exit 1
fi

current_modules=("${rust_output}"/sox_*.rs)
for current_module in "${current_modules[@]}"; do
    module_name="${current_module##*/}"
    if [[ ! -e "${rust_staging}/${module_name}" ]]; then
        rm -- "${current_module}"
    fi
done

for staged_module in "${staged_modules[@]}"; do
    cp -- "${staged_module}" "${rust_output}/${staged_module##*/}"
done
