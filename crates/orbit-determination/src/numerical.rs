use std::sync::{Arc, Mutex};

use dynamics::Propagator;
use finitediff::array::central_jacobian;
use frames::ReferenceFrame;
use hifitime::Epoch;
use nalgebra::{Matrix3, SMatrix, SVector};
use orbits::cartesian::CartesianState;
use orskit_core::Orbit;
use units::{Position, VelocityVector};

use crate::OrbitDeterminationError;

pub(crate) type RawState = SVector<f64, 6>;
pub(crate) type RawCovariance = SMatrix<f64, 6, 6>;
pub(crate) type RawPosition = SVector<f64, 3>;

const POSITION_SCALE_METRES: f64 = 10_000_000.0;
const VELOCITY_SCALE_METRES_PER_SECOND: f64 = 10_000.0;

pub(crate) fn raw_state(state: &CartesianState) -> RawState {
    let position = state.position().to_metres();
    let velocity = state.velocity().to_metres_per_second();
    RawState::from_row_slice(&[
        position[0],
        position[1],
        position[2],
        velocity[0],
        velocity[1],
        velocity[2],
    ])
}

pub(crate) fn raw_position(position: Position) -> RawPosition {
    RawPosition::from_row_slice(&position.to_metres())
}

pub(crate) fn position_from_raw(value: RawPosition) -> Position {
    Position::from_metres(value[0], value[1], value[2])
}

pub(crate) fn orbit_from_raw(
    epoch: Epoch,
    frame: ReferenceFrame,
    state: RawState,
) -> Result<Orbit<CartesianState>, OrbitDeterminationError> {
    let state = CartesianState::new(
        frame,
        Position::from_metres(state[0], state[1], state[2]),
        VelocityVector::from_metres_per_second(state[3], state[4], state[5]),
    )
    .map_err(OrbitDeterminationError::InvalidCartesianState)?;
    Ok(Orbit::new(epoch, state))
}

pub(crate) fn position_jacobian() -> SMatrix<f64, 3, 6> {
    let mut jacobian = SMatrix::<f64, 3, 6>::zeros();
    jacobian
        .fixed_view_mut::<3, 3>(0, 0)
        .copy_from(&Matrix3::identity());
    jacobian
}

/// Derives the Cartesian transition matrix through `finitediff`'s central
/// Jacobian. Coordinates are scaled before differentiation, avoiding an
/// algorithm-specific hand-written perturbation policy.
pub(crate) fn propagate_with_transition<P>(
    propagator: &P,
    initial: Orbit<CartesianState>,
    target: Epoch,
) -> Result<(Orbit<CartesianState>, RawCovariance), OrbitDeterminationError>
where
    P: Propagator<CartesianState>,
{
    let frame = initial.as_ref().frame();
    let epoch = initial.epoch();
    let initial_raw = raw_state(initial.as_ref());
    let propagated = propagator
        .propagate(initial, target)
        .map_err(OrbitDeterminationError::propagation)?;
    let failure = Arc::new(Mutex::new(None));
    let function_failure = Arc::clone(&failure);
    let transition_function = |normalized: &[f64; 6]| {
        let raw = RawState::from_fn(|index, _| normalized[index] * state_scale(index));
        let result = orbit_from_raw(epoch, frame, raw)
            .and_then(|orbit| {
                propagator
                    .propagate(orbit, target)
                    .map_err(OrbitDeterminationError::propagation)
            })
            .map(|orbit| {
                std::array::from_fn(|index| raw_state(orbit.as_ref())[index] / state_scale(index))
            });
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                *function_failure
                    .lock()
                    .expect("transition failure mutex is not poisoned") = Some(error);
                Ok([0.0; 6])
            }
        }
    };
    let normalized = std::array::from_fn(|index| initial_raw[index] / state_scale(index));
    let jacobian = central_jacobian(&transition_function)(&normalized)
        .expect("transition function converts failures into an explicit result");
    if let Some(error) = failure
        .lock()
        .expect("transition failure mutex is not poisoned")
        .take()
    {
        return Err(error);
    }
    let transition = RawCovariance::from_fn(|row, column| {
        jacobian[row][column] * state_scale(row) / state_scale(column)
    });
    Ok((propagated, transition))
}

fn state_scale(index: usize) -> f64 {
    if index < 3 {
        POSITION_SCALE_METRES
    } else {
        VELOCITY_SCALE_METRES_PER_SECOND
    }
}
