use crate::core::voxel::{VoxelGrid, Voxel};
use glam::IVec3;

#[derive(Debug, Clone)]
pub struct BlockDef {
    pub id: u16,
    pub name: &'static str,
    pub display: &'static str,
    pub color: [u8; 3],
    pub opaque: bool,
}

pub struct BlockDatabase {
    pub blocks: Vec<BlockDef>,
}

impl BlockDatabase {
    pub fn new() -> Self {
        let blocks = builtin_blocks();
        Self { blocks }
    }

    pub fn find_best_match(&self, color: [u8; 3], allowed_ids: Option<&[u16]>) -> u16 {
        let lab = rgb_to_lab(color);
        let mut best_id = 1u16;
        let mut best_dist = f64::MAX;

        for block in &self.blocks {
            if let Some(ids) = allowed_ids {
                if !ids.contains(&block.id) {
                    continue;
                }
            }
            if !block.opaque {
                continue;
            }
            let block_lab = rgb_to_lab(block.color);
            let dist = delta_e(lab, block_lab);
            if dist < best_dist {
                best_dist = dist;
                best_id = block.id;
            }
        }
        best_id
    }

    pub fn get_by_id(&self, id: u16) -> Option<&BlockDef> {
        self.blocks.iter().find(|b| b.id == id)
    }

    pub fn get_color(&self, id: u16) -> [u8; 3] {
        self.get_by_id(id).map(|b| b.color).unwrap_or([128, 128, 128])
    }

    pub fn mc_name(&self, id: u16) -> &str {
        self.get_by_id(id).map(|b| b.name).unwrap_or("minecraft:stone")
    }
}

impl Default for BlockDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct PaletteSettings {
    pub version: MinecraftVersion,
    pub filter: PaletteFilter,
    pub custom_ids: Vec<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MinecraftVersion {
    Java118,
    Java120,
    Java121,
}

impl MinecraftVersion {
    pub fn data_version(&self) -> i32 {
        match self {
            Self::Java118 => 2860,
            Self::Java120 => 3700,
            Self::Java121 => 3837,
        }
    }

    pub fn min_y(&self) -> i32 {
        match self {
            Self::Java118 | Self::Java120 | Self::Java121 => -64,
        }
    }

