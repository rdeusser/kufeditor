#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd -- "${script_dir}/.." && pwd)"

"${script_dir}/generate.sh"
git -C "${project_root}" diff --exit-code -- \
    schemas \
    crates/kufeditor-formats/src/generated
