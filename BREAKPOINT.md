# BREAKPOINT — Session 1

## What was built

Full MVP skeleton for **Chunky** — a native Rust desktop application converting 3D models to Minecraft structures.

### Files created (zero compile errors)

| File | Purpose |
|------|---------|
| `Cargo.toml` | Dependencies: eframe 0.28, wgpu 0.20, glam, tobj, gltf, rayon, flate2, byteorder, rfd |
| `src/main.rs` | eframe entry point with wgpu renderer |
| `src/app.rs` | Central app state, worker-thread dispatch, file I/O coordination |
| `src/ui/mod.rs` | Full egui UI: toolbar, import/transform/voxelize/palette/export panels, viewport |
| `src/core/scene.rs` | Scene, Mesh, Material, TextureData structs |
| `src/core/voxel.rs` | VoxelGrid (chunked HashMap), VoxelChunk (32³), Voxel |
| `src/core/import.rs` | OBJ (tobj), GLTF (gltf crate), binary STL loader; auto-normalization |
| `src/core/voxelize.rs` | SAT triangle–AABB surface voxelization, rayon-parallel per triangle |
| `src/core/palette.rs` | 80+ block database with LAB ΔE color matching; wool/concrete/terracotta filters |
| `src/core/optimize.rs` | Noise filter (neighbor count), hidden-face flag |
| `src/core/export.rs` | Hand-rolled NBT writer + Sponge .schem v3 + .mca region file generator |
| `src/renderer/camera.rs` | Orbit camera (yaw/pitch/distance, fit-to-bounds) |
| `src/renderer/voxel_renderer.rs` | wgpu instanced cube renderer; 2M instance cap |
| `shaders/voxel.wgsl` | Lambertian + ambient WGSL shader |

### What works
- `cargo check` passes (0 errors)
- Import pipeline: OBJ, GLTF, binary STL → normalized Scene
- Surface voxelization: triangle–AABB SAT, parallel (rayon), color sampling from UV/material
- Block mapping: LAB color space → nearest block by ΔE
- .schem export: Sponge schematic v3, gzip-compressed NBT
- .mca export: region files, per-chunk zlib-compressed NBT, 1.18+ section format
- UI: full panel layout, drag-to-orbit camera, progress bar, file dialogs, palette swatches
- Palette: 80+ blocks across stone/wood/wool/concrete/terracotta/metals/special

---

## Known gaps (next session)

### Critical for usability
1. **Depth buffer** — egui's main render pass has no depth attachment, so voxels overdraw incorrectly at oblique angles. Fix: in `prepare()`, render to a separate color+depth texture; in `paint()`, draw that texture as a fullscreen quad.

2. **Transform not wired** — `transform_scale` and `transform_offset` UI values are not applied before voxelization. Need to apply them to the scene before calling `voxelize()`.

3. **Solid fill voxelization** — current X-axis scanline misses concavities. Replace with flood fill from exterior or proper voxel ray casting.

### Performance
4. **Greedy meshing** — currently each voxel = 1 draw instance (6 faces). Need to merge coplanar same-color faces into quads to reduce draw calls. Essential for 100k+ voxels.

5. **LOD system** — for distant voxel clusters, collapse multiple voxels into larger quads.

### Missing features
6. **Mesh preview mode** — `ViewMode::Mesh` renders nothing; needs a separate mesh pipeline.

7. **.nbt structure block export** — similar to .schem but different NBT layout.

8. **Hybrid voxelization mode** — surface + solid with infill density control.

9. **Chunk boundary overlay** — visual grid showing 16×16 MC chunk boundaries.

10. **FBX support** — via external converter (assimp or FBX SDK wrapper).

### Testing needed
11. **NBT validation** — verify generated .schem loads in WorldEdit and .mca loads in Minecraft 1.21.

12. **Large model performance** — test with 500k+ triangle models to validate streaming.

---

## Next session plan

1. Fix depth rendering (render-to-texture approach)
2. Wire transform controls into voxelization
3. Implement greedy meshing for viewport
4. Add chunk boundary overlay
5. NBT integration test
6. Mesh preview pipeline
