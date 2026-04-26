use eframe::egui_wgpu;
use egui::*;
use glam::{IVec3, Vec3};

use crate::app::{ChunkyApp, ViewMode};
use crate::core::palette::{MinecraftVersion, PaletteFilter};
use crate::core::voxelize::VoxelizeMode;
use crate::renderer::camera::OrbitCamera;
use crate::renderer::voxel_renderer::VoxelRenderer;

// ─── Theme ────────────────────────────────────────────────────────────────────

const BG_DARK: Color32 = Color32::from_rgb(22, 22, 30);
const PANEL_BG: Color32 = Color32::from_rgb(30, 30, 42);
const ACCENT: Color32 = Color32::from_rgb(92, 186, 71);
const ACCENT_DIM: Color32 = Color32::from_rgb(60, 130, 46);
const TEXT: Color32 = Color32::from_rgb(220, 220, 220);
const TEXT_DIM: Color32 = Color32::from_rgb(140, 140, 150);
const ERR: Color32 = Color32::from_rgb(220, 60, 60);

pub fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals.window_fill = BG_DARK;
    style.visuals.panel_fill = PANEL_BG;
    style.visuals.faint_bg_color = Color32::from_rgb(35, 35, 48);
    style.visuals.extreme_bg_color = Color32::from_rgb(18, 18, 26);
    style.visuals.code_bg_color = Color32::from_rgb(20, 20, 28);
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(38, 38, 52);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(42, 42, 58);
    style.visuals.widgets.active.bg_fill = ACCENT_DIM;
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(50, 50, 68);
    style.visuals.selection.bg_fill = ACCENT_DIM;
    style.visuals.hyperlink_color = ACCENT;
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.window_margin = Margin::same(12.0);
    style.spacing.button_padding = Vec2::new(10.0, 5.0);
    ctx.set_style(style);
}

// ─── Main draw entry ─────────────────────────────────────────────────────────

pub fn draw(app: &mut ChunkyApp, ctx: &egui::Context, frame: &mut eframe::Frame) {
    apply_theme(ctx);

    // Top bar
    egui::TopBottomPanel::top("top_bar")
        .frame(
            Frame::none()
                .fill(Color32::from_rgb(18, 18, 26))
                .inner_margin(Margin::symmetric(12.0, 8.0)),
        )
        .show(ctx, |ui| {
            draw_top_bar(app, ui);
        });

    // Bottom status bar
    egui::TopBottomPanel::bottom("status_bar")
        .frame(
            Frame::none()
                .fill(Color32::from_rgb(18, 18, 26))
                .inner_margin(Margin::symmetric(12.0, 4.0)),
        )
        .show(ctx, |ui| {
            draw_status_bar(app, ui);
        });

    // Left panel
    egui::SidePanel::left("left_panel")
        .resizable(true)
        .default_width(260.0)
        .min_width(200.0)
        .max_width(400.0)
        .frame(Frame::none().fill(PANEL_BG).inner_margin(Margin::same(0.0)))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                draw_left_panel(app, ui);
            });
        });

    // Right panel
    egui::SidePanel::right("right_panel")
        .resizable(true)
        .default_width(260.0)
        .min_width(200.0)
        .max_width(400.0)
        .frame(Frame::none().fill(PANEL_BG).inner_margin(Margin::same(0.0)))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                draw_right_panel(app, ui);
            });
        });

    // Central viewport
    egui::CentralPanel::default()
        .frame(Frame::none().fill(BG_DARK))
        .show(ctx, |ui| {
            draw_viewport(app, ui, frame);
        });
}

// ─── Top bar ─────────────────────────────────────────────────────────────────

