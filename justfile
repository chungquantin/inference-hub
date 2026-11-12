# Inference Hub task runner.
# GUI (Next.js web app in apps/web) and Rust (Cargo workspace) live in module
# subdirectories so recipes read as `just gui/dev`, `just rs/format`, etc.
# Run `just gui` or `just rs` to list recipes in that module.

mod gui
mod rs

# Default: list recipes from all modules.
default:
    @just --list --list-submodules
