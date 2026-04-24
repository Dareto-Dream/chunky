use anyhow::{anyhow, Context, Result};
use glam::{Vec2, Vec3};
use std::path::Path;

use crate::core::scene::{Material, Mesh, Scene, TextureData};

pub fn load_model(path: &Path) -> Result<Scene> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "obj" => load_obj(path),
        "glb" | "gltf" => load_gltf(path),
        "stl" => load_stl(path),
        _ => Err(anyhow!("Unsupported format: .{}", ext)),
    }
}

fn load_obj(path: &Path) -> Result<Scene> {
    let load_opts = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ..Default::default()
    };

    let (models, materials_result) = tobj::load_obj(path, &load_opts)
        .with_context(|| format!("Failed to load OBJ: {}", path.display()))?;

    let raw_materials = materials_result.unwrap_or_default();
    let base_dir = path.parent().unwrap_or(Path::new("."));

    let mut materials: Vec<Material> = raw_materials
        .iter()
        .map(|m| {
            let diffuse = m.diffuse.unwrap_or([0.8, 0.8, 0.8]);
            let texture = m
                .diffuse_texture
                .as_ref()
                .and_then(|t| load_texture(base_dir.join(t)).ok());
            Material {
                name: m.name.clone(),
                base_color: [diffuse[0], diffuse[1], diffuse[2], 1.0],
                texture,
            }
        })
        .collect();

    if materials.is_empty() {
        materials.push(Material::default());
    }

    let meshes: Vec<Mesh> = models
        .into_iter()
        .map(|model| {
            let m = &model.mesh;
            let vertices: Vec<Vec3> = m
                .positions
                .chunks_exact(3)
                .map(|c| Vec3::new(c[0], c[1], c[2]))
                .collect();

            let normals: Vec<Vec3> = if m.normals.is_empty() {
                vec![Vec3::Y; vertices.len()]
            } else {
                m.normals
                    .chunks_exact(3)
                    .map(|c| Vec3::new(c[0], c[1], c[2]))
                    .collect()
            };

            let uvs: Vec<Vec2> = if m.texcoords.is_empty() {
                vec![Vec2::ZERO; vertices.len()]
            } else {
                m.texcoords
                    .chunks_exact(2)
                    .map(|c| Vec2::new(c[0], c[1]))
                    .collect()
            };

            let material_id = m.material_id;

            Mesh {
                vertices,
                indices: m.indices.clone(),
                normals,
                uvs,
                material_id,
                name: model.name.clone(),
            }
        })
        .collect();

    let scene = Scene {
        meshes,
        materials,
        source_path: Some(path.to_string_lossy().into_owned()),
    };

    normalize_scene(scene)
}

fn load_gltf(path: &Path) -> Result<Scene> {
    let (document, buffers, images) = gltf::import(path)
        .with_context(|| format!("Failed to load GLTF: {}", path.display()))?;

    let mut meshes = Vec::new();
    let mut materials: Vec<Material> = document
        .materials()
        .map(|mat| {
            let pbr = mat.pbr_metallic_roughness();
            let [r, g, b, a] = pbr.base_color_factor();
            let texture = pbr
                .base_color_texture()
                .and_then(|t| {
                    let src = t.texture().source().index();
                    images.get(src).and_then(|img| {
                        let pixels = match img.format {
                            gltf::image::Format::R8G8B8 => {
                                img.pixels.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect()
                            }
                            gltf::image::Format::R8G8B8A8 => img.pixels.clone(),
                            _ => return None,
                        };
                        Some(TextureData { width: img.width, height: img.height, pixels })
                    })
                });
            Material {
                name: mat.name().unwrap_or("material").to_string(),
                base_color: [r, g, b, a],
                texture,
            }
        })
        .collect();

    if materials.is_empty() {
        materials.push(Material::default());
    }

    for node in document.nodes() {
        if let Some(mesh) = node.mesh() {
            let transform = node.transform().matrix();
            let mat4 = glam::Mat4::from_cols_array_2d(&transform);

            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buf| buffers.get(buf.index()).map(|b| b.0.as_ref()));

                let positions: Vec<Vec3> = reader
                    .read_positions()
                    .map(|p| {
                        p.map(|v| {
                            let t = mat4.transform_point3(Vec3::from(v));
                            t
                        })
                        .collect()
                    })
                    .unwrap_or_default();

                if positions.is_empty() {
                    continue;
                }

                let normals: Vec<Vec3> = reader
                    .read_normals()
                    .map(|n| n.map(Vec3::from).collect())
                    .unwrap_or_else(|| vec![Vec3::Y; positions.len()]);

                let uvs: Vec<Vec2> = reader
                    .read_tex_coords(0)
                    .map(|t| t.into_f32().map(Vec2::from).collect())
                    .unwrap_or_else(|| vec![Vec2::ZERO; positions.len()]);

                let indices: Vec<u32> = reader
                    .read_indices()
                    .map(|i| i.into_u32().collect())
                    .unwrap_or_else(|| (0..positions.len() as u32).collect());

                let material_id = primitive.material().index();
                let name = mesh.name().unwrap_or("mesh").to_string();

                meshes.push(Mesh { vertices: positions, indices, normals, uvs, material_id, name });
            }
        }
    }

    let scene = Scene {
        meshes,
        materials,
        source_path: Some(path.to_string_lossy().into_owned()),
    };

    normalize_scene(scene)
}

