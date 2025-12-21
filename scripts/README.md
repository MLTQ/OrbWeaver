# Build Scripts

## patch_deps.sh

This script patches vendored Rust dependencies to fix compatibility issues with bleeding-edge toolchains.

### Why is this needed?

When building with bundled SDL2 and FFmpeg (static linking), certain newer system toolchains have incompatibilities with the older vendored source code:

1. **CMake 4.x compatibility** (Arch Linux, Fedora Rawhide, etc.)
   - CMake 4.0+ removed support for `cmake_minimum_required` < 3.5
   - SDL 2.26.x uses `cmake_minimum_required(VERSION 3.0.0)` and `3.4`
   - **Fix:** Update to version 3.5 in `SDL/CMakeLists.txt`

2. **GCC 15+ / C23 keyword conflicts**
   - GCC 15+ defaults to C23 standard
   - C23 made `bool`, `true`, `false` keywords
   - Old SDL2 code defines these as enum values
   - **Fix:** Force C11 standard with `-std=c11` in SDL2 build.rs

3. **PipeWire 1.4+ API incompatibility**
   - SDL 2.26.x uses old PipeWire API
   - PipeWire 1.4+ changed function signatures
   - **Fix:** Disable PipeWire support in SDL2 build.rs

4. **FFmpeg documentation build failures**
   - Some systems have incompatible Texinfo/Perl versions
   - Documentation build isn't needed for static linking
   - **Fix:** Add `--disable-doc` to FFmpeg configure

### When does this run?

- Automatically in GitHub Actions workflows before building
- Can be run manually on local machines: `./scripts/patch_deps.sh`
- Only patches files if they exist (no-op if using system libraries)

### Long-term solution

These patches are workarounds for using old vendored dependencies. Consider:
- Updating to newer `egui`/`sdl2` crates that use SDL 2.28+ or SDL3
- Using system libraries instead of bundled (loses portability)
- Contributing fixes upstream to sdl2-sys crate

### Related files

- `.github/workflows/rust.yml` - CI build workflow
- `.github/workflows/release.yml` - Release build workflow
- `Cargo.toml` - Could use `[patch.crates-io]` for more permanent fixes
