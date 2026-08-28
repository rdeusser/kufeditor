#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd -- "${script_dir}/.." && pwd)"
cleave_command="${CLEAVE:-cleave}"
rustfmt_command="${RUSTFMT:-rustfmt}"
rust_output="${project_root}/crates/kufeditor-formats/src/generated"

shopt -s nullglob
rust_schemas=(
    "${project_root}/schemas/kuf_save.clv"
    "${project_root}/schemas/kuf_stg.clv"
    "${project_root}"/schemas/sox_*.clv
)
if [[ ! -f "${rust_schemas[0]}" ]] || [[ ! -f "${rust_schemas[1]}" ]] ||
    ((${#rust_schemas[@]} == 2)); then
    printf 'error: no Rust schema inputs found\n' >&2
    exit 1
fi

generation_staging="$(mktemp -d)"
rust_staging="${generation_staging}/rust"
publication_temps=()

cleanup_generation() {
    if ((${#publication_temps[@]} > 0)); then
        for temporary in "${publication_temps[@]}"; do
            if [[ -e "${temporary}" ]]; then
                rm -- "${temporary}"
            fi
        done
    fi
    rm -rf -- "${generation_staging}"
}
trap cleanup_generation EXIT

mkdir -p "${rust_output}" "${rust_staging}"

for schema in "${rust_schemas[@]}"; do
    "${cleave_command}" generate \
        --lang rust \
        --no-cargo \
        --out "${rust_staging}" \
        "${schema}"
done

staged_files=()
destinations=()

for schema in "${rust_schemas[@]}"; do
    module_name="${schema##*/}"
    module_name="${module_name%.clv}.rs"
    staged_file="${rust_staging}/${module_name}"
    if [[ ! -f "${staged_file}" ]]; then
        printf 'error: Cleave generated no Rust schema module %s\n' "${module_name}" >&2
        exit 1
    fi
    "${rustfmt_command}" --edition 2024 "${staged_file}"
    staged_files+=("${staged_file}")
    destinations+=("${rust_output}/${module_name}")
done

for index in "${!staged_files[@]}"; do
    destination="${destinations[index]}"
    temporary="$(mktemp "${destination}.tmp.XXXXXX")"
    publication_temps+=("${temporary}")
    cp -p -- "${staged_files[index]}" "${temporary}"
done

for index in "${!publication_temps[@]}"; do
    mv -- "${publication_temps[index]}" "${destinations[index]}"
done

current_modules=("${rust_output}"/sox_*.rs)
for current_module in "${current_modules[@]}"; do
    module_name="${current_module##*/}"
    if [[ ! -e "${rust_staging}/${module_name}" ]]; then
        rm -- "${current_module}"
    fi
done
