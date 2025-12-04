# Build Guide

This guide explains how to build `graphchan_desktop` for Linux and Windows.

## Linux Build (via Docker)

Since the project depends on `SDL2` and `FFmpeg`, cross-compiling from macOS to Linux is complex. The recommended approach is to use Docker to build a compatible Linux binary.

### Prerequisites
- Docker Desktop installed and running.

### Steps

1.  **Build the Docker image:**
    Run this command from the root of the project (where the `Dockerfile` is):
    ```bash
    docker build -t graphchan-builder .
    ```
    *Note: This may take a while as it compiles FFmpeg and other dependencies.*

2.  **Extract the binary:**
    Once the build is complete, create a container and copy the binary out:
    ```bash
    # Create a dummy container
    docker create --name graphchan-temp graphchan-builder

    # Copy the binary to your current directory
    docker cp graphchan-temp:/usr/src/app/target/release/graphchan_desktop ./graphchan_desktop_linux

    # Remove the dummy container
    docker rm graphchan-temp
    ```

3.  **Run:**
    Transfer `graphchan_desktop_linux` to your Linux machine and run it. You may need to ensure the target machine has basic runtime libraries installed (like `libasound2`, `libsdl2-2.0-0` if not fully static, though we attempt static linking).

## Windows Build

Cross-compiling to Windows from macOS with these dependencies is highly experimental and prone to failure. The best way is to build on a Windows machine.

### Prerequisites on Windows
1.  **Install Rust:** Download `rustup-init.exe` from [rust-lang.org](https://www.rust-lang.org/tools/install).
2.  **Install C++ Build Tools:** During Rust installation, it will prompt for Visual Studio C++ Build Tools. Install them.
3.  **Install LLVM (Optional but recommended):** Some crates might need `clang`. You can install it via `winget install LLVM` or from the LLVM website.
4.  **Install NASM:** Required for FFmpeg compilation.
    - Download from [nasm.us](https://www.nasm.us/).
    - Add the installation path (e.g., `C:\Program Files\NASM`) to your system `PATH` environment variable.

### Steps

1.  **Clone the repository** on your Windows machine.
2.  **Open PowerShell** or Command Prompt in the project root.
3.  **Build:**
    ```powershell
    cargo build --release --bin graphchan_desktop
    ```
4.  **Locate Binary:**
    The executable will be at `target\release\graphchan_desktop.exe`.

## macOS Build (Native)

Since you are on macOS, you can build natively:

```bash
cargo build --release --bin graphchan_desktop
```
The binary will be in `target/release/graphchan_desktop`.

## GitHub Actions (Automated Builds)

The easiest way to get a Windows binary without a Windows machine is to use the included GitHub Actions workflow.

1.  Push your code to GitHub.
2.  Go to the **Actions** tab in your repository.
3.  Select **Release Build** on the left.
4.  Click **Run workflow**.
5.  Once finished, you can download the `graphchan_desktop-windows` artifact from the run summary page.