fn draw_top_bar(app: &mut ChunkyApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        // Logo
        ui.label(RichText::new("⬛ CHUNKY").color(ACCENT).size(18.0).strong());
        ui.separator();

        // Import
        let import_btn =
            Button::new(RichText::new("📂 Import").color(TEXT)).fill(Color32::from_rgb(42, 42, 58));
        if ui.add_enabled(!app.is_working, import_btn).clicked() {
            app.open_file_dialog();
        }

        // Voxelize
        let can_voxelize = app.scene.is_some() && !app.is_working;
        let vox_btn = Button::new(RichText::new("⚡ Voxelize").color(if can_voxelize {
            ACCENT
        } else {
            TEXT_DIM
        }))
        .fill(if can_voxelize {
            ACCENT_DIM
        } else {
            Color32::from_rgb(42, 42, 58)
        });
        if ui.add_enabled(can_voxelize, vox_btn).clicked() {
            app.run_voxelization();
        }

        ui.separator();

        // Export buttons
        let can_export = app.voxel_grid.is_some() && !app.is_working;

        let schem_btn =
            Button::new(RichText::new("💾 .schem").color(TEXT)).fill(Color32::from_rgb(42, 42, 58));
        if ui.add_enabled(can_export, schem_btn).clicked() {
            app.export_schematic();
        }

        let mca_btn =
            Button::new(RichText::new("🗺 .mca").color(TEXT)).fill(Color32::from_rgb(42, 42, 58));
        if ui.add_enabled(can_export, mca_btn).clicked() {
            app.export_region_files();
        }

        ui.separator();

        // View modes
        ui.label(RichText::new("View:").color(TEXT_DIM).small());
        for (label, mode) in [
            ("Voxel", ViewMode::Voxel),
            ("Mesh", ViewMode::Mesh),
            ("Blocks", ViewMode::Blocks),
        ] {
            let selected = app.view_mode == mode;
            let btn = SelectableLabel::new(selected, label);
            if ui.add(btn).clicked() {
                app.view_mode = mode;
            }
        }

        ui.separator();
        ui.checkbox(
            &mut app.show_grid,
            RichText::new("Grid").color(TEXT_DIM).small(),
        );
        ui.checkbox(
            &mut app.show_chunk_boundaries,
            RichText::new("Chunks").color(TEXT_DIM).small(),
        );

        // Right-align: stats
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if let Some(grid) = &app.voxel_grid {
                let voxels = grid.total_voxels();
                ui.label(
                    RichText::new(format!("{} blocks", voxels))
                        .color(ACCENT)
                        .small(),
                );
                ui.separator();
            }
            if let Some(scene) = &app.scene {
                ui.label(
                    RichText::new(format!("{} tris", scene.total_triangles()))
                        .color(TEXT_DIM)
                        .small(),
                );
            }
        });
    });
}

// ─── Left panel: Import + Transform + Voxelization ───────────────────────────

