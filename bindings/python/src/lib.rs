//! Experimental Python bindings for typed orskit spacecraft states.

use orskit_core::{
    CartesianCoordinates, CartesianState, CoordinateSample, Epoch, FramedPosition, FramedVelocity,
    InertiaTensor, Orientation, SpacecraftProperties, State,
};
use orskit_frames::{CustomFrameId, FrameOrientation, FrameOrigin, ReferenceFrame};
use orskit_units::uom::si::{
    mass::kilogram, moment_of_inertia::kilogram_square_meter, ratio::ratio,
};
use orskit_units::{Mass, MomentOfInertia, Position, Ratio, VelocityVector};
use pyo3::{exceptions::PyValueError, prelude::*};

/// Python-facing spacecraft state.
#[pyclass(name = "SpacecraftState")]
pub struct SpacecraftStateWrapper {
    state: CartesianState,
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
        orientation_wxyz,
        principal_inertia_kg_m2,
        body_frame_id,
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
        orientation_wxyz: (f64, f64, f64, f64),
        principal_inertia_kg_m2: (f64, f64, f64),
        body_frame_id: u64,
        frame: &str,
    ) -> PyResult<Self> {
        if !epoch_tai_seconds.is_finite() {
            return Err(PyValueError::new_err("epoch_tai_seconds must be finite"));
        }
        let frame = frame
            .parse::<ReferenceFrame>()
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let position = FramedPosition::new(Position::from_metres(x_m, y_m, z_m), frame)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let velocity = FramedVelocity::new(
            VelocityVector::from_metres_per_second(vx_m_s, vy_m_s, vz_m_s),
            frame,
        )
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let body_id = CustomFrameId::new(body_frame_id);
        let body_frame = ReferenceFrame::new(
            FrameOrigin::Custom(body_id),
            FrameOrientation::Custom(body_id),
        );
        let orientation = Orientation::from_quaternion(
            body_frame,
            frame,
            Ratio::new::<ratio>(orientation_wxyz.0),
            Ratio::new::<ratio>(orientation_wxyz.1),
            Ratio::new::<ratio>(orientation_wxyz.2),
            Ratio::new::<ratio>(orientation_wxyz.3),
        )
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let inertia = InertiaTensor::principal(
            body_frame,
            MomentOfInertia::new::<kilogram_square_meter>(principal_inertia_kg_m2.0),
            MomentOfInertia::new::<kilogram_square_meter>(principal_inertia_kg_m2.1),
            MomentOfInertia::new::<kilogram_square_meter>(principal_inertia_kg_m2.2),
        )
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let properties =
            SpacecraftProperties::new(Mass::new::<kilogram>(mass_kg), orientation, inertia)
                .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let coordinates = CartesianCoordinates::new(position, velocity);
        let sample = CoordinateSample::new(Epoch::from_tai_seconds(epoch_tai_seconds), coordinates);
        let state = CartesianState::new(sample, properties);
        Ok(Self { state })
    }

    /// Position in metres, in the state's reference frame.
    fn position_m(&self) -> (f64, f64, f64) {
        let [x, y, z] = self.state.position().value().to_metres();
        (x, y, z)
    }

    /// Velocity in metres per second, in the state's reference frame.
    fn velocity_m_s(&self) -> (f64, f64, f64) {
        let [x, y, z] = self.state.velocity().value().to_metres_per_second();
        (x, y, z)
    }

    /// Spacecraft mass in kilograms.
    fn mass_kg(&self) -> f64 {
        self.state.mass().get::<kilogram>()
    }

    /// Reference-frame name.
    fn frame(&self) -> String {
        self.state.position().frame().to_string()
    }

    /// Epoch as TAI seconds since Hifitime's reference epoch.
    fn epoch_tai_seconds(&self) -> f64 {
        self.state.epoch().to_tai_seconds()
    }

    fn __repr__(&self) -> String {
        format!(
            "SpacecraftState(position_m={:?}, velocity_m_s={:?}, mass_kg={}, frame='{}')",
            self.state.position().value().to_metres(),
            self.state.velocity().value().to_metres_per_second(),
            self.state.mass().get::<kilogram>(),
            self.state.position().frame(),
        )
    }
}

#[pymodule]
fn orskit_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<SpacecraftStateWrapper>()?;
    Ok(())
}
