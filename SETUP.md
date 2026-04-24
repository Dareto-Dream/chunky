# Setup

## Requirements

| Tool | Minimum version | Install |
|------|----------------|---------|
| Rust toolchain | 1.78 | [rustup.rs](https://rustup.rs) |
| Git | any | [git-scm.com](https://git-scm.com) |
| GPU driver | Vulkan, Metal, or DX12 | Your GPU vendor |

Chunky uses **wgpu 0.20** which selects the best backend automatically:
- **Windows** — Vulkan (preferred) or DX12
- **macOS** — Metal
- **Linux** — Vulkan

---

## Clone and build

```bash
git clone <repo-url>
cd chunky

# Development build (faster compile, slower runtime):
cargo run

# Release build (recommended for real use):
cargo run --release
```

First compile downloads ~80 dependencies and takes 2–5 minutes. Subsequent builds are incremental.

---

## Running the binary directly

```bash
# Build release binary
cargo build --release

# Binary is at:
# Windows:  target\release\chunky.exe
# macOS:    target/release/chunky
# Linux:    target/release/chunky
```

Double-click `chunky.exe` on Windows, or run from terminal on macOS/Linux.

---

## Directory structure

```
chunky/
├── Cargo.toml              — dependencies and build profile
├── shaders/
│   ├── voxel.wgsl          — instanced cube shader (Lambertian lighting)
│   └── blit.wgsl           — fullscreen triangle blit (offscreen → egui)
└── src/
    ├── main.rs             — eframe entry point
    ├── app.rs              — app state, worker thread dispatch
    ├── ui/
    │   └── mod.rs          — egui UI, viewport wgpu callback
    ├── core/
    │   ├── scene.rs        — Scene/Mesh/Material structs
    │   ├── voxel.rs        — VoxelGrid (chunked HashMap of 32³ chunks)
    │   ├── import.rs       — OBJ / GLTF / STL loader
    │   ├── voxelize.rs     — surface voxelization (SAT triangle–AABB, rayon)
    │   ├── palette.rs      — block database, LAB ΔE color matching
    │   ├── optimize.rs     — noise filter, hidden-face flag
    │   └── export.rs       — NBT writer, .schem (Sponge v3), .mca region files
    └── renderer/
        ├── camera.rs       — orbit camera
        └── voxel_renderer.rs — wgpu instanced renderer + offscreen depth pass
```

---

## Common issues

### Black screen / no voxels rendered
Make sure your GPU drivers support Vulkan (Windows/Linux) or Metal (macOS). Update drivers if the app crashes on startup.

### "thread 'main' panicked" at startup
Run from a terminal to see the error message. Most startup panics are missing GPU features — update your driver.

### Very slow voxelization
Lower the resolution slider (e.g. 32 blocks/unit instead of 128). Each doubling of resolution increases voxel count 8×.

### WorldEdit says "invalid schematic"
Make sure the Minecraft version in **Export** matches your WorldEdit/FAWE version. Use **Java 1.21** for current servers.

### .mca file not loading in Minecraft
Place the `.mca` file in `saves/<world>/region/`. The region coordinate in the filename (`r.X.Z.mca`) is set automatically from the export offset. If the export offset is (0,64,0), the file is `r.0.0.mca`.

---

## Development notes

- Voxel shader outputs **linear** colors; the offscreen texture is `Rgba8UnormSrgb` so the GPU handles sRGB encoding automatically.
- The 3D viewport renders to an offscreen color+depth texture in `prepare()`, then blits to the egui render pass in `paint()`. This is necessary because egui's main render pass has no depth attachment.
- Transforms (scale / offset) are applied to a **cloned** copy of the scene on the worker thread, so you can adjust and re-voxelize without re-importing.