fn draw_left_panel(app: &mut ChunkyApp, ui: &mut Ui) {
    section_header(ui, "IMPORT");
    ui.add_space(4.0);

    Frame::none()
        .inner_margin(Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            if let Some(scene) = &app.scene {
                let path = scene.source_path.as_deref().unwrap_or("unknown");
                let filename = std::path::Path::new(path)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or(path);
                ui.label(RichText::new(filename).color(ACCENT).strong());
                ui.label(
                    RichText::new(format!(
                        "{} meshes · {} triangles",
                        scene.meshes.len(),
                        scene.total_triangles()
                    ))
                    .color(TEXT_DIM)
                    .small(),
                );

                if let Some((min, max)) = scene.bounds() {
                    let size = max - min;
                    ui.label(
                        RichText::new(format!(
                            "Bounds: {:.2} × {:.2} × {:.2} units",
                            size.x, size.y, size.z
                        ))
                        .color(TEXT_DIM)
                        .small(),
                    );
                }
            } else {
                let drop_area = ui.add_sized(
                    [ui.available_width(), 80.0],
                    Button::new(
                        RichText::new("📂  Drop .obj / .glb / .stl\n  or click to browse")
                            .color(TEXT_DIM),
                    )
                    .fill(Color32::from_rgb(28, 28, 40)),
                );
                if drop_area.clicked() {
                    app.open_file_dialog();
                }
            }

            // Handle file drops
            if !ui.ctx().input(|i| i.raw.dropped_files.is_empty()) {
                if let Some(file) = ui.ctx().input(|i| i.raw.dropped_files.first().cloned()) {
                    if let Some(path) = file.path {
                        app.load_model_from_path(path);
                    }
                }
            }
        });

    ui.separator();
    section_header(ui, "TRANSFORM");
    Frame::none()
        .inner_margin(Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Scale").color(TEXT_DIM).small());
                ui.add(
                    Slider::new(&mut app.transform_scale, 0.01..=100.0)
                        .logarithmic(true)
                        .suffix("×"),
                );
            });

            ui.horizontal(|ui| {
                ui.label(RichText::new("Offset Y").color(TEXT_DIM).small());
                ui.add(Slider::new(&mut app.transform_offset.y, -100.0..=100.0));
            });

            ui.horizontal(|ui| {
                if ui.small_button("Center").clicked() {
                    app.transform_offset = glam::Vec3::ZERO;
                }
                if ui.small_button("Ground").clicked() {
                    if let Some(scene) = &app.scene {
                        if let Some((min, _)) = scene.bounds() {
                            app.transform_offset.y = -min.y;
                        }
                    }
                }
                if ui.small_button("Reset").clicked() {
                    app.transform_scale = 1.0;
                    app.transform_offset = glam::Vec3::ZERO;
                }
            });
        });

    ui.separator();
    section_header(ui, "VOXELIZATION");
    Frame::none()
        .inner_margin(Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            // Mode
            ui.horizontal(|ui| {
                ui.label(RichText::new("Mode").color(TEXT_DIM).small());
                ComboBox::from_id_source("vox_mode")
                    .selected_text(format!("{:?}", app.voxel_settings.mode))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut app.voxel_settings.mode,
                            VoxelizeMode::Surface,
                            "Surface",
                        );
                        ui.selectable_value(
                            &mut app.voxel_settings.mode,
                            VoxelizeMode::Solid,
                            "Solid",
                        );
                        ui.selectable_value(
                            &mut app.voxel_settings.mode,
                            VoxelizeMode::Hybrid,
                            "Hybrid",
                        );
                    });
            });

            // Resolution
            ui.horizontal(|ui| {
                ui.label(RichText::new("Resolution").color(TEXT_DIM).small());
                ui.add(
                    Slider::new(&mut app.voxel_settings.resolution, 4.0..=512.0)
                        .logarithmic(true)
                        .suffix(" b/u"),
                );
            });

            if app.voxel_settings.mode == VoxelizeMode::Solid
                || app.voxel_settings.mode == VoxelizeMode::Hybrid
            {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Infill").color(TEXT_DIM).small());
                    ui.add(
                        Slider::new(&mut app.voxel_settings.infill_density, 0.0..=1.0)
                            .fixed_decimals(2),
                    );
                });
            }

            // Estimated block count
            if let Some(scene) = &app.scene {
                if let Some((_min, _max)) = scene.bounds() {
                    let size = scene.size() * app.voxel_settings.resolution;
                    let est = size.x as u64 * size.y as u64 * size.z as u64;
                    let (val, unit) = if est > 1_000_000 {
                        (est / 1_000_000, "M")
                    } else if est > 1_000 {
                        (est / 1_000, "k")
                    } else {
                        (est, "")
                    };
                    ui.label(
                        RichText::new(format!("~{}{} voxels (max)", val, unit))
                            .color(TEXT_DIM)
                            .small(),
                    );
                }
            }
        });

    ui.separator();
    section_header(ui, "OPTIMIZE");
    Frame::none()
        .inner_margin(Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            ui.checkbox(
                &mut app.optimize_settings.remove_hidden,
                "Remove hidden faces",
            );
            ui.horizontal(|ui| {
                ui.checkbox(&mut app.optimize_settings.noise_filter, "Noise filter");
                if app.optimize_settings.noise_filter {
                    ui.add(
                        Slider::new(&mut app.optimize_settings.noise_threshold, 1..=6)
                            .text("min neighbors"),
                    );
                }
            });
        });
}

// ─── Right panel: Palette + Export ───────────────────────────────────────────

