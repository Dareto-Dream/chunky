use anyhow::{Context, Result};
use byteorder::{BigEndian, WriteBytesExt};
use flate2::{write::GzEncoder, write::ZlibEncoder, Compression};
use glam::IVec3;
use std::collections::HashMap;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

use crate::core::palette::{BlockDatabase, MinecraftVersion};
use crate::core::voxel::VoxelGrid;

pub struct ExportSettings {
    pub version: MinecraftVersion,
    pub offset: IVec3, // placement offset for .schem / .nbt
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            version: MinecraftVersion::Java121,
            offset: IVec3::ZERO,
        }
    }
}

// ─── .schem (WorldEdit Sponge Schematic v3) ─────────────────────────────────

pub fn export_schem(
    grid: &VoxelGrid,
    db: &BlockDatabase,
    settings: &ExportSettings,
    path: &Path,
) -> Result<()> {
    let Some((min, max)) = grid.bounds_voxel() else {
        return Err(anyhow::anyhow!("Empty voxel grid"));
    };

    let size = max - min + IVec3::ONE;
    let w = size.x as usize;
    let h = size.y as usize;
    let d = size.z as usize;

    // Build palette and block index array
    let mut palette_map: HashMap<String, u32> = HashMap::new();
    let mut palette_vec: Vec<String> = Vec::new();
    let mut blocks: Vec<u32> = vec![0; w * h * d];

    palette_map.insert("minecraft:air".into(), 0);
    palette_vec.push("minecraft:air".into());

    for (pos, voxel) in grid.iter_occupied() {
        if !voxel.occupied {
            continue;
        }
        let name = db.mc_name(voxel.block_id).to_string();
        let idx = if let Some(&i) = palette_map.get(&name) {
            i
        } else {
            let i = palette_vec.len() as u32;
            palette_map.insert(name.clone(), i);
            palette_vec.push(name);
            i
        };

        let lx = (pos.x - min.x) as usize;
        let ly = (pos.y - min.y) as usize;
        let lz = (pos.z - min.z) as usize;
        // Sponge v3 order: Y*Width*Length + Z*Width + X
        blocks[ly * w * d + lz * w + lx] = idx;
    }

    // Encode indices as varints (Sponge schematic format)
    let data_bytes = pack_varint_array(&blocks);

    let mut nbt_bytes = Vec::new();
    {
        let mut nbt = NbtWriter::new(&mut nbt_bytes);
        nbt.compound("Schematic", |c| {
            c.int("Version", 3);
            c.int("DataVersion", settings.version.data_version());
            c.compound("Metadata", |m| {
                m.string("Name", "Chunky Export");
            });
            // Dimensions at root level as TAG_Short (Sponge v3 spec)
            c.short("Width", size.x as i16);
            c.short("Height", size.y as i16);
            c.short("Length", size.z as i16);
            c.int_array(
                "Offset",
                &[settings.offset.x, settings.offset.y, settings.offset.z],
            );
            c.compound("Blocks", |b| {
                b.int("PaletteMax", palette_vec.len() as i32);
                b.compound("Palette", |p| {
                    // Sort by index for deterministic output
                    let mut entries: Vec<(&String, &u32)> = palette_map.iter().collect();
                    entries.sort_by_key(|(_, &idx)| idx);
                    for (name, &idx) in entries {
                        p.int(name, idx as i32);
                    }
                });
                b.byte_array("Data", &data_bytes);
                b.list_compounds("BlockEntities", |_| {});
            });
        });
    }

    let file =
        std::fs::File::create(path).with_context(|| format!("Cannot create {}", path.display()))?;
    let mut encoder = GzEncoder::new(file, Compression::default());
    encoder.write_all(&nbt_bytes)?;
    encoder.finish()?;
    Ok(())
}

// ─── .mca Region Files ────────────────────────────────────────────────────────

const REGION_SIDE: i32 = 32; // chunks per region side

