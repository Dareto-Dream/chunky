use glam::{Mat4, Vec2, Vec3};

#[derive(Debug, Clone)]
pub struct OrbitCamera {
    pub target: Vec3,
    pub distance: f32,
    pub yaw: f32,   // radians, horizontal rotation
    pub pitch: f32, // radians, vertical rotation
    pub fov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            distance: 5.0,
            yaw: 0.8,
            pitch: 0.5,
            fov: 60.0f32.to_radians(),
            aspect: 1.0,
            near: 0.01,
            far: 10000.0,
        }
    }
}

impl OrbitCamera {
    pub fn position(&self) -> Vec3 {
        let x = self.distance * self.pitch.cos() * self.yaw.sin();
        let y = self.distance * self.pitch.sin();
        let z = self.distance * self.pitch.cos() * self.yaw.cos();
        self.target + Vec3::new(x, y, z)
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.position(), self.target, Vec3::Y)
    }

    pub fn proj_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov, self.aspect, self.near, self.far)
    }

    pub fn view_proj(&self) -> Mat4 {
        self.proj_matrix() * self.view_matrix()
    }

    pub fn orbit(&mut self, delta: Vec2) {
        self.yaw -= delta.x * 0.01;
        self.pitch = (self.pitch - delta.y * 0.01).clamp(-1.5, 1.5);
    }

    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance * (1.0 - delta * 0.001)).clamp(0.1, 10000.0);
    }

    pub fn pan(&mut self, delta: Vec2) {
        let right = self.view_matrix().col(0).truncate();
        let up = self.view_matrix().col(1).truncate();
        let scale = self.distance * 0.001;
        self.target -= right * delta.x * scale;
        self.target += up * delta.y * scale;
    }

    pub fn fit_to_bounds(&mut self, min: Vec3, max: Vec3) {
        self.target = (min + max) * 0.5;
        let size = (max - min).length();
        self.distance = size * 1.5;
        self.near = size * 0.001;
        self.far = size * 100.0;
    }
}
