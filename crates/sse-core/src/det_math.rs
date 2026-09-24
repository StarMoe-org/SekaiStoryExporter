//! Cross-platform bit-identical math (determinism rule D-4).
//!
//! Everything here is backed by the pure-Rust `libm` crate; `std`'s transcendental
//! functions call the platform libm and differ between macOS and Windows.

pub fn sinf(x: f32) -> f32 {
    libm::sinf(x)
}

pub fn cosf(x: f32) -> f32 {
    libm::cosf(x)
}

pub fn powf(x: f32, y: f32) -> f32 {
    libm::powf(x, y)
}

pub fn expf(x: f32) -> f32 {
    libm::expf(x)
}

pub fn atan2f(y: f32, x: f32) -> f32 {
    libm::atan2f(y, x)
}

pub fn sqrtf(x: f32) -> f32 {
    libm::sqrtf(x)
}

pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}

pub fn cos(x: f64) -> f64 {
    libm::cos(x)
}

pub fn pow(x: f64, y: f64) -> f64 {
    libm::pow(x, y)
}

/// `Mathf.Clamp01`.
pub fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

/// `Mathf.Lerp` (clamped).
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * clamp01(t)
}
