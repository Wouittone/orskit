//! Orbital propagation using numerical integration

use core::orbit::OrbitalState;
use nalgebra::Vector6;

/// Two-body gravitational dynamics
pub struct TwoBodyDynamics {
    /// Standard gravitational parameter (mu = GM) in m^3/s^2
    pub mu: f64,
}

impl TwoBodyDynamics {
    /// Create a new two-body dynamics model
    /// Default mu is Earth's value: 3.986004418e14 m^3/s^2
    pub fn new() -> Self {
        Self {
            mu: 3.986004418e14,
        }
    }

    /// Create with custom gravitational parameter
    pub fn with_mu(mu: f64) -> Self {
        Self { mu }
    }

    /// Two-body equations of motion
    /// dr/dt = v
    /// dv/dt = -mu/r^3 * r
    pub fn dynamics(&self, state: &OrbitalState) -> Vector6<f64> {
        let r = state.state.fixed_rows::<3>(0);
        let v = state.state.fixed_rows::<3>(3);
        let r_mag = r.norm();
        let r_mag_cubed = r_mag * r_mag * r_mag;

        let mut derivatives = Vector6::zeros();
        derivatives.fixed_rows_mut::<3>(0).copy_from(&v);
        derivatives.fixed_rows_mut::<3>(3).copy_from(&(-self.mu / r_mag_cubed * r));

        derivatives
    }
}

impl Default for TwoBodyDynamics {
    fn default() -> Self {
        Self::new()
    }
}