pub fn export_region(
    grid: &VoxelGrid,
    db: &BlockDatabase,
    settings: &ExportSettings,
    out_dir: &Path,
) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;

    let Some((min, max)) = grid.bounds_voxel() else {
        return Err(anyhow::anyhow!("Empty voxel grid"));
    };

    let world_min = min + settings.offset;
    let world_max = max + settings.offset;
    let _ = (world_min, world_max); // referenced for bounds check below

    // Collect blocks per Minecraft chunk (16×16 columns)
    let mut chunk_blocks: HashMap<(i32, i32), Vec<(IVec3, u16)>> = HashMap::new();

    for (pos, voxel) in grid.iter_occupied() {
        if !voxel.occupied || voxel.block_id == 0 {
            continue;
        }
        let wx = pos.x + settings.offset.x;
        let wy = pos.y + settings.offset.y;
        let wz = pos.z + settings.offset.z;

        let min_y = settings.version.min_y();
        let max_y = settings.version.max_y();
        if wy < min_y || wy >= max_y {
            continue;
        }

        let cx = wx >> 4;
        let cz = wz >> 4;
        chunk_blocks
            .entry((cx, cz))
            .or_default()
            .push((IVec3::new(wx, wy, wz), voxel.block_id));
    }

    // Group by region file
    let mut regions: HashMap<(i32, i32), HashMap<(i32, i32), Vec<(IVec3, u16)>>> = HashMap::new();
    for ((cx, cz), blocks) in chunk_blocks {
        let rx = cx.div_euclid(REGION_SIDE);
        let rz = cz.div_euclid(REGION_SIDE);
        regions
            .entry((rx, rz))
            .or_default()
            .insert((cx, cz), blocks);
    }

    for ((rx, rz), chunks) in &regions {
        let filename = format!("r.{}.{}.mca", rx, rz);
        let filepath = out_dir.join(&filename);
        write_region_file(&filepath, chunks, db, settings)?;
    }

    Ok(())
}

fn write_region_file(
    path: &Path,
    chunks: &HashMap<(i32, i32), Vec<(IVec3, u16)>>,
    db: &BlockDatabase,
    settings: &ExportSettings,
) -> Result<()> {
    let mut file = std::fs::File::create(path)?;

    // Placeholder 8 KB header (locations + timestamps)
    let header = vec![0u8; 8192];
    file.write_all(&header)?;

    let mut locations: [u32; 1024] = [0; 1024];
    let mut sector_offset: u32 = 2; // first two sectors are the header

    for (&(cx, cz), block_list) in chunks {
        let chunk_nbt = build_chunk_nbt(cx, cz, block_list, db, settings);

        let mut compressed = Vec::new();
        {
            let mut encoder = ZlibEncoder::new(&mut compressed, Compression::default());
            encoder.write_all(&chunk_nbt)?;
            encoder.finish()?;
        }

        // Region chunk format: 4-byte BE length (includes 1-byte type) + 1-byte type + data
        let total_len = (compressed.len() + 1) as u32;
        let mut chunk_data = Vec::new();
        chunk_data.write_u32::<BigEndian>(total_len)?;
        chunk_data.write_u8(2)?; // compression type 2 = zlib
        chunk_data.write_all(&compressed)?;

        // Pad to 4 KB sector boundary
        let sectors_needed = (chunk_data.len() + 4095) / 4096;
        chunk_data.resize(sectors_needed * 4096, 0);

        file.write_all(&chunk_data)?;

        let lx = cx.rem_euclid(REGION_SIDE) as usize;
        let lz = cz.rem_euclid(REGION_SIDE) as usize;
        let loc_idx = lz * 32 + lx;
        locations[loc_idx] = (sector_offset << 8) | (sectors_needed as u32 & 0xFF);
        sector_offset += sectors_needed as u32;
    }

    // Rewrite header with actual chunk locations
    file.seek(SeekFrom::Start(0))?;
    for &loc in &locations {
        file.write_u32::<BigEndian>(loc)?;
    }

    Ok(())
}

