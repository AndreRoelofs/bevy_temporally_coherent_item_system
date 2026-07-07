# Cross-language entry points; `just --list` shows everything.

_default:
    @just --list --unsorted

# One-time dev setup: Python env + git hooks.
setup:
    uv sync
    uv run prek install --hook-type pre-commit --hook-type commit-msg

# Run the game.
play:
    cargo run

# Run the game with in-game debug overlays compiled in.
debug:
    cargo run --features debug

# Run the game with per-frame cost diagnostics logged to stdout every second
# (frame time / FPS, entity count, per-render-pass GPU timings). Same dev
# profile as `just play`, so the numbers match normal play.
profile:
    cargo run --features profile

# Build the shipping binary: fat LTO + panic=abort + stripped, with Bevy
# statically linked (--no-default-features drops `dev`/dynamic_linking). Slow to
# compile — this is the max-performance artifact, not an iteration build.
dist:
    cargo build --profile dist --no-default-features

# The shipping profile, but run it — for measuring true release performance.
play-dist:
    cargo run --profile dist --no-default-features

# Everything CI gates on.
check: check-rust check-python

# Clippy; locally also cargo machete + typos.
check-rust:
    scripts/clippy.sh

check-python:
    uv run ruff format --check .
    uv run ruff check .
    uv run ty check
    uv run pytest

# Rust tests (the Python ones run in check-python).
test:
    cargo test --workspace

# Auto-format and apply safe lint fixes.
fix:
    cargo fmt --all
    uv run ruff format .
    uv run ruff check --fix .
    typos --write-changes

# Interactive conventional-commit wizard. Hooks run up front so a failure
# doesn't discard your wizard answers.
commit:
    uv run prek run
    uv run cz commit
