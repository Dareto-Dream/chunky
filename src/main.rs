mod app;
mod core;
mod renderer;
mod ui;

fn main() -> eframe::Result<()> {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Chunky — 3D to Minecraft Converter")
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([900.0, 600.0])
            .with_icon(eframe::icon_data::from_png_bytes(&[]).unwrap_or_default()),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            supported_backends: wgpu::Backends::all(),
            device_descriptor: std::sync::Arc::new(|_adapter| wgpu::DeviceDescriptor {
                label: Some("chunky"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    eframe::run_native(
        "Chunky",
        options,
        Box::new(|cc| Ok(Box::new(app::ChunkyApp::new(cc)))),
    )
}
