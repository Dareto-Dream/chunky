use glam::{IVec3, Vec2, Vec3};
use rayon::prelude::*;
use std::sync::{Arc, Mutex};

use crate::core::scene::{Material, Scene};
use crate::core::voxel::{Voxel, VoxelGrid};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VoxelizeMode {
    Surface,
    Solid,
    Hybrid,
}

#[derive(Debug, Clone)]
pub struct VoxelizeSettings {
    pub mode: VoxelizeMode,
    pub resolution: f32,       // blocks per unit
    pub shell_thickness: u32,  // for surface mode
    pub infill_density: f32,   // 0..1 for solid/hybrid
}

impl Default for VoxelizeSettings {
    fn default() -> Self {
        Self {
            mode: VoxelizeMode::Surface,
            resolution: 64.0,
            shell_thickness: 1,
            infill_density: 1.0,
        }
    }
}

pub fn voxelize(scene: &Scene, settings: &VoxelizeSettings) -> VoxelGrid {
    let bounds = match scene.bounds() {
        Some(b) => b,
        None => return VoxelGrid::new(settings.resolution, Vec3::ZERO),
    };

    let (min, _max) = bounds;
    let origin = min;
    let mut grid = VoxelGrid::new(settings.resolution, origin);

    match settings.mode {
        VoxelizeMode::Surface | VoxelizeMode::Hybrid => {
            voxelize_surface(scene, &mut grid, settings);
        }
        VoxelizeMode::Solid => {
            voxelize_surface(scene, &mut grid, settings);
            fill_solid(&mut grid);
        }
    }

    grid
}

fn voxelize_surface(scene: &Scene, grid: &mut VoxelGrid, settings: &VoxelizeSettings) {
    let shared_grid = Arc::new(Mutex::new(std::mem::take(&mut grid.chunks)));
    let resolution = settings.resolution;
    let origin = grid.origin;

    let triangles: Vec<_> = scene
        .meshes
        .iter()
        .enumerate()
        .flat_map(|(mesh_idx, mesh)| {
            (0..mesh.triangle_count()).map(move |tri| (mesh_idx, tri))
        })
        .collect();

    let results: Vec<Vec<(IVec3, Voxel)>> = triangles
        .par_iter()
        .map(|&(mesh_idx, tri)| {
            let mesh = &scene.meshes[mesh_idx];
            let material = mesh.material_id.and_then(|id| scene.materials.get(id));
            rasterize_triangle(mesh, tri, material, resolution, origin)
        })
        .collect();

    let mut chunks = shared_grid.lock().unwrap();
    for voxel_list in results {
        for (voxel_coord, voxel) in voxel_list {
            let (chunk_coord, [lx, ly, lz]) = VoxelGrid::voxel_to_chunk(voxel_coord);
            let chunk = chunks
                .entry(chunk_coord)
                .or_insert_with(crate::core::voxel::VoxelChunk::new);
            let cell = chunk.get_mut(lx, ly, lz);
            if !cell.occupied {
                *cell = voxel;
            }
        }
    }
    drop(chunks);
    grid.chunks = Arc::try_unwrap(shared_grid).unwrap().into_inner().unwrap();
}

fn rasterize_triangle(
    mesh: &crate::core::scene::Mesh,
    tri: usize,
    material: Option<&Material>,
    resolution: f32,
    origin: Vec3,
) -> Vec<(IVec3, Voxel)> {
    let [a, b, c] = mesh.triangle(tri);
    let uv = mesh.triangle_uv(tri);

    // Transform to voxel space
    let av = world_to_voxel_f32(a, origin, resolution);
    let bv = world_to_voxel_f32(b, origin, resolution);
    let cv = world_to_voxel_f32(c, origin, resolution);

    // AABB in voxel space
    let v_min = av.min(bv).min(cv).floor().as_ivec3() - IVec3::ONE;
    let v_max = av.max(bv).max(cv).ceil().as_ivec3() + IVec3::ONE;

    let mut result = Vec::new();

    for gz in v_min.z..=v_max.z {
        for gy in v_min.y..=v_max.y {
            for gx in v_min.x..=v_max.x {
                let voxel_coord = IVec3::new(gx, gy, gz);
                let voxel_min = Vec3::new(gx as f32, gy as f32, gz as f32);
                let voxel_max = voxel_min + Vec3::ONE;

                if triangle_aabb_intersect(av, bv, cv, voxel_min, voxel_max) {
                    let color = sample_color(av, bv, cv, uv, voxel_min + 0.5, material);
                    result.push((voxel_coord, Voxel::new(color)));
                }
            }
        }
    }

    result
}

fn world_to_voxel_f32(world: Vec3, origin: Vec3, resolution: f32) -> Vec3 {
    (world - origin) * resolution
}

