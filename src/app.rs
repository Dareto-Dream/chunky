use glam::Vec3;
use std::path::PathBuf;
use std::sync::{mpsc, Arc};

use crate::core::export::{export_region, export_schem, ExportSettings};
use crate::core::import::load_model;
use crate::core::optimize::OptimizeSettings;
use crate::core::palette::{apply_palette, BlockDatabase, PaletteSettings};
use crate::core::scene::Scene;
use crate::core::voxel::VoxelGrid;
use crate::core::voxelize::{voxelize, VoxelizeSettings};
use crate::renderer::camera::OrbitCamera;
use crate::renderer::voxel_renderer::VoxelRenderer;

// ─── Worker messages ─────────────────────────────────────────────────────────

pub enum WorkerMsg {
    Progress(f32, String),
    SceneLoaded(Box<Scene>),
    VoxelsDone(Box<VoxelGrid>),
    Error(String),
}

// ─── App state ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    Voxel,
    Mesh,
    Blocks,
}

pub struct ChunkyApp {
    // Data
    pub scene: Option<Arc<Scene>>,
    pub voxel_grid: Option<Arc<VoxelGrid>>,

    // Settings
    pub voxel_settings: VoxelizeSettings,
    pub palette_settings: PaletteSettings,
    pub export_settings: ExportSettings,
    pub optimize_settings: OptimizeSettings,

    // Transform
    pub transform_scale: f32,
    pub transform_offset: Vec3,

    // UI
    pub view_mode: ViewMode,
    pub show_grid: bool,
    pub show_chunk_boundaries: bool,
    pub camera: OrbitCamera,
    pub drag_start: Option<egui::Pos2>,
    pub drag_button: Option<egui::PointerButton>,

    // Status
    pub status_message: String,
    pub is_working: bool,
    pub work_progress: f32,
    pub work_label: String,

    // Worker channel
    pub rx: Option<mpsc::Receiver<WorkerMsg>>,

    // Block database (shared)
    pub block_db: Arc<BlockDatabase>,

    // Dirty flag: voxel grid needs to be uploaded to GPU
    pub gpu_dirty: bool,
}