fn load_stl(path: &Path) -> Result<Scene> {
    use std::io::{BufReader, Read};
    let mut file = std::fs::File::open(path)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;

    // Try binary STL first (skip 80-byte header, then triangle count)
    if data.len() < 84 {
        return Err(anyhow!("STL file too small"));
    }

    let tri_count = u32::from_le_bytes(data[80..84].try_into()?) as usize;
    let expected_size = 84 + tri_count * 50;

    let mut vertices = Vec::with_capacity(tri_count * 3);
    let mut normals = Vec::with_capacity(tri_count * 3);
    let mut indices = Vec::with_capacity(tri_count * 3);

    if data.len() >= expected_size {
        // Binary STL
        for i in 0..tri_count {
            let base = 84 + i * 50;
            let nx = f32::from_le_bytes(data[base..base + 4].try_into()?);
            let ny = f32::from_le_bytes(data[base + 4..base + 8].try_into()?);
            let nz = f32::from_le_bytes(data[base + 8..base + 12].try_into()?);
            let normal = Vec3::new(nx, ny, nz);

            for j in 0..3 {
                let vbase = base + 12 + j * 12;
                let x = f32::from_le_bytes(data[vbase..vbase + 4].try_into()?);
                let y = f32::from_le_bytes(data[vbase + 4..vbase + 8].try_into()?);
                let z = f32::from_le_bytes(data[vbase + 8..vbase + 12].try_into()?);
                indices.push(vertices.len() as u32);
                vertices.push(Vec3::new(x, y, z));
                normals.push(normal);
            }
        }
    } else {
        return Err(anyhow!("Unsupported ASCII STL — convert to binary STL first"));
    }

    let uvs = vec![Vec2::ZERO; vertices.len()];
    let mesh = Mesh { vertices, indices, normals, uvs, material_id: Some(0), name: "stl".into() };
    let scene = Scene {
        meshes: vec![mesh],
        materials: vec![Material::default()],
        source_path: Some(path.to_string_lossy().into_owned()),
    };

    normalize_scene(scene)
}

fn load_texture(path: impl AsRef<Path>) -> Result<TextureData> {
    let img = image::open(path)?.to_rgba8();
    Ok(TextureData {
        width: img.width(),
        height: img.height(),
        pixels: img.into_raw(),
    })
}

pub fn normalize_scene(mut scene: Scene) -> Result<Scene> {
    if let Some((min, max)) = scene.bounds() {
        let center = (min + max) * 0.5;
        let size = max - min;
        let max_dim = size.x.max(size.y).max(size.z);
        if max_dim > 0.0 {
            let scale = 1.0 / max_dim;
            for mesh in &mut scene.meshes {
                for v in &mut mesh.vertices {
                    *v = (*v - center) * scale;
                }
            }
        }
    }
    Ok(scene)
}