fn draw_right_panel(app: &mut ChunkyApp, ui: &mut Ui) {
    section_header(ui, "BLOCK PALETTE");
    Frame::none()
        .inner_margin(Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Version").color(TEXT_DIM).small());
                ComboBox::from_id_source("mc_version")
                    .selected_text(app.palette_settings.version.to_string())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut app.palette_settings.version,
                            MinecraftVersion::Java121,
                            "Java 1.21",
                        );
                        ui.selectable_value(
                            &mut app.palette_settings.version,
                            MinecraftVersion::Java120,
                            "Java 1.20",
                        );
                        ui.selectable_value(
                            &mut app.palette_settings.version,
                            MinecraftVersion::Java118,
                            "Java 1.18",
                        );
                    });
            });

            ui.horizontal(|ui| {
                ui.label(RichText::new("Filter").color(TEXT_DIM).small());
                ComboBox::from_id_source("pal_filter")
                    .selected_text(format!("{:?}", app.palette_settings.filter))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut app.palette_settings.filter,
                            PaletteFilter::All,
                            "All blocks",
                        );
                        ui.selectable_value(
                            &mut app.palette_settings.filter,
                            PaletteFilter::ConcretesOnly,
                            "Concrete only",
                        );
                        ui.selectable_value(
                            &mut app.palette_settings.filter,
                            PaletteFilter::WoolOnly,
                            "Wool only",
                        );
                        ui.selectable_value(
                            &mut app.palette_settings.filter,
                            PaletteFilter::TerraCottaOnly,
                            "Terracotta only",
                        );
                    });
            });

            // Show palette preview swatch grid
            if let Some(grid) = &app.voxel_grid {
                ui.add_space(6.0);
                ui.label(RichText::new("Used blocks:").color(TEXT_DIM).small());

                let db = &app.block_db;
                let mut used_ids: Vec<u16> = Vec::new();
                for (_, voxel) in grid.iter_occupied() {
                    if voxel.block_id > 0 && !used_ids.contains(&voxel.block_id) {
                        used_ids.push(voxel.block_id);
                    }
                }
                used_ids.sort();

                let swatch_size = 16.0;
                let cols = (ui.available_width() / (swatch_size + 2.0)) as usize;
                let cols = cols.max(1);

                egui::Grid::new("palette_swatches")
                    .num_columns(cols)
                    .spacing([2.0, 2.0])
                    .show(ui, |ui| {
                        for (i, &id) in used_ids.iter().enumerate() {
                            let color = db.get_color(id);
                            let rect = ui.allocate_space(Vec2::splat(swatch_size)).1;
                            ui.painter().rect_filled(
                                rect,
                                2.0,
                                Color32::from_rgb(color[0], color[1], color[2]),
                            );

                            let resp = ui.interact(rect, Id::new(("swatch", id)), Sense::hover());
                            resp.on_hover_text(db.get_by_id(id).map(|b| b.display).unwrap_or("?"));

                            if (i + 1) % cols == 0 {
                                ui.end_row();
                            }
                        }
                    });
            }
        });

    ui.separator();
    section_header(ui, "EXPORT");
    Frame::none()
        .inner_margin(Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Version").color(TEXT_DIM).small());
                ComboBox::from_id_source("exp_version")
                    .selected_text(app.export_settings.version.to_string())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut app.export_settings.version,
                            MinecraftVersion::Java121,
                            "Java 1.21",
                        );
                        ui.selectable_value(
                            &mut app.export_settings.version,
                            MinecraftVersion::Java120,
                            "Java 1.20",
                        );
                        ui.selectable_value(
                            &mut app.export_settings.version,
                            MinecraftVersion::Java118,
                            "Java 1.18",
                        );
                    });
            });

            ui.label(RichText::new("World offset:").color(TEXT_DIM).small());
            ui.horizontal(|ui| {
                ui.label(RichText::new("X").color(TEXT_DIM).small());
                ui.add(DragValue::new(&mut app.export_settings.offset.x).speed(1));
                ui.label(RichText::new("Y").color(TEXT_DIM).small());
                ui.add(DragValue::new(&mut app.export_settings.offset.y).speed(1));
                ui.label(RichText::new("Z").color(TEXT_DIM).small());
                ui.add(DragValue::new(&mut app.export_settings.offset.z).speed(1));
            });

            ui.add_space(8.0);

            let can_export = app.voxel_grid.is_some() && !app.is_working;

            ui.add_enabled_ui(can_export, |ui| {
                ui.vertical(|ui| {
                    let schem = ui.add_sized(
                        [ui.available_width(), 32.0],
                        Button::new(RichText::new("💾  Export .schem (WorldEdit)").color(TEXT))
                            .fill(Color32::from_rgb(42, 42, 58)),
                    );
                    if schem.clicked() {
                        app.export_schematic();
                    }

                    let mca = ui.add_sized(
                        [ui.available_width(), 32.0],
                        Button::new(RichText::new("🗺  Export .mca (Region Files)").color(TEXT))
                            .fill(Color32::from_rgb(42, 42, 58)),
                    );
                    if mca.clicked() {
                        app.export_region_files();
                    }
                });
            });

            if !can_export {
                ui.label(
                    RichText::new("Voxelize model first")
                        .color(TEXT_DIM)
                        .small(),
                );
            }

            // Stats
            if let Some(grid) = &app.voxel_grid {
                ui.add_space(8.0);
                ui.separator();
                let total = grid.total_voxels();
                let chunks = grid.chunks.len();
                ui.label(
                    RichText::new(format!("Total blocks: {}", total))
                        .color(TEXT_DIM)
                        .small(),
                );
                ui.label(
                    RichText::new(format!("Chunks: {} ({} × 32³)", chunks, chunks))
                        .color(TEXT_DIM)
                        .small(),
                );

                if let Some((min, max)) = grid.bounds_voxel() {
                    let size = max - min + IVec3::ONE;
                    ui.label(
                        RichText::new(format!(
                            "Dimensions: {}×{}×{} blocks",
                            size.x, size.y, size.z
                        ))
                        .color(TEXT_DIM)
                        .small(),
                    );
                }
            }
        });

    ui.separator();
    section_header(ui, "HELP");
    Frame::none()
        .inner_margin(Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new("Orbit: Left drag\nZoom: Scroll\nPan: Right drag / Middle drag")
                    .color(TEXT_DIM)
                    .small(),
            );
        });
}

