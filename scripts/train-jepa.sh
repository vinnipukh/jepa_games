#!/usr/bin/env bash
# The whole Phase 7 experiment on your own (GPU) machine, in one command:
#
#   scripts/train-jepa.sh                    # full preset → reports/jepa_eval.md (+ .json)
#   PRESET=default scripts/train-jepa.sh     # default configuration only, one seed (quick check)
#   scripts/train-jepa.sh --device cpu       # extra arguments go to `python -m jepa.pipeline`
#
# Needs git, a C toolchain for Rust, and network access; installs rustup / uv when missing
# (scripts/setup-cloud.sh --jepa). torch is installed for the machine's CUDA driver
# (TORCH_BACKEND=auto; set e.g. TORCH_BACKEND=cu128 or cpu to override). Every stage resumes
# from its outputs under data/jepa/<preset>, so rerunning after an interruption continues.
# TensorBoard: `uv run --no-project tensorboard --logdir ../data/jepa/full/runs` from python/.
set -euo pipefail
cd "$(dirname "$0")/.."
scripts/setup-cloud.sh --jepa
preset="${PRESET:-full}"
report="reports/jepa_eval.md"
[ "$preset" = full ] || report="reports/jepa_eval_${preset}.md"
cd python
exec uv run --no-project python -m jepa.pipeline --preset "$preset" --report "../$report" "$@"
