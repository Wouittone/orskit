//! Orbital mechanics core structures

use nalgebra::Vector6;

/// Represents an orbital state vector (position and velocity)
///
/// The state is represented as a 6-element vector:
/// [x, y, z, vx, vy, vz] in meters and meters/second
#[derive(Debug, Clone, Copy)]
pub struct OrbitalState {
    /// Position and velocity vector (6D)
    pub state: Vector6<f64>,
}

impl OrbitalState {
    /// Create a new orbital state from position and velocity
    pub fn new(x: f64, y: f64, z: f64, vx: f64, vy: f64, vz: f64) -> Self {
        Self {
            state: Vector6::new(x, y, z, vx, vy, vz),
        }
    }

    /// Get position vector
    pub fn position(&self) -> [f64; 3] {
        [self.state[0], self.state[1], self.state[2]]
    }

    /// Get velocity vector
    pub fn velocity(&self) -> [f64; 3] {
        [self.state[3], self.state[4], self.state[5]]
    }
}