fn build_chunk_nbt(
    cx: i32,
    cz: i32,
    blocks: &[(IVec3, u16)],
    db: &BlockDatabase,
    settings: &ExportSettings,
) -> Vec<u8> {
    let min_y = settings.version.min_y();
    let data_version = settings.version.data_version();

    // Group blocks by 16-block section using absolute section Y (not relative)
    let mut section_blocks: HashMap<i8, Vec<(i32, i32, i32, u16)>> = HashMap::new();
    for &(world_pos, block_id) in blocks {
        // Section Y = absolute world Y / 16 (arithmetic shift preserves sign)
        let section_y = (world_pos.y >> 4) as i8;
        let local_x = world_pos.x & 0xF;
        let local_y = (world_pos.y & 0xF) as i32;
        let local_z = world_pos.z & 0xF;
        section_blocks
            .entry(section_y)
            .or_default()
            .push((local_x, local_y, local_z, block_id));
    }

    let mut nbt = Vec::new();
    let mut w = NbtWriter::new(&mut nbt);

    w.compound("", |root| {
        root.int("DataVersion", data_version);
        root.int("xPos", cx);
        root.int("yPos", min_y >> 4); // lowest section Y for this version
        root.int("zPos", cz);
        root.string("Status", "minecraft:full");
        root.long("LastUpdate", 0);
        root.long("InhabitedTime", 0);

        root.list_compounds("sections", |sections_list| {
            let mut sy_sorted: Vec<i8> = section_blocks.keys().copied().collect();
            sy_sorted.sort();

            for sy in &sy_sorted {
                sections_list.push_compound(|section| {
                    section.byte("Y", *sy);

                    let block_list = &section_blocks[sy];
                    let (palette_vec, state_array) = encode_section(block_list, db);

                    section.compound("block_states", |bs| {
                        bs.list_compounds("palette", |pal_list| {
                            for name in &palette_vec {
                                pal_list.push_compound(|entry| {
                                    entry.string("Name", name);
                                });
                            }
                        });
                        if palette_vec.len() > 1 {
                            bs.long_array("data", &state_array);
                        }
                    });

                    section.compound("biomes", |bio| {
                        bio.list_strings("palette", &["minecraft:plains"]);
                    });

                    section.byte_array("SkyLight", &[]);
                    section.byte_array("BlockLight", &[]);
                });
            }
        });

        root.compound("Heightmaps", |_| {});
        root.list_compounds("block_entities", |_| {});
    });

    nbt
}

fn encode_section(blocks: &[(i32, i32, i32, u16)], db: &BlockDatabase) -> (Vec<String>, Vec<i64>) {
    let mut palette_map: HashMap<String, usize> = HashMap::new();
    let mut palette_vec: Vec<String> = Vec::new();

    palette_map.insert("minecraft:air".into(), 0);
    palette_vec.push("minecraft:air".into());

    let mut state_indices = vec![0usize; 4096];

    for &(lx, ly, lz, block_id) in blocks {
        let name = db.mc_name(block_id).to_string();
        let idx = if let Some(&i) = palette_map.get(&name) {
            i
        } else {
            let i = palette_vec.len();
            palette_map.insert(name.clone(), i);
            palette_vec.push(name);
            i
        };
        // Section storage: Y*256 + Z*16 + X
        let arr_idx = (ly * 256 + lz * 16 + lx) as usize;
        if arr_idx < 4096 {
            state_indices[arr_idx] = idx;
        }
    }

    // Minecraft 1.16+ packing: no value spans two longs
    let bits = (usize::BITS - (palette_vec.len() - 1).leading_zeros()).max(4) as usize;
    let values_per_long = 64 / bits;
    let long_count = (4096 + values_per_long - 1) / values_per_long;

    let mut longs = vec![0i64; long_count];
    for (i, &idx) in state_indices.iter().enumerate() {
        let long_idx = i / values_per_long;
        let bit_offset = (i % values_per_long) * bits;
        longs[long_idx] |= (idx as i64) << bit_offset;
    }

    (palette_vec, longs)
}

// ─── NBT Writer ───────────────────────────────────────────────────────────────

pub struct NbtWriter<'a> {
    out: &'a mut Vec<u8>,
}

impl<'a> NbtWriter<'a> {
    pub fn new(out: &'a mut Vec<u8>) -> Self {
        Self { out }
    }

    fn write_tag_header(&mut self, tag_type: u8, name: &str) {
        self.out.push(tag_type);
        let bytes = name.as_bytes();
        self.out.write_u16::<BigEndian>(bytes.len() as u16).unwrap();
        self.out.extend_from_slice(bytes);
    }

    pub fn compound(&mut self, name: &str, f: impl FnOnce(&mut CompoundWriter)) {
        self.write_tag_header(10, name);
        let mut cw = CompoundWriter { out: self.out };
        f(&mut cw);
        self.out.push(0); // TAG_End
    }
}

pub struct CompoundWriter<'a> {
    pub out: &'a mut Vec<u8>,
}

impl<'a> CompoundWriter<'a> {
    fn write_tag_header(&mut self, tag_type: u8, name: &str) {
        self.out.push(tag_type);
        let bytes = name.as_bytes();
        self.out.write_u16::<BigEndian>(bytes.len() as u16).unwrap();
        self.out.extend_from_slice(bytes);
    }

