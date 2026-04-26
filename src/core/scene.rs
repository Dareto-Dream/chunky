use glam::{Vec2, Vec3};

#[derive(Debug, Clone, Default)]
pub struct Scene {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
    pub source_path: Option<String>,
}

impl Scene {
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        let mut any = false;
        for mesh in &self.meshes {
            for &v in &mesh.vertices {
                min = min.min(v);
                max = max.max(v);
                any = true;
            }
        }
        any.then_some((min, max))
    }

    #[allow(dead_code)]
    pub fn center(&self) -> Vec3 {
        self.bounds()
            .map(|(min, max)| (min + max) * 0.5)
            .unwrap_or(Vec3::ZERO)
    }

    pub fn size(&self) -> Vec3 {
        self.bounds()
            .map(|(min, max)| max - min)
            .unwrap_or(Vec3::ZERO)
    }

    pub fn total_triangles(&self) -> usize {
        self.meshes.iter().map(|m| m.indices.len() / 3).sum()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub vertices: Vec<Vec3>,
    pub indices: Vec<u32>,
    #[allow(dead_code)]
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    pub material_id: Option<usize>,
    #[allow(dead_code)]
    pub name: String,
}

impl Mesh {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn triangle(&self, tri: usize) -> [Vec3; 3] {
        let i0 = self.indices[tri * 3] as usize;
        let i1 = self.indices[tri * 3 + 1] as usize;
        let i2 = self.indices[tri * 3 + 2] as usize;
        [self.vertices[i0], self.vertices[i1], self.vertices[i2]]
    }

    pub fn triangle_uv(&self, tri: usize) -> Option<[Vec2; 3]> {
        if self.uvs.is_empty() {
            return None;
        }
        let i0 = self.indices[tri * 3] as usize;
        let i1 = self.indices[tri * 3 + 1] as usize;
        let i2 = self.indices[tri * 3 + 2] as usize;
        Some([self.uvs[i0], self.uvs[i1], self.uvs[i2]])
    }

    #[allow(dead_code)]
    pub fn triangle_normal(&self, tri: usize) -> Vec3 {
        let [a, b, c] = self.triangle(tri);
        (b - a).cross(c - a).normalize_or_zero()
    }
}

#[derive(Debug, Clone)]
pub struct Material {
    #[allow(dead_code)]
    pub name: String,
    pub base_color: [f32; 4],
    pub texture: Option<TextureData>,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            base_color: [0.8, 0.8, 0.8, 1.0],
            texture: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TextureData {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>, // RGBA8
}

impl TextureData {
    pub fn sample(&self, uv: Vec2) -> [u8; 4] {
        let u = uv.x.rem_euclid(1.0);
        let v = 1.0 - uv.y.rem_euclid(1.0);
        let x = (u * self.width as f32) as u32 % self.width;
        let y = (v * self.height as f32) as u32 % self.height;
        let idx = ((y * self.width + x) * 4) as usize;
        if idx + 3 < self.pixels.len() {
            [
                self.pixels[idx],
                self.pixels[idx + 1],
                self.pixels[idx + 2],
                self.pixels[idx + 3],
            ]
        } else {
            [204, 204, 204, 255]
        }
    }
}
