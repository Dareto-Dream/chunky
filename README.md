# Chunky

A native desktop tool that converts 3D models into Minecraft structures. Import `.obj`, `.glb`, or `.stl` files and export Sponge `.schem` schematics or `.mca` region files ready to paste into a world.

---

## Features

- **Import** — OBJ, GLTF/GLB, and binary STL
- **Surface voxelization** — triangle–AABB SAT test, rayon-parallel
- **Block palette** — 80+ blocks matched by perceptual LAB ΔE color distance; filter by concrete, wool, terracotta
- **Transform controls** — scale and Y-offset before voxelizing; re-run freely
- **Optimize** — noise filter (removes floating single voxels), hidden-face flag
- **Export**
  - `.schem` — Sponge Schematic v3, gzip-compressed NBT (WorldEdit / FAWE)
  - `.mca` — Region files, zlib-compressed NBT, Minecraft 1.18+ section format
- **Viewport** — instanced wgpu renderer with depth buffer; orbit / pan / zoom / fit-to-bounds
- **Version targeting** — Java 1.18 · 1.20 · 1.21

---

## Quick start

See **[SETUP.md](SETUP.md)** for full build and install instructions.

```
cargo run --release
```

---

## Workflow

1. **Import** — drag a `.obj / .glb / .stl` onto the window, or click **Import**
2. **Transform** — adjust scale and Y-offset (e.g. "Ground" snaps the bottom to Y=0)
3. **Voxelize** — pick resolution (blocks per unit) and mode (Surface / Solid / Hybrid), then click **⚡ Voxelize**
4. **Palette** — choose a Minecraft version and block filter; swatches update after voxelization
5. **Export** — click **💾 .schem** or **🗺 .mca** and pick a save location

---

## Controls

| Action | Input |
|--------|-------|
| Orbit | Left drag |
| Pan | Right drag / Middle drag |
| Zoom | Scroll wheel |
| Fit to view | Double-click viewport |

---

## Export compatibility

| Format | Tested with |
|--------|-------------|
| `.schem` | WorldEdit 7.x (FAWE), 1.18–1.21 |
| `.mca` | Vanilla Minecraft 1.18+ |

---

## Known limitations

- Solid fill uses a simple X-scanline (may miss concavities in complex meshes — use Surface mode for accuracy)
- No LOD / greedy meshing yet; very large models (500k+ voxels) may be slow in the viewport
- Mesh preview mode not yet implemented (ViewMode::Mesh is a placeholder)
- FBX not supported (convert to OBJ/GLB first with Blender)

---

## License

MIT
