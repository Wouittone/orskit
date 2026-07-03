//! Experimental Python bindings for typed orskit spacecraft states.

use orskit_core::{Epoch, SpacecraftState};
use orskit_frames::ReferenceFrame;
use orskit_units::uom::si::mass::kilogram;
use orskit_units::{Mass, Position, VelocityVector};
use pyo3::{exceptions::PyValueError, prelude::*};

/// Python-facing spacecraft state.
#[pyclass(name = "SpacecraftState")]
pub struct SpacecraftStateWrapper {
    state: SpacecraftState,
}

#[pymethods]
impl SpacecraftStateWrapper {
    #[new]
    #[pyo3(signature = (
        x_m,
        y_m,
        z_m,
        vx_m_s,
        vy_m_s,
        vz_m_s,
        mass_kg,
        epoch_tai_seconds,
        frame = "GCRF"
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        x_m: f64,
        y_m: f64,
        z_m: f64,
        vx_m_s: f64,
        vy_m_s: f64,
        vz_m_s: f64,
        mass_kg: f64,
        epoch_tai_seconds: f64,
        frame: &str,
    ) -> PyResult<Self> {
        if !epoch_tai_seconds.is_finite() {
            return Err(PyValueError::new_err("epoch_tai_seconds must be finite"));
        }
        let frame = frame
            .parse::<ReferenceFrame>()
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let state = SpacecraftState::new(
            Epoch::from_tai_seconds(epoch_tai_seconds),
            frame,
            Position::from_metres(x_m, y_m, z_m),
            VelocityVector::from_metres_per_second(vx_m_s, vy_m_s, vz_m_s),
            Mass::new::<kilogram>(mass_kg),
        )
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
        Ok(Self { state })
    }

    /// Position in metres, in the state's reference frame.
    fn position_m(&self) -> (f64, f64, f64) {
        let [x, y, z] = self.state.position().to_metres();
        (x, y, z)
    }

    /// Velocity in metres per second, in the state's reference frame.
    fn velocity_m_s(&self) -> (f64, f64, f64) {
        let [x, y, z] = self.state.velocity().to_metres_per_second();
        (x, y, z)
    }

    /// Spacecraft mass in kilograms.
    fn mass_kg(&self) -> f64 {
        self.state.mass().get::<kilogram>()
    }

    /// Reference-frame name.
    fn frame(&self) -> String {
        self.state.frame().to_string()
    }

    /// Epoch as TAI seconds since Hifitime's reference epoch.
    fn epoch_tai_seconds(&self) -> f64 {
        self.state.epoch().to_tai_seconds()
    }

    fn __repr__(&self) -> String {
        format!(
            "SpacecraftState(position_m={:?}, velocity_m_s={:?}, mass_kg={}, frame='{}')",
            self.state.position().to_metres(),
            self.state.velocity().to_metres_per_second(),
            self.state.mass().get::<kilogram>(),
            self.state.frame(),
        )
    }
}

#[pymodule]
fn orskit_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<SpacecraftStateWrapper>()?;
    Ok(())
}
