#!/bin/bash
# Patch cargo dependencies for compatibility with bleeding-edge toolchains
# This script fixes issues when building with bundled SDL2/FFmpeg on systems with:
# - CMake 4.x (drops support for cmake_minimum_required < 3.5)
# - GCC 15+ (defaults to C23, conflicts with old SDL2 code)
# - PipeWire 1.4+ (API incompatibilities with SDL 2.26.x)

set -e

REGISTRY_DIR="${CARGO_HOME:-$HOME/.cargo}/registry/src"

echo "Searching for dependencies to patch in $REGISTRY_DIR..."

# Find SDL2 source directory
SDL2_DIR=$(find "$REGISTRY_DIR" -type d -path "*/sdl2-sys-*/SDL" 2>/dev/null | head -n 1)
if [ -n "$SDL2_DIR" ]; then
    echo "Found SDL2 at: $SDL2_DIR"

    # Patch 1: Fix CMakeLists.txt cmake_minimum_required versions
    if grep -q "cmake_minimum_required(VERSION 3.0.0)" "$SDL2_DIR/CMakeLists.txt" 2>/dev/null; then
        echo "  Patching SDL2 CMakeLists.txt (version 3.0.0 -> 3.5.0)..."
        sed -i 's/cmake_minimum_required(VERSION 3\.0\.0)/cmake_minimum_required(VERSION 3.5.0)/' "$SDL2_DIR/CMakeLists.txt"
    fi

    if grep -q "cmake_minimum_required(VERSION 3.4)" "$SDL2_DIR/CMakeLists.txt" 2>/dev/null; then
        echo "  Patching SDL2 CMakeLists.txt (version 3.4 -> 3.5)..."
        sed -i 's/cmake_minimum_required(VERSION 3\.4)/cmake_minimum_required(VERSION 3.5)/' "$SDL2_DIR/CMakeLists.txt"
    fi

    # Patch 2: Disable PipeWire and force C11 in build.rs
    SDL2_BUILD_RS=$(dirname "$SDL2_DIR")/build.rs
    if [ -f "$SDL2_BUILD_RS" ]; then
        if ! grep -q "SDL_PIPEWIRE" "$SDL2_BUILD_RS" 2>/dev/null; then
            echo "  Patching SDL2 build.rs (disable PipeWire, force C11)..."
            # Find the line with cfg.build() and add patches before it
            sed -i '/cfg\.build()/i\    // Disable PipeWire to avoid compatibility issues with newer versions\n    cfg.define("SDL_PIPEWIRE", "OFF");\n\n    // Force C11 standard to avoid C23 keyword conflicts with old SDL2 code\n    cfg.cflag("-std=c11");\n' "$SDL2_BUILD_RS"
        fi
    fi
else
    echo "SDL2 not found (might be using system library)"
fi

# Find FFmpeg build script
FFMPEG_BUILD_RS=$(find "$REGISTRY_DIR" -type f -path "*/ffmpeg-sys-the-third-*/build.rs" 2>/dev/null | head -n 1)
if [ -n "$FFMPEG_BUILD_RS" ]; then
    echo "Found FFmpeg build.rs at: $FFMPEG_BUILD_RS"

    # Patch 3: Disable FFmpeg documentation build
    if ! grep -q '"--disable-doc"' "$FFMPEG_BUILD_RS" 2>/dev/null; then
        echo "  Patching FFmpeg build.rs (disable doc build)..."
        # Add --disable-doc after --disable-programs
        sed -i '/configure\.arg("--disable-programs");/a\    \n    // do not build documentation\n    configure.arg("--disable-doc");' "$FFMPEG_BUILD_RS"
    fi
else
    echo "FFmpeg build.rs not found (might be using system library)"
fi

echo "Patching complete!"
