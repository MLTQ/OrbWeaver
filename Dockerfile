# Use a recent Rust image
FROM rust:latest as builder

# Install build dependencies
# ffmpeg needs yasm/nasm
# sdl2 needs cmake and system libs even for static linking (X11, Wayland, etc.)
# rodio needs alsa
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    libsdl2-dev \
    libasound2-dev \
    libudev-dev \
    clang \
    cmake \
    yasm \
    nasm \
    libx11-dev \
    libxext-dev \
    libxrender-dev \
    libxinerama-dev \
    libxi-dev \
    libxrandr-dev \
    libxcursor-dev \
    libgl1-mesa-dev \
    libglu1-mesa-dev \
    libwayland-dev \
    libxkbcommon-dev

# Create a new empty shell project
WORKDIR /usr/src/app
COPY . .

# Build the release binary
# We use --bin graphchan_desktop to specifically build the desktop app
RUN cargo build --release --bin graphchan_desktop

# Create a minimal runtime image (optional, but good for testing)
# or just output the binary. Here we just keep the builder for simplicity of extraction.