fn sample_color(
    av: Vec3, bv: Vec3, cv: Vec3,
    uvs: Option<[Vec2; 3]>,
    point: Vec3,
    material: Option<&Material>,
) -> [u8; 3] {
    let Some(mat) = material else {
        return [204, 204, 204];
    };

    if let (Some(uvs), Some(texture)) = (uvs, &mat.texture) {
        // Barycentric interpolation for UV
        if let Some(bary) = barycentric(av, bv, cv, point) {
            let uv = uvs[0] * bary.x + uvs[1] * bary.y + uvs[2] * bary.z;
            let [r, g, b, _] = texture.sample(uv);
            return [r, g, b];
        }
    }

    let [r, g, b, _] = mat.base_color;
    [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8]
}

fn barycentric(a: Vec3, b: Vec3, c: Vec3, p: Vec3) -> Option<Vec3> {
    let v0 = b - a;
    let v1 = c - a;
    let v2 = p - a;

    let d00 = v0.dot(v0);
    let d01 = v0.dot(v1);
    let d11 = v1.dot(v1);
    let d20 = v2.dot(v0);
    let d21 = v2.dot(v1);

    let denom = d00 * d11 - d01 * d01;
    if denom.abs() < 1e-8 {
        return None;
    }

    let v = (d11 * d20 - d01 * d21) / denom;
    let w = (d00 * d21 - d01 * d20) / denom;
    let u = 1.0 - v - w;
    Some(Vec3::new(u, v, w))
}

// Separating Axis Theorem for triangle-AABB intersection
fn triangle_aabb_intersect(a: Vec3, b: Vec3, c: Vec3, aabb_min: Vec3, aabb_max: Vec3) -> bool {
    let center = (aabb_min + aabb_max) * 0.5;
    let half = (aabb_max - aabb_min) * 0.5;

    let a = a - center;
    let b = b - center;
    let c = c - center;

    let ab = b - a;
    let bc = c - b;
    let ca = a - c;

    // 9 cross-product axes
    let axes_cross = [
        Vec3::X.cross(ab), Vec3::X.cross(bc), Vec3::X.cross(ca),
        Vec3::Y.cross(ab), Vec3::Y.cross(bc), Vec3::Y.cross(ca),
        Vec3::Z.cross(ab), Vec3::Z.cross(bc), Vec3::Z.cross(ca),
    ];

    for axis in &axes_cross {
        if axis.length_squared() < 1e-10 {
            continue;
        }
        if !overlap_on_axis(*axis, a, b, c, half) {
            return false;
        }
    }

    // 3 face normals of AABB
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        if !overlap_on_axis(axis, a, b, c, half) {
            return false;
        }
    }

    // 1 triangle normal
    let tri_normal = ab.cross(bc);
    if tri_normal.length_squared() > 1e-10 {
        let r = half.x * tri_normal.x.abs() + half.y * tri_normal.y.abs() + half.z * tri_normal.z.abs();
        let s = tri_normal.dot(a);
        if s.abs() > r {
            return false;
        }
    }

    true
}

fn overlap_on_axis(axis: Vec3, a: Vec3, b: Vec3, c: Vec3, half: Vec3) -> bool {
    let pa = axis.dot(a);
    let pb = axis.dot(b);
    let pc = axis.dot(c);
    let tri_min = pa.min(pb).min(pc);
    let tri_max = pa.max(pb).max(pc);
    let r = half.x * axis.x.abs() + half.y * axis.y.abs() + half.z * axis.z.abs();
    !(tri_min > r || tri_max < -r)
}

fn fill_solid(grid: &mut VoxelGrid) {
    let Some((min, max)) = grid.bounds_voxel() else { return };

    // Scanline fill: for each Y slice, find min/max X for each Z row
    for gy in min.y..=max.y {
        for gz in min.z..=max.z {
            let mut inside = false;
            let mut last_surface = false;
            for gx in min.x..=max.x {
                let occupied = grid.is_occupied(IVec3::new(gx, gy, gz));
                if occupied && !last_surface {
                    inside = !inside;
                }
                if inside && !occupied {
                    let color = find_nearby_color(grid, IVec3::new(gx, gy, gz));
                    grid.set_voxel(IVec3::new(gx, gy, gz), Voxel::new(color));
                }
                last_surface = occupied;
            }
        }
    }
}

fn find_nearby_color(grid: &VoxelGrid, pos: IVec3) -> [u8; 3] {
    for r in 1..=5i32 {
        for dz in -r..=r {
            for dy in -r..=r {
                for dx in -r..=r {
                    if let Some(v) = grid.get_voxel(pos + IVec3::new(dx, dy, dz)) {
                        if v.occupied {
                            return v.color;
                        }
                    }
                }
            }
        }
    }
    [128, 128, 128]
}
