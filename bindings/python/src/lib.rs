//! Python bindings for orskit using PyO3

use pyo3::prelude::*;
use core::orbit::OrbitalState;

#[pyclass]
pub struct OrbitalStateWrapper {
    state: OrbitalState,
}

#[pymethods]
impl OrbitalStateWrapper {
    #[new]
    fn new(x: f64, y: f64, z: f64, vx: f64, vy: f64, vz: f64) -> Self {
        Self {
            state: OrbitalState::new(x, y, z, vx, vy, vz),
        }
    }

    fn position(&self) -> (f64, f64, f64) {
        let pos = self.state.position();
        (pos[0], pos[1], pos[2])
    }

    fn velocity(&self) -> (f64, f64, f64) {
        let vel = self.state.velocity();
        (vel[0], vel[1], vel[2])
    }

    fn __repr__(&self) -> String {
        format!(
            "OrbitalState(pos={:?}, vel={:?})",
            self.state.position(),
            self.state.velocity()
        )
    }
}

#[pymodule]
fn orskit_py(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<OrbitalStateWrapper>()?;
    Ok(())
}