// ─── Viewport ─────────────────────────────────────────────────────────────────

fn draw_viewport(app: &mut ChunkyApp, ui: &mut Ui, _frame: &mut eframe::Frame) {
    let rect = ui.available_rect_before_wrap();

    // Handle mouse for orbit camera
    let response = ui.allocate_rect(rect, Sense::click_and_drag());

    // Zoom
    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
    if scroll != 0.0 && rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default())) {
        app.camera.zoom(scroll);
    }

    // Drag
    if response.drag_started() {
        app.drag_start = response.interact_pointer_pos();
        app.drag_button = Some(if response.drag_started_by(PointerButton::Primary) {
            PointerButton::Primary
        } else if response.drag_started_by(PointerButton::Secondary) {
            PointerButton::Secondary
        } else {
            PointerButton::Middle
        });
    }

    let drag_delta = response.drag_delta();
    if drag_delta.length() > 0.0 {
        match app.drag_button {
            Some(PointerButton::Primary) => {
                app.camera
                    .orbit(glam::Vec2::new(drag_delta.x, drag_delta.y));
            }
            Some(PointerButton::Secondary) | Some(PointerButton::Middle) => {
                app.camera.pan(glam::Vec2::new(drag_delta.x, -drag_delta.y));
            }
            _ => {}
        }
    }

    if response.drag_stopped() {
        app.drag_start = None;
        app.drag_button = None;
    }

    // Update camera aspect ratio
    app.camera.aspect = rect.width() / rect.height().max(1.0);

    // Double-click to re-fit
    if response.double_clicked() {
        if let Some(grid) = &app.voxel_grid {
            if let Some((min, max)) = grid.bounds_voxel() {
                app.camera
                    .fit_to_bounds(min.as_vec3(), (max + IVec3::ONE).as_vec3());
            }
        } else if let Some(scene) = &app.scene {
            if let Some((min, max)) = scene.bounds() {
                app.camera.fit_to_bounds(min, max);
            }
        }
    }

    // Empty viewport message
    if app.voxel_grid.is_none() && app.scene.is_none() {
        let center = rect.center();
        ui.painter().text(
            center,
            Align2::CENTER_CENTER,
            "Import a model to get started",
            FontId::proportional(16.0),
            TEXT_DIM,
        );
        return;
    }

    let show_mesh_preview = app.view_mode == ViewMode::Mesh || app.voxel_grid.is_none();
    if show_mesh_preview {
        if let Some(scene) = &app.scene {
            draw_mesh_preview(ui, rect, &app.camera, scene);
        }
    }

    // wgpu render callback
    let vp_size = [rect.width() as u32, rect.height() as u32];
    if vp_size[0] == 0 || vp_size[1] == 0 {
        return;
    }

    let camera = app.camera.clone();

    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
        rect,
        ViewportCallback {
            camera,
            viewport_size: vp_size,
            has_content: app.voxel_grid.is_some() && !show_mesh_preview,
        },
    ));

    if app.show_chunk_boundaries {
        if let Some(grid) = &app.voxel_grid {
            draw_chunk_boundaries(ui, rect, &app.camera, grid);
        }
    }

    // Overlay: coordinates / help text
    let pos = rect.left_top() + Vec2::new(8.0, 8.0);
    if let Some(grid) = &app.voxel_grid {
        ui.painter().text(
            pos,
            Align2::LEFT_TOP,
            format!("{} blocks • {:.1}b/u", grid.total_voxels(), grid.resolution),
            FontId::monospace(11.0),
            Color32::from_rgba_premultiplied(180, 220, 180, 200),
        );
    }
}

