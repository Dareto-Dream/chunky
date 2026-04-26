use glam::IVec3;

use crate::core::voxel::VoxelGrid;

#[derive(Debug, Clone)]
pub struct OptimizeSettings {
    pub remove_hidden: bool,
    pub noise_filter: bool,
    pub noise_threshold: usize, // min neighbors for a voxel to survive
}

impl Default for OptimizeSettings {
    fn default() -> Self {
        Self {
            remove_hidden: true,
            noise_filter: false,
            noise_threshold: 1,
        }
    }
}

const NEIGHBORS_6: [IVec3; 6] = [
    IVec3::new(1, 0, 0),
    IVec3::new(-1, 0, 0),
    IVec3::new(0, 1, 0),
    IVec3::new(0, -1, 0),
    IVec3::new(0, 0, 1),
    IVec3::new(0, 0, -1),
];

pub fn optimize(grid: &mut VoxelGrid, settings: &OptimizeSettings) {
    if settings.noise_filter {
        remove_noise(grid, settings.noise_threshold);
    }
    // Note: hidden block removal is implicit in export — removing them from the
    // grid would change the shape. We handle face culling at the render/export stage.
}

fn remove_noise(grid: &mut VoxelGrid, min_neighbors: usize) {
    let occupied: Vec<IVec3> = grid.iter_occupied().map(|(pos, _)| pos).collect();

    let to_remove: Vec<IVec3> = occupied
        .iter()
        .filter(|&&pos| {
            let count = NEIGHBORS_6
                .iter()
                .filter(|&&d| grid.is_occupied(pos + d))
                .count();
            count < min_neighbors
        })
        .copied()
        .collect();

    for pos in to_remove {
        let (chunk_coord, [lx, ly, lz]) = VoxelGrid::voxel_to_chunk(pos);
        if let Some(chunk) = grid.chunks.get_mut(&chunk_coord) {
            chunk.get_mut(lx, ly, lz).occupied = false;
        }
    }

    // Remove empty chunks
    grid.chunks.retain(|_, chunk| !chunk.is_empty());
}

#[allow(dead_code)]
pub fn is_face_visible(grid: &VoxelGrid, pos: IVec3, direction: IVec3) -> bool {
    !grid.is_occupied(pos + direction)
}