impl ChunkyApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Initialize wgpu renderer in callback resources
        if let Some(render_state) = cc.wgpu_render_state.as_ref() {
            let device = &render_state.device;
            let renderer = VoxelRenderer::new(device, render_state.target_format);
            render_state
                .renderer
                .write()
                .callback_resources
                .insert(renderer);
        }

        Self {
            scene: None,
            voxel_grid: None,

            voxel_settings: VoxelizeSettings::default(),
            palette_settings: PaletteSettings::default(),
            export_settings: ExportSettings::default(),
            optimize_settings: OptimizeSettings::default(),

            transform_scale: 1.0,
            transform_offset: Vec3::ZERO,

            view_mode: ViewMode::Voxel,
            show_grid: true,
            show_chunk_boundaries: false,
            camera: OrbitCamera::default(),
            drag_start: None,
            drag_button: None,

            status_message: "Ready — import a model to begin".to_string(),
            is_working: false,
            work_progress: 0.0,
            work_label: String::new(),

            rx: None,
            block_db: Arc::new(BlockDatabase::new()),
            gpu_dirty: false,
        }
    }

    // ─── Actions ─────────────────────────────────────────────────────────────

    pub fn open_file_dialog(&mut self) {
        let path = rfd::FileDialog::new()
            .set_title("Import 3D Model")
            .add_filter("3D Models", &["obj", "glb", "gltf", "stl"])
            .add_filter("OBJ", &["obj"])
            .add_filter("GLTF/GLB", &["glb", "gltf"])
            .add_filter("STL", &["stl"])
            .pick_file();

        if let Some(path) = path {
            self.load_model_from_path(path);
        }
    }

    pub fn load_model_from_path(&mut self, path: PathBuf) {
        self.is_working = true;
        self.work_progress = 0.0;
        self.work_label = format!(
            "Loading {}...",
            path.file_name().unwrap_or_default().to_string_lossy()
        );
        self.status_message = self.work_label.clone();

        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);

        std::thread::spawn(move || {
            let _ = tx.send(WorkerMsg::Progress(0.1, "Parsing model...".into()));
            match load_model(&path) {
                Ok(scene) => {
                    let _ = tx.send(WorkerMsg::Progress(1.0, "Model loaded".into()));
                    let _ = tx.send(WorkerMsg::SceneLoaded(Box::new(scene)));
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::Error(e.to_string()));
                }
            }
        });
    }

    pub fn run_voxelization(&mut self) {
        let Some(scene_arc) = self.scene.clone() else {
            return;
        };
        let settings = self.voxel_settings.clone();
        let scale = self.transform_scale;
        let offset = self.transform_offset;

        self.is_working = true;
        self.work_progress = 0.0;
        self.work_label = "Voxelizing...".into();

        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);

        let db = self.block_db.clone();
        let palette = self.palette_settings.clone();
        let opt_settings = self.optimize_settings.clone();

        std::thread::spawn(move || {
            let _ = tx.send(WorkerMsg::Progress(0.1, "Rasterizing triangles...".into()));

            // Apply scale + offset to a local copy; original scene stays intact
            // so the user can re-voxelize with different transform settings.
            let mut scene = (*scene_arc).clone();
            if (scale - 1.0).abs() > 1e-6 || offset.length_squared() > 1e-10 {
                for mesh in &mut scene.meshes {
                    for v in &mut mesh.vertices {
                        *v = *v * scale + offset;
                    }
                }
            }

            let mut grid = voxelize(&scene, &settings);
            let _ = tx.send(WorkerMsg::Progress(0.6, "Mapping blocks...".into()));
            apply_palette(&mut grid, &db, &palette);
            let _ = tx.send(WorkerMsg::Progress(0.85, "Optimizing...".into()));
            crate::core::optimize::optimize(&mut grid, &opt_settings);
            let _ = tx.send(WorkerMsg::Progress(1.0, "Done".into()));
            let _ = tx.send(WorkerMsg::VoxelsDone(Box::new(grid)));
        });
    }

    pub fn export_schematic(&mut self) {
        let Some(grid) = self.voxel_grid.clone() else {
            return;
        };
        let db = self.block_db.clone();
        let settings = ExportSettings {
            version: self.export_settings.version,
            offset: self.export_settings.offset,
        };

        let path = rfd::FileDialog::new()
            .set_title("Export Schematic")
            .add_filter("WorldEdit Schematic", &["schem"])
            .set_file_name("export.schem")
            .save_file();

        if let Some(path) = path {
            self.is_working = true;
            self.work_label = "Exporting .schem...".into();
            let (tx, rx) = mpsc::channel();
            self.rx = Some(rx);

            std::thread::spawn(move || {
                let _ = tx.send(WorkerMsg::Progress(0.1, "Writing schematic...".into()));
                match export_schem(&grid, &db, &settings, &path) {
                    Ok(_) => {
                        let _ = tx.send(WorkerMsg::Progress(
                            1.0,
                            format!("Saved to {}", path.display()),
                        ));
                    }
                    Err(e) => {
                        let _ = tx.send(WorkerMsg::Error(e.to_string()));
                    }
                }
            });
        }
    }

    pub fn export_region_files(&mut self) {
        let Some(grid) = self.voxel_grid.clone() else {
            return;
        };
        let db = self.block_db.clone();
        let settings = ExportSettings {
            version: self.export_settings.version,
            offset: self.export_settings.offset,
        };

        let dir = rfd::FileDialog::new()
            .set_title("Select Output Folder for Region Files")
            .pick_folder();

        if let Some(dir) = dir {
            self.is_working = true;
            self.work_label = "Exporting region files...".into();
            let (tx, rx) = mpsc::channel();
            self.rx = Some(rx);

            std::thread::spawn(move || {
                let _ = tx.send(WorkerMsg::Progress(0.1, "Generating chunks...".into()));
                match export_region(&grid, &db, &settings, &dir) {
                    Ok(_) => {
                        let _ = tx.send(WorkerMsg::Progress(
                            1.0,
                            format!("Saved to {}", dir.display()),
                        ));
                    }
                    Err(e) => {
                        let _ = tx.send(WorkerMsg::Error(e.to_string()));
                    }
                }
            });
        }
    }

    // ─── Poll worker ─────────────────────────────────────────────────────────

    pub fn poll_worker(&mut self) {
        let rx = match self.rx.take() {
            Some(rx) => rx,
            None => return,
        };

        let mut last_msg = None;
        let mut disconnected = false;

        // Drain all pending messages
        loop {
            match rx.try_recv() {
                Ok(msg) => {
                    last_msg = Some(msg);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }

        if let Some(msg) = last_msg {
            match msg {
                WorkerMsg::Progress(p, label) => {
                    self.work_progress = p;
                    self.work_label = label.clone();
                    self.status_message = label;
                    self.rx = Some(rx);
                }
                WorkerMsg::SceneLoaded(scene) => {
                    let bounds = scene.bounds();
                    self.scene = Some(Arc::new(*scene));
                    if let Some((min, max)) = bounds {
                        self.camera.fit_to_bounds(min, max);
                    }
                    self.status_message = format!(
                        "Loaded: {} triangles",
                        self.scene
                            .as_ref()
                            .map(|s| s.total_triangles())
                            .unwrap_or(0)
                    );
                    self.is_working = false;
                }
                WorkerMsg::VoxelsDone(grid) => {
                    let voxel_count = grid.total_voxels();
                    if let Some((min, max)) = grid.bounds_voxel() {
                        self.camera
                            .fit_to_bounds(min.as_vec3(), (max + glam::IVec3::ONE).as_vec3());
                    }
                    self.voxel_grid = Some(Arc::new(*grid));
                    self.gpu_dirty = true;
                    self.status_message = format!("Voxelized: {} blocks", voxel_count);
                    self.is_working = false;
                }
                WorkerMsg::Error(e) => {
                    self.status_message = format!("Error: {}", e);
                    self.is_working = false;
                }
            }
        } else if !disconnected {
            self.rx = Some(rx);
        }
    }
}

impl eframe::App for ChunkyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.poll_worker();

        if self.gpu_dirty {
            if let Some(render_state) = frame.wgpu_render_state() {
                if let Some(grid) = self.voxel_grid.clone() {
                    let queue = &render_state.queue;
                    let mut renderer = render_state.renderer.write();
                    if let Some(vr) = renderer.callback_resources.get_mut::<VoxelRenderer>() {
                        vr.update_instances(queue, &grid);
                    }
                }
            }
            self.gpu_dirty = false;
        }

        crate::ui::draw(self, ctx, frame);
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}
