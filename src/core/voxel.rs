use glam::{IVec3, Vec3};
use std::collections::HashMap;

pub const CHUNK_SIZE: i32 = 32;
pub const CHUNK_SIZE_U: usize = CHUNK_SIZE as usize;
pub const CHUNK_VOL: usize = CHUNK_SIZE_U * CHUNK_SIZE_U * CHUNK_SIZE_U;

#[derive(Debug, Clone, Copy, Default)]
pub struct Voxel {
    pub occupied: bool,
    pub color: [u8; 3],
    pub block_id: u16, // 0 = unmapped / air
}

impl Voxel {
    pub fn new(color: [u8; 3]) -> Self {
        Self {
            occupied: true,
            color,
            block_id: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VoxelChunk {
    pub voxels: Box<[Voxel; CHUNK_VOL]>,
}

impl VoxelChunk {
    pub fn new() -> Self {
        Self {
            voxels: Box::new([Voxel::default(); CHUNK_VOL]),
        }
    }

    pub fn idx(lx: usize, ly: usize, lz: usize) -> usize {
        lx + ly * CHUNK_SIZE_U + lz * CHUNK_SIZE_U * CHUNK_SIZE_U
    }

    pub fn get(&self, lx: usize, ly: usize, lz: usize) -> &Voxel {
        &self.voxels[Self::idx(lx, ly, lz)]
    }

    pub fn get_mut(&mut self, lx: usize, ly: usize, lz: usize) -> &mut Voxel {
        &mut self.voxels[Self::idx(lx, ly, lz)]
    }

    pub fn is_empty(&self) -> bool {
        self.voxels.iter().all(|v| !v.occupied)
    }

    pub fn occupied_count(&self) -> usize {
        self.voxels.iter().filter(|v| v.occupied).count()
    }
}

impl Default for VoxelChunk {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Default, Clone)]
pub struct VoxelGrid {
    pub chunks: HashMap<IVec3, VoxelChunk>,
    pub resolution: f32, // voxels per unit
    pub origin: Vec3,    // world origin offset
}

impl VoxelGrid {
    pub fn new(resolution: f32, origin: Vec3) -> Self {
        Self {
            chunks: HashMap::new(),
            resolution,
            origin,
        }
    }

    #[allow(dead_code)]
    pub fn world_to_voxel(&self, world: Vec3) -> IVec3 {
        let local = (world - self.origin) * self.resolution;
        IVec3::new(
            local.x.floor() as i32,
            local.y.floor() as i32,
            local.z.floor() as i32,
        )
    }

    #[allow(dead_code)]
    pub fn voxel_to_world(&self, voxel: IVec3) -> Vec3 {
        self.origin + Vec3::new(voxel.x as f32, voxel.y as f32, voxel.z as f32) / self.resolution
    }

    pub fn voxel_to_chunk(voxel: IVec3) -> (IVec3, [usize; 3]) {
        let cx = voxel.x.div_euclid(CHUNK_SIZE);
        let cy = voxel.y.div_euclid(CHUNK_SIZE);
        let cz = voxel.z.div_euclid(CHUNK_SIZE);
        let lx = voxel.x.rem_euclid(CHUNK_SIZE) as usize;
        let ly = voxel.y.rem_euclid(CHUNK_SIZE) as usize;
        let lz = voxel.z.rem_euclid(CHUNK_SIZE) as usize;
        (IVec3::new(cx, cy, cz), [lx, ly, lz])
    }

    pub fn set_voxel(&mut self, voxel: IVec3, v: Voxel) {
        let (chunk_coord, [lx, ly, lz]) = Self::voxel_to_chunk(voxel);
        self.chunks
            .entry(chunk_coord)
            .or_insert_with(VoxelChunk::new)
            .get_mut(lx, ly, lz)
            .clone_from(&v);
    }

    pub fn get_voxel(&self, voxel: IVec3) -> Option<&Voxel> {
        let (chunk_coord, [lx, ly, lz]) = Self::voxel_to_chunk(voxel);
        self.chunks.get(&chunk_coord).map(|c| c.get(lx, ly, lz))
    }

    pub fn is_occupied(&self, voxel: IVec3) -> bool {
        self.get_voxel(voxel).map(|v| v.occupied).unwrap_or(false)
    }

    pub fn total_voxels(&self) -> usize {
        self.chunks.values().map(|c| c.occupied_count()).sum()
    }

    pub fn bounds_voxel(&self) -> Option<(IVec3, IVec3)> {
        if self.chunks.is_empty() {
            return None;
        }
        let mut min = IVec3::splat(i32::MAX);
        let mut max = IVec3::splat(i32::MIN);
        for (&coord, chunk) in &self.chunks {
            for lz in 0..CHUNK_SIZE_U {
                for ly in 0..CHUNK_SIZE_U {
                    for lx in 0..CHUNK_SIZE_U {
                        if chunk.get(lx, ly, lz).occupied {
                            let gx = coord.x * CHUNK_SIZE + lx as i32;
                            let gy = coord.y * CHUNK_SIZE + ly as i32;
                            let gz = coord.z * CHUNK_SIZE + lz as i32;
                            let g = IVec3::new(gx, gy, gz);
                            min = min.min(g);
                            max = max.max(g);
                        }
                    }
                }
            }
        }
        Some((min, max))
    }

    pub fn iter_occupied(&self) -> impl Iterator<Item = (IVec3, &Voxel)> {
        self.chunks.iter().flat_map(|(&coord, chunk)| {
            (0..CHUNK_VOL).filter_map(move |idx| {
                let v = &chunk.voxels[idx];
                if !v.occupied {
                    return None;
                }
                let lx = idx % CHUNK_SIZE_U;
                let ly = (idx / CHUNK_SIZE_U) % CHUNK_SIZE_U;
                let lz = idx / (CHUNK_SIZE_U * CHUNK_SIZE_U);
                let gx = coord.x * CHUNK_SIZE + lx as i32;
                let gy = coord.y * CHUNK_SIZE + ly as i32;
                let gz = coord.z * CHUNK_SIZE + lz as i32;
                Some((IVec3::new(gx, gy, gz), v))
            })
        })
    }
}