fn draw_mesh_preview(
    ui: &mut Ui,
    rect: Rect,
    camera: &OrbitCamera,
    scene: &crate::core::scene::Scene,
) {
    let painter = ui.painter();
    let stroke = Stroke::new(1.0, Color32::from_rgba_premultiplied(120, 220, 150, 150));
    let mut drawn = 0usize;
    const MAX_TRIANGLES: usize = 20_000;

    for mesh in &scene.meshes {
        for tri in 0..mesh.triangle_count() {
            if drawn >= MAX_TRIANGLES {
                return;
            }
            let [a, b, c] = mesh.triangle(tri);
            draw_projected_segment(painter, rect, camera, a, b, stroke);
            draw_projected_segment(painter, rect, camera, b, c, stroke);
            draw_projected_segment(painter, rect, camera, c, a, stroke);
            drawn += 1;
        }
    }
}

fn draw_chunk_boundaries(
    ui: &mut Ui,
    rect: Rect,
    camera: &OrbitCamera,
    grid: &crate::core::voxel::VoxelGrid,
) {
    let Some((min, max)) = grid.bounds_voxel() else {
        return;
    };
    let painter = ui.painter();
    let stroke = Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 210, 80, 100));
    let y0 = min.y as f32;
    let y1 = (max.y + 1) as f32;
    let x_start = min.x.div_euclid(16) * 16;
    let x_end = (max.x + 15).div_euclid(16) * 16;
    let z_start = min.z.div_euclid(16) * 16;
    let z_end = (max.z + 15).div_euclid(16) * 16;

    for x in (x_start..=x_end).step_by(16) {
        let x = x as f32;
        draw_projected_segment(
            painter,
            rect,
            camera,
            Vec3::new(x, y0, z_start as f32),
            Vec3::new(x, y0, z_end as f32),
            stroke,
        );
        draw_projected_segment(
            painter,
            rect,
            camera,
            Vec3::new(x, y1, z_start as f32),
            Vec3::new(x, y1, z_end as f32),
            stroke,
        );
        draw_projected_segment(
            painter,
            rect,
            camera,
            Vec3::new(x, y0, z_start as f32),
            Vec3::new(x, y1, z_start as f32),
            stroke,
        );
        draw_projected_segment(
            painter,
            rect,
            camera,
            Vec3::new(x, y0, z_end as f32),
            Vec3::new(x, y1, z_end as f32),
            stroke,
        );
    }

    for z in (z_start..=z_end).step_by(16) {
        let z = z as f32;
        draw_projected_segment(
            painter,
            rect,
            camera,
            Vec3::new(x_start as f32, y0, z),
            Vec3::new(x_end as f32, y0, z),
            stroke,
        );
        draw_projected_segment(
            painter,
            rect,
            camera,
            Vec3::new(x_start as f32, y1, z),
            Vec3::new(x_end as f32, y1, z),
            stroke,
        );
        draw_projected_segment(
            painter,
            rect,
            camera,
            Vec3::new(x_start as f32, y0, z),
            Vec3::new(x_start as f32, y1, z),
            stroke,
        );
        draw_projected_segment(
            painter,
            rect,
            camera,
            Vec3::new(x_end as f32, y0, z),
            Vec3::new(x_end as f32, y1, z),
            stroke,
        );
    }
}

