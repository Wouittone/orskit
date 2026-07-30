use std::fmt;

use frames::ReferenceFrame;
use nalgebra::{Cholesky, Matrix3, SMatrix};
use orbits::cartesian::CartesianState;
use orskit_core::{Orbit, SpacecraftState};
use units::uom::si::area::square_meter;
use units::{Area, Position, PositionVelocityCovariance, VelocityVariance};

use crate::{numerical::RawCovariance, OrbitDeterminationError};

pub use orbits::cartesian::CartesianCovariance;

/// A covariance representation associated with one spacecraft-state type.
pub trait StateCovariance<S: SpacecraftState>: fmt::Debug + Clone + Send + Sync {
    /// Frame in which the covariance is expressed.
    fn frame(&self) -> ReferenceFrame;
}

/// Epoch-qualified estimate in its selected domain representation.
#[derive(Debug, Clone, PartialEq)]
pub struct StateEstimate<S: SpacecraftState, C: StateCovariance<S>> {
    orbit: Orbit<S>,
    covariance: C,
}

impl<S: SpacecraftState, C: StateCovariance<S>> StateEstimate<S, C> {
    /// Creates an estimate when state and covariance use the same frame.
    pub fn new(orbit: Orbit<S>, covariance: C) -> Result<Self, OrbitDeterminationError> {
        if orbit.as_ref().frame() != covariance.frame() {
            return Err(OrbitDeterminationError::FrameMismatch);
        }
        Ok(Self { orbit, covariance })
    }

    /// Returns the epoch-qualified state estimate.
    #[must_use]
    pub const fn orbit(&self) -> &Orbit<S> {
        &self.orbit
    }

    /// Returns the state-domain covariance.
    #[must_use]
    pub const fn covariance(&self) -> &C {
        &self.covariance
    }
}

impl StateCovariance<CartesianState> for CartesianCovariance {
    fn frame(&self) -> ReferenceFrame {
        CartesianCovariance::frame(self)
    }
}

pub(crate) fn cartesian_covariance_raw(covariance: &CartesianCovariance) -> RawCovariance {
    RawCovariance::from_fn(|row, column| match (row < 3, column < 3) {
        (true, true) => covariance.position_position()[row][column].get::<square_meter>(),
        (true, false) => {
            covariance.position_velocity()[row][column - 3].as_square_metres_per_second()
        }
        (false, true) => {
            covariance.position_velocity()[column][row - 3].as_square_metres_per_second()
        }
        (false, false) => {
            covariance.velocity_velocity()[row - 3][column - 3].as_square_metres_per_square_second()
        }
    })
}

pub(crate) fn cartesian_covariance_from_raw(
    frame: ReferenceFrame,
    entries: RawCovariance,
) -> Result<CartesianCovariance, OrbitDeterminationError> {
    validate_positive_definite(&entries, "Cartesian covariance")?;
    let entries = symmetrize(entries);
    let position_position = std::array::from_fn(|row| {
        std::array::from_fn(|column| Area::new::<square_meter>(entries[(row, column)]))
    });
    let position_velocity = std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            PositionVelocityCovariance::from_square_metres_per_second(entries[(row, column + 3)])
        })
    });
    let velocity_velocity = std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            VelocityVariance::from_square_metres_per_square_second(entries[(row + 3, column + 3)])
        })
    });
    CartesianCovariance::from_blocks(
        frame,
        position_position,
        position_velocity,
        velocity_velocity,
    )
    .map_err(OrbitDeterminationError::CartesianCovariance)
}

/// Cartesian estimate used by the supplied Kalman implementations.
pub type CartesianStateEstimate = StateEstimate<CartesianState, CartesianCovariance>;

/// Position-only covariance in square metres.
#[derive(Debug, Clone, PartialEq)]
pub struct PositionCovariance {
    frame: ReferenceFrame,
    entries: Matrix3<f64>,
}

impl PositionCovariance {
    /// Creates a diagonal covariance from typed position standard deviations.
    pub fn from_standard_deviations(
        frame: ReferenceFrame,
        standard_deviation: Position,
    ) -> Result<Self, OrbitDeterminationError> {
        let values = standard_deviation.to_metres();
        if values.iter().any(|value| !value.is_finite()) {
            return Err(OrbitDeterminationError::NonFinite {
                what: "position standard deviation",
            });
        }
        if values.iter().any(|value| *value <= 0.0) {
            return Err(OrbitDeterminationError::NotPositiveDefinite {
                what: "position standard deviation",
            });
        }
        Self::from_raw(
            frame,
            Matrix3::from_diagonal(&nalgebra::Vector3::new(
                values[0] * values[0],
                values[1] * values[1],
                values[2] * values[2],
            )),
        )
    }

    /// Returns the expression frame.
    #[must_use]
    pub const fn frame(&self) -> ReferenceFrame {
        self.frame
    }

    pub(crate) fn raw(&self) -> Matrix3<f64> {
        self.entries
    }

    pub(crate) fn from_raw(
        frame: ReferenceFrame,
        entries: Matrix3<f64>,
    ) -> Result<Self, OrbitDeterminationError> {
        validate_positive_definite(&entries, "position covariance")?;
        Ok(Self {
            frame,
            entries: symmetrize(entries),
        })
    }
}

pub(crate) fn validate_positive_definite<const N: usize>(
    matrix: &SMatrix<f64, N, N>,
    what: &'static str,
) -> Result<(), OrbitDeterminationError> {
    if matrix.iter().any(|value| !value.is_finite()) {
        return Err(OrbitDeterminationError::NonFinite { what });
    }
    let scale = matrix
        .iter()
        .fold(1.0_f64, |maximum, value| maximum.max(value.abs()));
    if !(matrix - matrix.transpose())
        .iter()
        .all(|value| value.abs() <= 32.0 * f64::EPSILON * scale)
    {
        return Err(OrbitDeterminationError::NotSymmetric { what });
    }
    if Cholesky::new(symmetrize(*matrix)).is_none() {
        return Err(OrbitDeterminationError::NotPositiveDefinite { what });
    }
    Ok(())
}

pub(crate) fn symmetrize<const N: usize>(matrix: SMatrix<f64, N, N>) -> SMatrix<f64, N, N> {
    (matrix + matrix.transpose()) * 0.5
}
