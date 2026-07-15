//! Experimental Python bindings for typed orskit spacecraft states.

use core_crate::{
    AttitudeState, CartesianState, Epoch, FramedAngularVelocity, InertiaTensor, Orientation,
};
use frames::{CustomFrameId, FrameMotion, FrameOrientation, FrameOrigin, ReferenceFrame};
use units::uom::si::{
    mass::kilogram, moment_of_inertia::kilogram_square_meter, ratio::ratio,
};
use units::{AngularVelocityVector, Mass, MomentOfInertia, Position, Ratio, VelocityVector};
use pyo3::{exceptions::PyValueError, prelude::*};

/// Python-facing spacecraft state.
#[pyclass(name = "SpacecraftState")]
pub struct SpacecraftStateWrapper {
    state: CartesianState,
    epoch: Epoch,
    mass: Mass,
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
        angular_velocity_rad_s,
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
        angular_velocity_rad_s: (f64, f64, f64),
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
        let state = CartesianState::new(
            frame,
            Position::from_metres(x_m, y_m, z_m),
            VelocityVector::from_metres_per_second(vx_m_s, vy_m_s, vz_m_s),
        )
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let body_id = CustomFrameId::new(body_frame_id);
        let body_frame = ReferenceFrame::new(
            FrameOrigin::Custom(body_id),
            FrameOrientation::custom(body_id, FrameMotion::NonInertial),
        );
        let orientation = Orientation::try_from((
            body_frame,
            frame,
            [
                Ratio::new::<ratio>(orientation_wxyz.0),
                Ratio::new::<ratio>(orientation_wxyz.1),
                Ratio::new::<ratio>(orientation_wxyz.2),
                Ratio::new::<ratio>(orientation_wxyz.3),
            ],
        ))
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let attitude = AttitudeState::new(
            orientation,
            FramedAngularVelocity::new(
                AngularVelocityVector::from_radians_per_second(
                    angular_velocity_rad_s.0,
                    angular_velocity_rad_s.1,
                    angular_velocity_rad_s.2,
                ),
                body_frame,
            )
            .map_err(|error| PyValueError::new_err(error.to_string()))?,
        )
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let inertia = InertiaTensor::principal(
            body_frame,
            MomentOfInertia::new::<kilogram_square_meter>(principal_inertia_kg_m2.0),
            MomentOfInertia::new::<kilogram_square_meter>(principal_inertia_kg_m2.1),
            MomentOfInertia::new::<kilogram_square_meter>(principal_inertia_kg_m2.2),
        )
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let mass = Mass::new::<kilogram>(mass_kg);
        if !mass_kg.is_finite() || mass_kg <= 0.0 {
            return Err(PyValueError::new_err("mass_kg must be positive and finite"));
        }
        // The experimental binding does not yet expose the core spacecraft/view
        // split; construction still validates all supplied rigid-body values.
        let _validated_rigid_body = (inertia, attitude);
        Ok(Self {
            state,
            epoch: Epoch::from_tai_seconds(epoch_tai_seconds),
            mass,
        })
    }

    /// Position in metres, in the state's reference frame.
    fn position_m(&self) -> (f64, f64, f64) {
        let [x, y, z] = self.cartesian().position().to_metres();
        (x, y, z)
    }

    /// Velocity in metres per second, in the state's reference frame.
    fn velocity_m_s(&self) -> (f64, f64, f64) {
        let [x, y, z] = self.cartesian().velocity().to_metres_per_second();
        (x, y, z)
    }

    /// Spacecraft mass in kilograms.
    fn mass_kg(&self) -> f64 {
        self.mass.get::<kilogram>()
    }

    /// Reference-frame name.
    fn frame(&self) -> String {
        self.cartesian().frame().to_string()
    }

    /// Epoch as TAI seconds since Hifitime's reference epoch.
    fn epoch_tai_seconds(&self) -> f64 {
        self.epoch.to_tai_seconds()
    }

    fn __repr__(&self) -> String {
        format!(
            "SpacecraftState(position_m={:?}, velocity_m_s={:?}, mass_kg={}, frame='{}')",
            self.cartesian().position().to_metres(),
            self.cartesian().velocity().to_metres_per_second(),
            self.mass.get::<kilogram>(),
            self.cartesian().frame(),
        )
    }
}

impl SpacecraftStateWrapper {
    fn cartesian(&self) -> CartesianState {
        self.state
    }
}

#[pymodule]
fn py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<SpacecraftStateWrapper>()?;
    Ok(())
}