fn draw_projected_segment(
    painter: &Painter,
    rect: Rect,
    camera: &OrbitCamera,
    a: Vec3,
    b: Vec3,
    stroke: Stroke,
) {
    let Some(pa) = project_world(rect, camera, a) else {
        return;
    };
    let Some(pb) = project_world(rect, camera, b) else {
        return;
    };
    painter.line_segment([pa, pb], stroke);
}

fn project_world(rect: Rect, camera: &OrbitCamera, point: Vec3) -> Option<Pos2> {
    let clip = camera.view_proj() * point.extend(1.0);
    if clip.w <= 0.0 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if ndc.z < -1.0 || ndc.z > 1.0 {
        return None;
    }
    Some(Pos2::new(
        rect.left() + (ndc.x + 1.0) * 0.5 * rect.width(),
        rect.top() + (1.0 - (ndc.y + 1.0) * 0.5) * rect.height(),
    ))
}

// ─── wgpu paint callback ─────────────────────────────────────────────────────

struct ViewportCallback {
    camera: OrbitCamera,
    viewport_size: [u32; 2],
    has_content: bool,
}

impl egui_wgpu::CallbackTrait for ViewportCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(renderer) = resources.get_mut::<VoxelRenderer>() {
            renderer.ensure_offscreen(device, self.viewport_size);
            renderer.update_uniforms(queue, &self.camera);
            // Render voxels into the offscreen texture (with depth) before
            // egui's main render pass opens. blit() will copy it in paint().
            if self.has_content {
                renderer.render_to_offscreen(encoder);
            }
        }
        vec![]
    }

    fn paint<'a>(
        &'a self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'a>,
        resources: &'a egui_wgpu::CallbackResources,
    ) {
        if !self.has_content {
            return;
        }
        if let Some(renderer) = resources.get::<VoxelRenderer>() {
            renderer.blit(render_pass);
        }
    }
}

// ─── Status bar ──────────────────────────────────────────────────────────────

fn draw_status_bar(app: &mut ChunkyApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        if app.is_working {
            ui.spinner();
            let bar_width = 120.0;
            let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(bar_width, 12.0), Sense::hover());
            ui.painter()
                .rect_filled(bar_rect, 2.0, Color32::from_rgb(40, 40, 55));
            let fill_rect = Rect::from_min_size(
                bar_rect.min,
                Vec2::new(bar_width * app.work_progress, bar_rect.height()),
            );
            ui.painter().rect_filled(fill_rect, 2.0, ACCENT);
            ui.label(RichText::new(&app.work_label).color(TEXT_DIM).small());
        } else {
            let color = if app.status_message.starts_with("Error") {
                ERR
            } else if app.status_message.starts_with("Loaded")
                || app.status_message.contains("blocks")
            {
                ACCENT
            } else {
                TEXT_DIM
            };
            ui.label(RichText::new(&app.status_message).color(color).small());
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new("Chunky v0.1.0").color(TEXT_DIM).small());
        });
    });
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn section_header(ui: &mut Ui, text: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, 0.0, Color32::from_rgb(26, 26, 38));
    ui.painter().text(
        rect.left_center() + Vec2::new(12.0, 0.0),
        Align2::LEFT_CENTER,
        text,
        FontId::monospace(11.0),
        TEXT_DIM,
    );
}
