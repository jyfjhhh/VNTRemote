#!/bin/bash
# VNTRemote macOS Build Script
# Requires: Rust, Node.js

set -e

echo "========================================"
echo "  VNTRemote - macOS Build"
echo "========================================"

# Check prerequisites
check_cmd() {
    if ! command -v "$1" &>/dev/null; then
        echo "[ERROR] $1 not found!"
        case "$1" in
            rustc) echo "  Install: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh" ;;
            node)  echo "  Install: brew install node" ;;
            cargo) echo "  Install: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh" ;;
        esac
        exit 1
    fi
    echo "[OK] $1: $($1 --version 2>&1 | head -1)"
}

check_cmd rustc
check_cmd node
check_cmd cargo

# Install frontend deps
echo ""
echo "[1/3] Installing frontend dependencies..."
cd frontend
npm install
cd ..

# Build
echo ""
echo "[2/3] Building VNTRemote..."
cargo tauri build --bundles dmg

echo ""
echo "[3/3] Done!"
echo ""
echo "Output: src-tauri/target/release/bundle/"
echo "  - VNTRemote.app  (App bundle)"
echo "  - VNTRemote_*.dmg (Installer)"
echo ""
