#[derive(Debug)]
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub diameter: f32,
}

impl Particle {
    pub fn new(x: f32) -> Self {
        Self {
            x,
            y: rand::random::<f32>()*2.0-1.0,
            r: rand::random::<f32>(),
            g: rand::random::<f32>(),
            b: rand::random::<f32>(),
            diameter: 0.0,
        }
    }
}