    pub fn byte(&mut self, name: &str, val: i8) {
        self.write_tag_header(1, name);
        self.out.push(val as u8);
    }

    pub fn short(&mut self, name: &str, val: i16) {
        self.write_tag_header(2, name);
        self.out.write_i16::<BigEndian>(val).unwrap();
    }

    pub fn int(&mut self, name: &str, val: i32) {
        self.write_tag_header(3, name);
        self.out.write_i32::<BigEndian>(val).unwrap();
    }

    pub fn long(&mut self, name: &str, val: i64) {
        self.write_tag_header(4, name);
        self.out.write_i64::<BigEndian>(val).unwrap();
    }

    pub fn string(&mut self, name: &str, val: &str) {
        self.write_tag_header(8, name);
        let bytes = val.as_bytes();
        self.out.write_u16::<BigEndian>(bytes.len() as u16).unwrap();
        self.out.extend_from_slice(bytes);
    }

    pub fn byte_array(&mut self, name: &str, data: &[u8]) {
        self.write_tag_header(7, name);
        self.out.write_i32::<BigEndian>(data.len() as i32).unwrap();
        self.out.extend_from_slice(data);
    }

    pub fn int_array(&mut self, name: &str, data: &[i32]) {
        self.write_tag_header(11, name);
        self.out.write_i32::<BigEndian>(data.len() as i32).unwrap();
        for &v in data {
            self.out.write_i32::<BigEndian>(v).unwrap();
        }
    }

    pub fn long_array(&mut self, name: &str, data: &[i64]) {
        self.write_tag_header(12, name);
        self.out.write_i32::<BigEndian>(data.len() as i32).unwrap();
        for &v in data {
            self.out.write_i64::<BigEndian>(v).unwrap();
        }
    }

    pub fn compound(&mut self, name: &str, f: impl FnOnce(&mut CompoundWriter)) {
        self.write_tag_header(10, name);
        let mut cw = CompoundWriter { out: self.out };
        f(&mut cw);
        self.out.push(0); // TAG_End
    }

    pub fn list_compounds(&mut self, name: &str, f: impl FnOnce(&mut Vec<Vec<u8>>)) {
        self.write_tag_header(9, name);
        // Collect first so we know count and whether list is empty
        let mut items: Vec<Vec<u8>> = Vec::new();
        f(&mut items);
        // Empty TAG_List must use element type 0 per NBT spec
        let elem_type: u8 = if items.is_empty() { 0 } else { 10 };
        self.out.push(elem_type);
        self.out.write_i32::<BigEndian>(items.len() as i32).unwrap();
        for item in items {
            self.out.extend_from_slice(&item);
        }
    }

    pub fn list_strings(&mut self, name: &str, vals: &[&str]) {
        self.write_tag_header(9, name);
        let elem_type: u8 = if vals.is_empty() { 0 } else { 8 };
        self.out.push(elem_type);
        self.out.write_i32::<BigEndian>(vals.len() as i32).unwrap();
        for &s in vals {
            let bytes = s.as_bytes();
            self.out.write_u16::<BigEndian>(bytes.len() as u16).unwrap();
            self.out.extend_from_slice(bytes);
        }
    }
}

// Allow list_compounds closures to call push_compound on the Vec
pub trait CompoundListWriter {
    fn push_compound(&mut self, f: impl FnOnce(&mut CompoundWriter));
}

impl CompoundListWriter for Vec<Vec<u8>> {
    fn push_compound(&mut self, f: impl FnOnce(&mut CompoundWriter)) {
        let mut buf = Vec::new();
        {
            let mut cw = CompoundWriter { out: &mut buf };
            f(&mut cw);
        }
        buf.push(0); // TAG_End for this compound
        self.push(buf);
    }
}

// ─── Varint encoding (WorldEdit Sponge schematic Data field) ─────────────────

fn pack_varint_array(data: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for val in data.iter().copied() {
        write_varint(&mut out, val);
    }
    out
}

fn write_varint(out: &mut Vec<u8>, mut val: u32) {
    loop {
        let byte = (val & 0x7F) as u8;
        val >>= 7;
        if val == 0 {
            out.push(byte);
            break;
        } else {
            out.push(byte | 0x80);
        }
    }
}
