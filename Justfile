set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

linux_target := env_var_or_default("LINUX_TARGET", "x86_64-unknown-linux-musl")
win_target := env_var_or_default("WIN_TARGET", "x86_64-pc-windows-gnu")

default:
    @just --list

# Build release binary for the current platform.
build:
    cargo build --release

# Cross-build Linux release binary via cargo-zigbuild.
build-linux:
    cargo zigbuild --release --target {{ linux_target }}

# Cross-build Windows release binary via cargo-zigbuild.
build-win:
    cargo zigbuild --release --target {{ win_target }}