    pub fn max_y(&self) -> i32 {
        match self {
            Self::Java118 | Self::Java120 | Self::Java121 => 320,
        }
    }
}

impl std::fmt::Display for MinecraftVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Java118 => write!(f, "Java 1.18"),
            Self::Java120 => write!(f, "Java 1.20"),
            Self::Java121 => write!(f, "Java 1.21"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaletteFilter {
    All,
    ConcretesOnly,
    WoolOnly,
    TerraCottaOnly,
    Custom,
}

impl Default for PaletteSettings {
    fn default() -> Self {
        Self {
            version: MinecraftVersion::Java121,
            filter: PaletteFilter::All,
            custom_ids: Vec::new(),
        }
    }
}

pub fn apply_palette(grid: &mut VoxelGrid, db: &BlockDatabase, settings: &PaletteSettings) {
    let allowed = match settings.filter {
        PaletteFilter::All => None,
        PaletteFilter::ConcretesOnly => Some(CONCRETE_IDS.to_vec()),
        PaletteFilter::WoolOnly => Some(WOOL_IDS.to_vec()),
        PaletteFilter::TerraCottaOnly => Some(TERRACOTTA_IDS.to_vec()),
        PaletteFilter::Custom => {
            if settings.custom_ids.is_empty() { None } else { Some(settings.custom_ids.clone()) }
        }
    };

    // Collect all occupied voxel coords to avoid borrow issues
    let coords: Vec<IVec3> = grid.iter_occupied().map(|(pos, _)| pos).collect();
    let colors: Vec<[u8; 3]> = coords.iter()
        .map(|&pos| grid.get_voxel(pos).map(|v| v.color).unwrap_or([128,128,128]))
        .collect();

    let mapped: Vec<u16> = colors.iter()
        .map(|&color| db.find_best_match(color, allowed.as_deref()))
        .collect();

    for (i, &pos) in coords.iter().enumerate() {
        let (chunk_coord, [lx, ly, lz]) = VoxelGrid::voxel_to_chunk(pos);
        if let Some(chunk) = grid.chunks.get_mut(&chunk_coord) {
            let cell = chunk.get_mut(lx, ly, lz);
            cell.block_id = mapped[i];
            cell.color = db.get_color(mapped[i]);
        }
    }
}

// --- Color math ---

fn rgb_to_linear(c: u8) -> f64 {
    let v = c as f64 / 255.0;
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

fn linear_to_xyz(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let x = r * 0.4124564 + g * 0.3575761 + b * 0.1804375;
    let y = r * 0.2126729 + g * 0.7151522 + b * 0.0721750;
    let z = r * 0.0193339 + g * 0.1191920 + b * 0.9503041;
    (x, y, z)
}

fn xyz_to_lab(x: f64, y: f64, z: f64) -> (f64, f64, f64) {
    fn f(t: f64) -> f64 {
        if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 }
    }
    let fx = f(x / 0.95047);
    let fy = f(y / 1.00000);
    let fz = f(z / 1.08883);
    let l = 116.0 * fy - 16.0;
    let a = 500.0 * (fx - fy);
    let b = 200.0 * (fy - fz);
    (l, a, b)
}

pub fn rgb_to_lab(rgb: [u8; 3]) -> (f64, f64, f64) {
    let r = rgb_to_linear(rgb[0]);
    let g = rgb_to_linear(rgb[1]);
    let b = rgb_to_linear(rgb[2]);
    let (x, y, z) = linear_to_xyz(r, g, b);
    xyz_to_lab(x, y, z)
}

pub fn delta_e(lab1: (f64, f64, f64), lab2: (f64, f64, f64)) -> f64 {
    let dl = lab1.0 - lab2.0;
    let da = lab1.1 - lab2.1;
    let db = lab1.2 - lab2.2;
    (dl * dl + da * da + db * db).sqrt()
}

// Block ID ranges for filters
pub const CONCRETE_IDS: [u16; 16] = [
    100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115,
];
pub const WOOL_IDS: [u16; 16] = [
    50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65,
];
pub const TERRACOTTA_IDS: [u16; 16] = [
    150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165,
];

pub fn builtin_blocks() -> Vec<BlockDef> {
    vec![
        // --- Stone/Rock ---
        BlockDef { id: 1,   name: "minecraft:stone",              display: "Stone",              color: [125, 125, 125], opaque: true },
        BlockDef { id: 2,   name: "minecraft:cobblestone",        display: "Cobblestone",        color: [127, 121, 119], opaque: true },
        BlockDef { id: 3,   name: "minecraft:granite",            display: "Granite",            color: [153, 114, 99],  opaque: true },
        BlockDef { id: 4,   name: "minecraft:diorite",            display: "Diorite",            color: [188, 188, 188], opaque: true },
        BlockDef { id: 5,   name: "minecraft:andesite",           display: "Andesite",           color: [135, 135, 135], opaque: true },
        BlockDef { id: 6,   name: "minecraft:deepslate",          display: "Deepslate",          color: [83,  83,  90],  opaque: true },
        BlockDef { id: 7,   name: "minecraft:cobbled_deepslate",  display: "Cobbled Deepslate",  color: [80,  80,  87],  opaque: true },
        BlockDef { id: 8,   name: "minecraft:tuff",               display: "Tuff",               color: [111, 112, 105], opaque: true },
        BlockDef { id: 9,   name: "minecraft:calcite",            display: "Calcite",            color: [223, 224, 224], opaque: true },
        BlockDef { id: 10,  name: "minecraft:blackstone",         display: "Blackstone",         color: [42,  35,  45],  opaque: true },
        BlockDef { id: 11,  name: "minecraft:basalt",             display: "Basalt",             color: [85,  90,  90],  opaque: true },
        BlockDef { id: 12,  name: "minecraft:netherrack",         display: "Netherrack",         color: [97,  41,  41],  opaque: true },
        BlockDef { id: 13,  name: "minecraft:soul_sand",          display: "Soul Sand",          color: [84,  65,  51],  opaque: true },
        BlockDef { id: 14,  name: "minecraft:end_stone",          display: "End Stone",          color: [219, 222, 158], opaque: true },
        BlockDef { id: 15,  name: "minecraft:obsidian",           display: "Obsidian",           color: [15,  10,  25],  opaque: true },

        // --- Dirt/Ground ---
        BlockDef { id: 20,  name: "minecraft:dirt",               display: "Dirt",               color: [134, 96,  67],  opaque: true },
        BlockDef { id: 21,  name: "minecraft:grass_block",        display: "Grass Block",        color: [94,  157, 52],  opaque: true },
        BlockDef { id: 22,  name: "minecraft:sand",               display: "Sand",               color: [219, 207, 163], opaque: true },
        BlockDef { id: 23,  name: "minecraft:red_sand",           display: "Red Sand",           color: [190, 103, 40],  opaque: true },
        BlockDef { id: 24,  name: "minecraft:gravel",             display: "Gravel",             color: [136, 126, 126], opaque: true },
        BlockDef { id: 25,  name: "minecraft:clay",               display: "Clay",               color: [161, 168, 182], opaque: true },
        BlockDef { id: 26,  name: "minecraft:mycelium",           display: "Mycelium",           color: [111, 100, 112], opaque: true },
        BlockDef { id: 27,  name: "minecraft:mud",                display: "Mud",                color: [60,  49,  46],  opaque: true },
        BlockDef { id: 28,  name: "minecraft:packed_mud",         display: "Packed Mud",         color: [108, 87,  66],  opaque: true },
        BlockDef { id: 29,  name: "minecraft:mud_bricks",         display: "Mud Bricks",         color: [121, 90,  72],  opaque: true },

        // --- Wood Planks ---
        BlockDef { id: 40,  name: "minecraft:oak_planks",         display: "Oak Planks",         color: [197, 163, 97],  opaque: true },
        BlockDef { id: 41,  name: "minecraft:spruce_planks",      display: "Spruce Planks",      color: [115, 84,  51],  opaque: true },
        BlockDef { id: 42,  name: "minecraft:birch_planks",       display: "Birch Planks",       color: [216, 207, 163], opaque: true },
        BlockDef { id: 43,  name: "minecraft:jungle_planks",      display: "Jungle Planks",      color: [160, 115, 80],  opaque: true },
        BlockDef { id: 44,  name: "minecraft:acacia_planks",      display: "Acacia Planks",      color: [168, 93,  52],  opaque: true },
        BlockDef { id: 45,  name: "minecraft:dark_oak_planks",    display: "Dark Oak Planks",    color: [67,  43,  20],  opaque: true },
        BlockDef { id: 46,  name: "minecraft:mangrove_planks",    display: "Mangrove Planks",    color: [118, 54,  48],  opaque: true },
        BlockDef { id: 47,  name: "minecraft:cherry_planks",      display: "Cherry Planks",      color: [220, 166, 161], opaque: true },
        BlockDef { id: 48,  name: "minecraft:bamboo_planks",      display: "Bamboo Planks",      color: [197, 176, 83],  opaque: true },
        BlockDef { id: 49,  name: "minecraft:crimson_planks",     display: "Crimson Planks",     color: [149, 84,  110], opaque: true },

        // --- Wool ---
        BlockDef { id: 50,  name: "minecraft:white_wool",         display: "White Wool",         color: [233, 236, 236], opaque: true },
        BlockDef { id: 51,  name: "minecraft:orange_wool",        display: "Orange Wool",        color: [240, 118, 19],  opaque: true },
        BlockDef { id: 52,  name: "minecraft:magenta_wool",       display: "Magenta Wool",       color: [189, 68,  179], opaque: true },
        BlockDef { id: 53,  name: "minecraft:light_blue_wool",    display: "Light Blue Wool",    color: [58,  175, 217], opaque: true },
        BlockDef { id: 54,  name: "minecraft:yellow_wool",        display: "Yellow Wool",        color: [248, 197, 39],  opaque: true },
        BlockDef { id: 55,  name: "minecraft:lime_wool",          display: "Lime Wool",          color: [112, 185, 25],  opaque: true },
        BlockDef { id: 56,  name: "minecraft:pink_wool",          display: "Pink Wool",          color: [237, 141, 172], opaque: true },
        BlockDef { id: 57,  name: "minecraft:gray_wool",          display: "Gray Wool",          color: [62,  68,  71],  opaque: true },
        BlockDef { id: 58,  name: "minecraft:light_gray_wool",    display: "Light Gray Wool",    color: [142, 142, 134], opaque: true },
        BlockDef { id: 59,  name: "minecraft:cyan_wool",          display: "Cyan Wool",          color: [21,  137, 145], opaque: true },
        BlockDef { id: 60,  name: "minecraft:purple_wool",        display: "Purple Wool",        color: [121, 42,  172], opaque: true },
        BlockDef { id: 61,  name: "minecraft:blue_wool",          display: "Blue Wool",          color: [53,  57,  157], opaque: true },
        BlockDef { id: 62,  name: "minecraft:brown_wool",         display: "Brown Wool",         color: [114, 71,  40],  opaque: true },
        BlockDef { id: 63,  name: "minecraft:green_wool",         display: "Green Wool",         color: [84,  109, 27],  opaque: true },
        BlockDef { id: 64,  name: "minecraft:red_wool",           display: "Red Wool",           color: [160, 39,  34],  opaque: true },
        BlockDef { id: 65,  name: "minecraft:black_wool",         display: "Black Wool",         color: [20,  21,  25],  opaque: true },

        // --- Concrete ---
        BlockDef { id: 100, name: "minecraft:white_concrete",     display: "White Concrete",     color: [207, 213, 214], opaque: true },
        BlockDef { id: 101, name: "minecraft:orange_concrete",    display: "Orange Concrete",    color: [224, 97,  0],   opaque: true },
        BlockDef { id: 102, name: "minecraft:magenta_concrete",   display: "Magenta Concrete",   color: [169, 48,  159], opaque: true },
        BlockDef { id: 103, name: "minecraft:light_blue_concrete",display: "Light Blue Concrete",color: [36,  137, 199], opaque: true },
        BlockDef { id: 104, name: "minecraft:yellow_concrete",    display: "Yellow Concrete",    color: [240, 175, 21],  opaque: true },
        BlockDef { id: 105, name: "minecraft:lime_concrete",      display: "Lime Concrete",      color: [94,  168, 24],  opaque: true },
        BlockDef { id: 106, name: "minecraft:pink_concrete",      display: "Pink Concrete",      color: [214, 101, 143], opaque: true },
        BlockDef { id: 107, name: "minecraft:gray_concrete",      display: "Gray Concrete",      color: [54,  57,  61],  opaque: true },
        BlockDef { id: 108, name: "minecraft:light_gray_concrete",display: "Light Gray Concrete",color: [125, 125, 115], opaque: true },
        BlockDef { id: 109, name: "minecraft:cyan_concrete",      display: "Cyan Concrete",      color: [21,  119, 136], opaque: true },
        BlockDef { id: 110, name: "minecraft:purple_concrete",    display: "Purple Concrete",    color: [100, 31,  156], opaque: true },
        BlockDef { id: 111, name: "minecraft:blue_concrete",      display: "Blue Concrete",      color: [44,  46,  143], opaque: true },
        BlockDef { id: 112, name: "minecraft:brown_concrete",     display: "Brown Concrete",     color: [96,  59,  31],  opaque: true },
        BlockDef { id: 113, name: "minecraft:green_concrete",     display: "Green Concrete",     color: [73,  91,  36],  opaque: true },
        BlockDef { id: 114, name: "minecraft:red_concrete",       display: "Red Concrete",       color: [142, 32,  32],  opaque: true },
        BlockDef { id: 115, name: "minecraft:black_concrete",     display: "Black Concrete",     color: [8,   10,  15],  opaque: true },

        // --- Terracotta ---
        BlockDef { id: 150, name: "minecraft:white_terracotta",   display: "White Terracotta",   color: [209, 177, 161], opaque: true },
        BlockDef { id: 151, name: "minecraft:orange_terracotta",  display: "Orange Terracotta",  color: [161, 83,  37],  opaque: true },
        BlockDef { id: 152, name: "minecraft:magenta_terracotta", display: "Magenta Terracotta", color: [149, 88,  108], opaque: true },
        BlockDef { id: 153, name: "minecraft:light_blue_terracotta", display: "Light Blue Terracotta", color: [113, 108, 137], opaque: true },
        BlockDef { id: 154, name: "minecraft:yellow_terracotta",  display: "Yellow Terracotta",  color: [186, 133, 35],  opaque: true },
        BlockDef { id: 155, name: "minecraft:lime_terracotta",    display: "Lime Terracotta",    color: [103, 117, 52],  opaque: true },
        BlockDef { id: 156, name: "minecraft:pink_terracotta",    display: "Pink Terracotta",    color: [161, 78,  78],  opaque: true },
        BlockDef { id: 157, name: "minecraft:gray_terracotta",    display: "Gray Terracotta",    color: [57,  42,  35],  opaque: true },
        BlockDef { id: 158, name: "minecraft:light_gray_terracotta", display: "Light Gray Terracotta", color: [135, 107, 98], opaque: true },
        BlockDef { id: 159, name: "minecraft:cyan_terracotta",    display: "Cyan Terracotta",    color: [86,  91,  91],  opaque: true },
        BlockDef { id: 160, name: "minecraft:purple_terracotta",  display: "Purple Terracotta",  color: [118, 70,  86],  opaque: true },
        BlockDef { id: 161, name: "minecraft:blue_terracotta",    display: "Blue Terracotta",    color: [74,  59,  91],  opaque: true },
        BlockDef { id: 162, name: "minecraft:brown_terracotta",   display: "Brown Terracotta",   color: [77,  51,  35],  opaque: true },
        BlockDef { id: 163, name: "minecraft:green_terracotta",   display: "Green Terracotta",   color: [76,  83,  42],  opaque: true },
        BlockDef { id: 164, name: "minecraft:red_terracotta",     display: "Red Terracotta",     color: [143, 61,  46],  opaque: true },
        BlockDef { id: 165, name: "minecraft:black_terracotta",   display: "Black Terracotta",   color: [37,  22,  16],  opaque: true },

        // --- Glazed Terracotta (selected) ---
        BlockDef { id: 170, name: "minecraft:terracotta",         display: "Terracotta",         color: [152, 94,  67],  opaque: true },

        // --- Metals/Special ---
        BlockDef { id: 200, name: "minecraft:iron_block",         display: "Iron Block",         color: [220, 220, 220], opaque: true },
        BlockDef { id: 201, name: "minecraft:gold_block",         display: "Gold Block",         color: [246, 208, 61],  opaque: true },
        BlockDef { id: 202, name: "minecraft:diamond_block",      display: "Diamond Block",      color: [100, 220, 214], opaque: true },
        BlockDef { id: 203, name: "minecraft:emerald_block",      display: "Emerald Block",      color: [17,  179, 64],  opaque: true },
        BlockDef { id: 204, name: "minecraft:lapis_block",        display: "Lapis Block",        color: [28,  68,  142], opaque: true },
        BlockDef { id: 205, name: "minecraft:redstone_block",     display: "Redstone Block",     color: [175, 7,   7],   opaque: true },
        BlockDef { id: 206, name: "minecraft:copper_block",       display: "Copper Block",       color: [196, 108, 74],  opaque: true },
        BlockDef { id: 207, name: "minecraft:exposed_copper",     display: "Exposed Copper",     color: [151, 122, 95],  opaque: true },
        BlockDef { id: 208, name: "minecraft:weathered_copper",   display: "Weathered Copper",   color: [100, 153, 116], opaque: true },
        BlockDef { id: 209, name: "minecraft:oxidized_copper",    display: "Oxidized Copper",    color: [82,  179, 139], opaque: true },
        BlockDef { id: 210, name: "minecraft:amethyst_block",     display: "Amethyst Block",     color: [155, 105, 211], opaque: true },
        BlockDef { id: 211, name: "minecraft:netherite_block",    display: "Netherite Block",    color: [67,  61,  66],  opaque: true },

        // --- Snow/Ice ---
        BlockDef { id: 220, name: "minecraft:snow_block",         display: "Snow Block",         color: [249, 251, 254], opaque: true },
        BlockDef { id: 221, name: "minecraft:packed_ice",         display: "Packed Ice",         color: [161, 197, 229], opaque: true },
        BlockDef { id: 222, name: "minecraft:blue_ice",           display: "Blue Ice",           color: [116, 167, 228], opaque: true },

        // --- Special ---
        BlockDef { id: 230, name: "minecraft:glowstone",          display: "Glowstone",          color: [207, 169, 96],  opaque: true },
        BlockDef { id: 231, name: "minecraft:purpur_block",       display: "Purpur Block",       color: [169, 125, 169], opaque: true },
        BlockDef { id: 232, name: "minecraft:sponge",             display: "Sponge",             color: [182, 182, 55],  opaque: true },
        BlockDef { id: 233, name: "minecraft:pumpkin",            display: "Pumpkin",            color: [198, 118, 24],  opaque: true },
        BlockDef { id: 234, name: "minecraft:hay_block",          display: "Hay Block",          color: [175, 157, 22],  opaque: true },
        BlockDef { id: 235, name: "minecraft:bookshelf",          display: "Bookshelf",          color: [197, 162, 113], opaque: true },
        BlockDef { id: 236, name: "minecraft:warped_planks",      display: "Warped Planks",      color: [43,  136, 134], opaque: true },
        BlockDef { id: 237, name: "minecraft:nether_bricks",      display: "Nether Bricks",      color: [48,  18,  18],  opaque: true },
        BlockDef { id: 238, name: "minecraft:quartz_block",       display: "Quartz Block",       color: [235, 229, 222], opaque: true },
        BlockDef { id: 239, name: "minecraft:smooth_quartz",      display: "Smooth Quartz",      color: [234, 231, 224], opaque: true },
        BlockDef { id: 240, name: "minecraft:prismarine",         display: "Prismarine",         color: [99,  171, 158], opaque: true },
        BlockDef { id: 241, name: "minecraft:dark_prismarine",    display: "Dark Prismarine",    color: [51,  95,  78],  opaque: true },
    ]
}
