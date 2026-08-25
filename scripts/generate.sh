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
rust_schemas=(
    "${project_root}/schemas/kuf_save.clv"
    "${project_root}"/schemas/sox_*.clv
)
if [[ ! -f "${rust_schemas[0]}" ]] || ((${#rust_schemas[@]} == 1)); then
    printf 'error: no Rust schema inputs found\n' >&2
    exit 1
fi

for schema in "${rust_schemas[@]}"; do
    "${cleave_command}" generate \
        --lang rust \
        --no-cargo \
        --out "${rust_staging}" \
        "${schema}"
done

staged_modules=()
for schema in "${rust_schemas[@]}"; do
    module_name="${schema##*/}"
    module_name="${module_name%.clv}.rs"
    staged_module="${rust_staging}/${module_name}"
    if [[ ! -f "${staged_module}" ]]; then
        printf 'error: Cleave generated no Rust schema module %s\n' "${module_name}" >&2
        exit 1
    fi
    staged_modules+=("${staged_module}")
done

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
