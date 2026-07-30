//! Cartesian variational equations, state transition, and covariance mapping.
//!
//! The integrated equations `d Phi / dt = A Phi`, `Phi(t0) = I`, and the
//! covariance map `P = Phi P0 Phi^T` follow P. J. Huxel and R. H. Bishop,
//! [*Navigation Algorithms for Formation Flying
//! Missions*](https://ntrs.nasa.gov/citations/20060048534), Proceedings of the
//! 2nd International Symposium on Formation Flying Missions and Technologies,
//! 2004. Only the public equations were consulted.

use std::error::Error;

use dynamics::CartesianVariationalDynamics;
use frames::ReferenceFrame;
use hifitime::{Duration, Epoch};
use orbits::cartesian::{CartesianCovariance, CartesianCovarianceError, CartesianState};
use orskit_core::{Orbit, OrbitParts};
use thiserror::Error;
use units::uom::si::{area::square_meter, ratio::ratio, time::second};
use units::{Area, InverseTime, PositionVelocityCovariance, Ratio, Time, VelocityVariance};

use crate::{
    array_to_state, state_to_array, step_factor, BogackiShampine32, IntegrationConfiguration,
    NumericalPropagationError, COMPONENT_COUNT,
};

const STM_COMPONENT_COUNT: usize = 36;
const AUGMENTED_COMPONENT_COUNT: usize = COMPONENT_COUNT + STM_COMPONENT_COUNT;

/// Absolute tolerances for the four dimensionally distinct STM blocks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VariationalConfiguration {
    dimensionless_absolute_tolerance: Ratio,
    position_velocity_absolute_tolerance: Time,
    velocity_position_absolute_tolerance: InverseTime,
}

impl VariationalConfiguration {
    /// Creates explicit absolute tolerances for STM error control.
    ///
    /// The dimensionless tolerance applies to `d position / d position` and
    /// `d velocity / d velocity`; the other values apply to the seconds-valued
    /// and reciprocal-seconds-valued off-diagonal blocks. The propagator's
    /// existing relative tolerance is shared by state and STM components.
    pub fn new(
        dimensionless_absolute_tolerance: Ratio,
        position_velocity_absolute_tolerance: Time,
        velocity_position_absolute_tolerance: InverseTime,
    ) -> Result<Self, VariationalConfigurationError> {
        let dimensionless = dimensionless_absolute_tolerance.get::<ratio>();
        if !dimensionless.is_finite() || dimensionless <= 0.0 {
            return Err(VariationalConfigurationError::InvalidDimensionlessTolerance);
        }
        let position_velocity = position_velocity_absolute_tolerance.get::<second>();
        if !position_velocity.is_finite() || position_velocity <= 0.0 {
            return Err(VariationalConfigurationError::InvalidPositionVelocityTolerance);
        }
        let velocity_position = velocity_position_absolute_tolerance.as_per_second();
        if !velocity_position.is_finite() || velocity_position <= 0.0 {
            return Err(VariationalConfigurationError::InvalidVelocityPositionTolerance);
        }
        Ok(Self {
            dimensionless_absolute_tolerance,
            position_velocity_absolute_tolerance,
            velocity_position_absolute_tolerance,
        })
    }

    /// Returns the tolerance for both dimensionless diagonal blocks.
    #[must_use]
    pub const fn dimensionless_absolute_tolerance(self) -> Ratio {
        self.dimensionless_absolute_tolerance
    }

    /// Returns the tolerance for `d position / d velocity`.
    #[must_use]
    pub const fn position_velocity_absolute_tolerance(self) -> Time {
        self.position_velocity_absolute_tolerance
    }

    /// Returns the tolerance for `d velocity / d position`.
    #[must_use]
    pub const fn velocity_position_absolute_tolerance(self) -> InverseTime {
        self.velocity_position_absolute_tolerance
    }
}

/// Invalid STM error-control settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum VariationalConfigurationError {
    /// A dimensionless STM tolerance is not positive and finite.
    #[error("dimensionless STM tolerance must be positive and finite")]
    InvalidDimensionlessTolerance,
    /// The position-with-respect-to-velocity tolerance is invalid.
    #[error("position/velocity STM tolerance must be positive and finite")]
    InvalidPositionVelocityTolerance,
    /// The velocity-with-respect-to-position tolerance is invalid.
    #[error("velocity/position STM tolerance must be positive and finite")]
    InvalidVelocityPositionTolerance,
}

/// Unit-qualified Cartesian state transition matrix.
///
/// Rows describe final Cartesian components and columns describe initial
/// components, both in `[x, y, z, vx, vy, vz]` order.
#[derive(Debug, Clone, PartialEq)]
pub struct CartesianStateTransition {
    initial_epoch: Epoch,
    final_epoch: Epoch,
    frame: ReferenceFrame,
    position_position: [[Ratio; 3]; 3],
    position_velocity: [[Time; 3]; 3],
    velocity_position: [[InverseTime; 3]; 3],
    velocity_velocity: [[Ratio; 3]; 3],
}

impl CartesianStateTransition {
    fn from_raw(
        initial_epoch: Epoch,
        final_epoch: Epoch,
        frame: ReferenceFrame,
        raw: [[f64; 6]; 6],
    ) -> Self {
        Self {
            initial_epoch,
            final_epoch,
            frame,
            position_position: std::array::from_fn(|row| {
                std::array::from_fn(|column| Ratio::new::<ratio>(raw[row][column]))
            }),
            position_velocity: std::array::from_fn(|row| {
                std::array::from_fn(|column| Time::new::<second>(raw[row][column + 3]))
            }),
            velocity_position: std::array::from_fn(|row| {
                std::array::from_fn(|column| InverseTime::from_per_second(raw[row + 3][column]))
            }),
            velocity_velocity: std::array::from_fn(|row| {
                std::array::from_fn(|column| Ratio::new::<ratio>(raw[row + 3][column + 3]))
            }),
        }
    }

    /// Returns the initial epoch with respect to which derivatives are taken.
    #[must_use]
    pub const fn initial_epoch(&self) -> Epoch {
        self.initial_epoch
    }

    /// Returns the final mapped epoch.
    #[must_use]
    pub const fn final_epoch(&self) -> Epoch {
        self.final_epoch
    }

    /// Returns the common Cartesian expression frame.
    #[must_use]
    pub const fn frame(&self) -> ReferenceFrame {
        self.frame
    }

    /// Returns `d final_position / d initial_position`.
    #[must_use]
    pub const fn position_position(&self) -> &[[Ratio; 3]; 3] {
        &self.position_position
    }

    /// Returns `d final_position / d initial_velocity` in seconds.
    #[must_use]
    pub const fn position_velocity(&self) -> &[[Time; 3]; 3] {
        &self.position_velocity
    }

    /// Returns `d final_velocity / d initial_position` in reciprocal seconds.
    #[must_use]
    pub const fn velocity_position(&self) -> &[[InverseTime; 3]; 3] {
        &self.velocity_position
    }

    /// Returns `d final_velocity / d initial_velocity`.
    #[must_use]
    pub const fn velocity_velocity(&self) -> &[[Ratio; 3]; 3] {
        &self.velocity_velocity
    }

    fn raw(&self) -> [[f64; 6]; 6] {
        std::array::from_fn(|row| {
            std::array::from_fn(|column| match (row < 3, column < 3) {
                (true, true) => self.position_position[row][column].get::<ratio>(),
                (true, false) => self.position_velocity[row][column - 3].get::<second>(),
                (false, true) => self.velocity_position[row - 3][column].as_per_second(),
                (false, false) => self.velocity_velocity[row - 3][column - 3].get::<ratio>(),
            })
        })
    }
}

/// Final Cartesian orbit and its state transition from the initial epoch.
#[derive(Debug, Clone, PartialEq)]
pub struct VariationalPropagation {
    final_orbit: Orbit<CartesianState>,
    state_transition: CartesianStateTransition,
}

impl VariationalPropagation {
    /// Returns the propagated orbit.
    #[must_use]
    pub const fn final_orbit(&self) -> &Orbit<CartesianState> {
        &self.final_orbit
    }

    /// Returns the integrated state transition matrix.
    #[must_use]
    pub const fn state_transition(&self) -> &CartesianStateTransition {
        &self.state_transition
    }

    /// Consumes the result into orbit and STM.
    #[must_use]
    pub fn into_parts(self) -> (Orbit<CartesianState>, CartesianStateTransition) {
        (self.final_orbit, self.state_transition)
    }
}

/// Final orbit, STM, and mapped covariance without additive process noise.
#[derive(Debug, Clone, PartialEq)]
pub struct CovariancePropagation {
    variational: VariationalPropagation,
    covariance: CartesianCovariance,
}

impl CovariancePropagation {
    /// Returns the propagated orbit.
    #[must_use]
    pub const fn final_orbit(&self) -> &Orbit<CartesianState> {
        self.variational.final_orbit()
    }

    /// Returns the state transition matrix.
    #[must_use]
    pub const fn state_transition(&self) -> &CartesianStateTransition {
        self.variational.state_transition()
    }

    /// Returns the covariance mapped as `Phi P0 Phi^T`.
    #[must_use]
    pub const fn covariance(&self) -> &CartesianCovariance {
        &self.covariance
    }
}

impl<P> BogackiShampine32<P>
where
    P: CartesianVariationalDynamics,
{
    /// Integrates the Cartesian state and first variational equations.
    ///
    /// The augmented system advances `d Phi / dt = A(t) Phi` with `Phi(t0)=I`
    /// using the same Bogacki--Shampine stages and combined state/STM local
    /// error norm.
    pub fn propagate_with_state_transition(
        &self,
        initial: Orbit<CartesianState>,
        target: Epoch,
        variational_configuration: VariationalConfiguration,
    ) -> Result<VariationalPropagation, NumericalPropagationError<P::Error>> {
        let OrbitParts { epoch, state } = initial.into();
        self.problem()
            .validate(&state)
            .map_err(NumericalPropagationError::Dynamics)?;
        let raw = integrate_augmented(
            self.problem(),
            self.configuration(),
            variational_configuration,
            epoch,
            state,
            target,
        )?;
        let final_state =
            array_to_state::<P::Error>(state.frame(), std::array::from_fn(|index| raw[index]))?;
        let transition_raw =
            std::array::from_fn(|row| std::array::from_fn(|column| raw[stm_index(row, column)]));
        Ok(VariationalPropagation {
            final_orbit: Orbit::new(target, final_state),
            state_transition: CartesianStateTransition::from_raw(
                epoch,
                target,
                state.frame(),
                transition_raw,
            ),
        })
    }

    /// Integrates the STM and maps a same-frame initial covariance.
    ///
    /// No process-noise or maneuver-execution covariance is added.
    pub fn propagate_with_covariance(
        &self,
        initial: Orbit<CartesianState>,
        covariance: &CartesianCovariance,
        target: Epoch,
        variational_configuration: VariationalConfiguration,
    ) -> Result<CovariancePropagation, VariationalPropagationError<P::Error>> {
        if initial.as_ref().frame() != covariance.frame() {
            return Err(VariationalPropagationError::FrameMismatch {
                state_frame: Box::new(initial.as_ref().frame()),
                covariance_frame: Box::new(covariance.frame()),
            });
        }
        let variational = self
            .propagate_with_state_transition(initial, target, variational_configuration)
            .map_err(VariationalPropagationError::Numerical)?;
        let transition = variational.state_transition.raw();
        let initial_covariance = covariance_raw(covariance);
        let left = matrix_multiply(transition, initial_covariance);
        let propagated_raw = matrix_multiply(left, transpose(transition));
        let propagated_covariance = covariance_from_raw(covariance.frame(), propagated_raw)
            .map_err(VariationalPropagationError::Covariance)?;
        Ok(CovariancePropagation {
            variational,
            covariance: propagated_covariance,
        })
    }
}

fn integrate_augmented<P>(
    problem: &P,
    integration: IntegrationConfiguration,
    variational: VariationalConfiguration,
    initial_epoch: Epoch,
    initial_state: CartesianState,
    target: Epoch,
) -> Result<[f64; AUGMENTED_COMPONENT_COUNT], NumericalPropagationError<P::Error>>
where
    P: CartesianVariationalDynamics,
{
    let total_seconds = (target - initial_epoch).to_seconds();
    if !total_seconds.is_finite() {
        return Err(NumericalPropagationError::NonFiniteDuration);
    }
    let mut values = [0.0; AUGMENTED_COMPONENT_COUNT];
    values[..COMPONENT_COUNT].copy_from_slice(&state_to_array(initial_state));
    for index in 0..6 {
        values[stm_index(index, index)] = 1.0;
    }
    if total_seconds == 0.0 {
        return Ok(values);
    }

    let direction = total_seconds.signum();
    let total_magnitude = total_seconds.abs();
    let minimum_step = integration.minimum_step().to_seconds();
    let maximum_step = integration.maximum_step().to_seconds();
    let mut step_magnitude = integration
        .initial_step()
        .to_seconds()
        .min(maximum_step)
        .min(total_magnitude);
    let mut elapsed = 0.0;
    let mut attempted = 0;
    let mut rejected = 0;
    let frame = initial_state.frame();

    while elapsed < total_magnitude {
        if attempted >= integration.max_steps() {
            return Err(NumericalPropagationError::StepLimitExceeded { attempted });
        }
        let remaining = total_magnitude - elapsed;
        let proposed_magnitude = step_magnitude.min(remaining);
        if proposed_magnitude == 0.0 || !proposed_magnitude.is_finite() {
            return Err(NumericalPropagationError::StepUnderflow);
        }
        let signed_step = direction * proposed_magnitude;
        let signed_elapsed = direction * elapsed;
        let step = augmented_step(
            problem,
            initial_epoch,
            signed_elapsed,
            signed_step,
            frame,
            values,
        )?;
        attempted += 1;
        let error_norm =
            augmented_error_norm(values, step.candidate, step.error, integration, variational);
        if !error_norm.is_finite() {
            return Err(NumericalPropagationError::NonFiniteErrorEstimate);
        }
        let factor = step_factor(error_norm);
        if error_norm <= 1.0 {
            values = step.candidate;
            elapsed = if proposed_magnitude == remaining {
                total_magnitude
            } else {
                elapsed + proposed_magnitude
            };
            step_magnitude = (proposed_magnitude * factor).clamp(minimum_step, maximum_step);
        } else {
            rejected += 1;
            if rejected > integration.max_rejections() {
                return Err(NumericalPropagationError::RejectionLimitExceeded { rejected });
            }
            let reduced = proposed_magnitude * factor.min(1.0);
            if reduced < minimum_step {
                return Err(NumericalPropagationError::MinimumStepExhausted {
                    required_seconds: reduced,
                    minimum_seconds: minimum_step,
                });
            }
            step_magnitude = reduced;
        }
    }
    Ok(values)
}

fn augmented_step<P>(
    problem: &P,
    initial_epoch: Epoch,
    elapsed_seconds: f64,
    step_seconds: f64,
    frame: ReferenceFrame,
    y: [f64; AUGMENTED_COMPONENT_COUNT],
) -> Result<AugmentedStep, NumericalPropagationError<P::Error>>
where
    P: CartesianVariationalDynamics,
{
    let k1 = augmented_derivative(problem, initial_epoch, elapsed_seconds, frame, y)?;
    let k2_state = augmented_combine(y, step_seconds, &[(0.5, k1)]);
    let k2 = augmented_derivative(
        problem,
        initial_epoch,
        elapsed_seconds + 0.5 * step_seconds,
        frame,
        k2_state,
    )?;
    let k3_state = augmented_combine(y, step_seconds, &[(0.75, k2)]);
    let k3 = augmented_derivative(
        problem,
        initial_epoch,
        elapsed_seconds + 0.75 * step_seconds,
        frame,
        k3_state,
    )?;
    let candidate = augmented_combine(
        y,
        step_seconds,
        &[(2.0 / 9.0, k1), (1.0 / 3.0, k2), (4.0 / 9.0, k3)],
    );
    let k4 = augmented_derivative(
        problem,
        initial_epoch,
        elapsed_seconds + step_seconds,
        frame,
        candidate,
    )?;
    let embedded = augmented_combine(
        y,
        step_seconds,
        &[(7.0 / 24.0, k1), (0.25, k2), (1.0 / 3.0, k3), (0.125, k4)],
    );
    let error = std::array::from_fn(|index| candidate[index] - embedded[index]);
    if candidate
        .into_iter()
        .chain(error)
        .any(|value| !value.is_finite())
    {
        return Err(NumericalPropagationError::NonFiniteState);
    }
    Ok(AugmentedStep { candidate, error })
}

fn augmented_derivative<P>(
    problem: &P,
    initial_epoch: Epoch,
    elapsed_seconds: f64,
    frame: ReferenceFrame,
    values: [f64; AUGMENTED_COMPONENT_COUNT],
) -> Result<[f64; AUGMENTED_COMPONENT_COUNT], NumericalPropagationError<P::Error>>
where
    P: CartesianVariationalDynamics,
{
    let state_values = std::array::from_fn(|index| values[index]);
    let state = array_to_state::<P::Error>(frame, state_values)?;
    let epoch = initial_epoch + Duration::from_seconds(elapsed_seconds);
    let acceleration = problem
        .acceleration(epoch, &state)
        .map_err(NumericalPropagationError::Dynamics)?;
    if acceleration.frame() != frame {
        return Err(NumericalPropagationError::AccelerationFrameMismatch {
            state_frame: Box::new(frame),
            acceleration_frame: Box::new(acceleration.frame()),
        });
    }
    let jacobian = problem
        .acceleration_jacobian(epoch, &state)
        .map_err(NumericalPropagationError::Dynamics)?;
    let position_partials = jacobian.position();
    let velocity_partials = jacobian.velocity();
    let mut system = [[0.0; 6]; 6];
    for index in 0..3 {
        system[index][index + 3] = 1.0;
    }
    for row in 0..3 {
        for column in 0..3 {
            system[row + 3][column] = position_partials[row][column].as_per_square_second();
            system[row + 3][column + 3] = velocity_partials[row][column].as_per_second();
        }
    }
    let [ax, ay, az] = acceleration.value().to_metres_per_second_squared();
    let mut derivative = [0.0; AUGMENTED_COMPONENT_COUNT];
    derivative[..6].copy_from_slice(&[values[3], values[4], values[5], ax, ay, az]);
    for row in 0..6 {
        for column in 0..6 {
            derivative[stm_index(row, column)] = (0..6).fold(0.0, |sum, inner| {
                system[row][inner].mul_add(values[stm_index(inner, column)], sum)
            });
        }
    }
    if derivative.into_iter().any(|value| !value.is_finite()) {
        return Err(NumericalPropagationError::NonFiniteDerivative);
    }
    Ok(derivative)
}

fn augmented_combine(
    initial: [f64; AUGMENTED_COMPONENT_COUNT],
    step: f64,
    terms: &[(f64, [f64; AUGMENTED_COMPONENT_COUNT])],
) -> [f64; AUGMENTED_COMPONENT_COUNT] {
    std::array::from_fn(|component| {
        terms
            .iter()
            .fold(initial[component], |value, (weight, derivative)| {
                (step * weight).mul_add(derivative[component], value)
            })
    })
}

fn augmented_error_norm(
    initial: [f64; AUGMENTED_COMPONENT_COUNT],
    candidate: [f64; AUGMENTED_COMPONENT_COUNT],
    error: [f64; AUGMENTED_COMPONENT_COUNT],
    integration: IntegrationConfiguration,
    variational: VariationalConfiguration,
) -> f64 {
    let position_absolute = integration
        .position_absolute_tolerance()
        .get::<units::uom::si::length::meter>();
    let velocity_absolute = integration
        .velocity_absolute_tolerance()
        .get::<units::uom::si::velocity::meter_per_second>();
    let dimensionless_absolute = variational.dimensionless_absolute_tolerance.get::<ratio>();
    let position_velocity_absolute = variational
        .position_velocity_absolute_tolerance
        .get::<second>();
    let velocity_position_absolute = variational
        .velocity_position_absolute_tolerance
        .as_per_second();
    let relative = integration.relative_tolerance().get::<ratio>();
    let sum = (0..AUGMENTED_COMPONENT_COUNT).fold(0.0, |sum, index| {
        let absolute = if index < 3 {
            position_absolute
        } else if index < 6 {
            velocity_absolute
        } else {
            let matrix_index = index - COMPONENT_COUNT;
            let row = matrix_index / 6;
            let column = matrix_index % 6;
            match (row < 3, column < 3) {
                (true, true) | (false, false) => dimensionless_absolute,
                (true, false) => position_velocity_absolute,
                (false, true) => velocity_position_absolute,
            }
        };
        let scale = absolute + relative * initial[index].abs().max(candidate[index].abs());
        (error[index] / scale).mul_add(error[index] / scale, sum)
    });
    (sum / AUGMENTED_COMPONENT_COUNT as f64).sqrt()
}

fn stm_index(row: usize, column: usize) -> usize {
    COMPONENT_COUNT + row * 6 + column
}

#[derive(Debug, Clone, Copy)]
struct AugmentedStep {
    candidate: [f64; AUGMENTED_COMPONENT_COUNT],
    error: [f64; AUGMENTED_COMPONENT_COUNT],
}

fn covariance_raw(covariance: &CartesianCovariance) -> [[f64; 6]; 6] {
    std::array::from_fn(|row| {
        std::array::from_fn(|column| match (row < 3, column < 3) {
            (true, true) => covariance.position_position()[row][column].get::<square_meter>(),
            (true, false) => {
                covariance.position_velocity()[row][column - 3].as_square_metres_per_second()
            }
            (false, true) => {
                covariance.position_velocity()[column][row - 3].as_square_metres_per_second()
            }
            (false, false) => covariance.velocity_velocity()[row - 3][column - 3]
                .as_square_metres_per_square_second(),
        })
    })
}

fn covariance_from_raw(
    frame: ReferenceFrame,
    raw: [[f64; 6]; 6],
) -> Result<CartesianCovariance, CartesianCovarianceError> {
    let symmetric: [[f64; 6]; 6] = std::array::from_fn(|row| {
        std::array::from_fn(|column| 0.5 * (raw[row][column] + raw[column][row]))
    });
    CartesianCovariance::from_blocks(
        frame,
        std::array::from_fn(|row| {
            std::array::from_fn(|column| Area::new::<square_meter>(symmetric[row][column]))
        }),
        std::array::from_fn(|row| {
            std::array::from_fn(|column| {
                PositionVelocityCovariance::from_square_metres_per_second(
                    symmetric[row][column + 3],
                )
            })
        }),
        std::array::from_fn(|row| {
            std::array::from_fn(|column| {
                VelocityVariance::from_square_metres_per_square_second(
                    symmetric[row + 3][column + 3],
                )
            })
        }),
    )
}

fn matrix_multiply(left: [[f64; 6]; 6], right: [[f64; 6]; 6]) -> [[f64; 6]; 6] {
    std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            (0..6).fold(0.0, |sum, inner| {
                left[row][inner].mul_add(right[inner][column], sum)
            })
        })
    })
}

fn transpose(matrix: [[f64; 6]; 6]) -> [[f64; 6]; 6] {
    std::array::from_fn(|row| std::array::from_fn(|column| matrix[column][row]))
}

/// Variational or covariance propagation failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum VariationalPropagationError<E>
where
    E: Error + Send + Sync + 'static,
{
    /// The adaptive state/STM integration failed.
    #[error("state and variational integration failed")]
    Numerical(#[source] NumericalPropagationError<E>),
    /// Initial state and covariance are expressed in different frames.
    #[error("state frame {state_frame} does not match covariance frame {covariance_frame}")]
    FrameMismatch {
        /// Cartesian state frame.
        state_frame: Box<ReferenceFrame>,
        /// Covariance frame.
        covariance_frame: Box<ReferenceFrame>,
    },
    /// The mapped covariance failed domain validation.
    #[error("propagated Cartesian covariance is invalid")]
    Covariance(#[source] CartesianCovarianceError),
}

#[cfg(test)]
mod tests {
    use std::{convert::Infallible, sync::Arc};

    use dynamics::{CartesianAccelerationJacobian, CartesianDynamics, Propagator};
    use dynamics_two_bodies::{PointMassGravityModel, TwoBodyDynamics};
    use frames::{Body, FrameOrigin};
    use gravity::{PointMass, SharedCentralGravity};
    use units::uom::si::{length::meter, ratio::ratio, velocity::meter_per_second};
    use units::{
        AccelerationVector, GravitationalParameter, InverseTimeSquared, Length, Position, Velocity,
        VelocityVector,
    };

    use super::*;

    #[derive(Debug, Clone, Copy)]
    struct InertialMotion;

    impl CartesianDynamics for InertialMotion {
        type Error = Infallible;

        fn validate(&self, _state: &CartesianState) -> Result<(), Self::Error> {
            Ok(())
        }

        fn acceleration(
            &self,
            _epoch: Epoch,
            state: &CartesianState,
        ) -> Result<orbits::cartesian::FramedAcceleration, Self::Error> {
            Ok(orbits::cartesian::FramedAcceleration::new(
                AccelerationVector::from_metres_per_second_squared(0.0, 0.0, 0.0),
                state.frame(),
            )
            .expect("finite acceleration"))
        }
    }

    impl CartesianVariationalDynamics for InertialMotion {
        fn acceleration_jacobian(
            &self,
            _epoch: Epoch,
            _state: &CartesianState,
        ) -> Result<CartesianAccelerationJacobian, Self::Error> {
            Ok(CartesianAccelerationJacobian::new(
                [[InverseTimeSquared::from_per_square_second(0.0); 3]; 3],
                [[InverseTime::from_per_second(0.0); 3]; 3],
            )
            .expect("finite zero Jacobian"))
        }
    }

    fn integration() -> IntegrationConfiguration {
        IntegrationConfiguration::new(
            Length::new::<meter>(1.0e-6),
            Velocity::new::<meter_per_second>(1.0e-9),
            Ratio::new::<ratio>(1.0e-11),
            Duration::from_seconds(1.0e-6),
            Duration::from_seconds(10.0),
            Duration::from_seconds(1.0),
            100_000,
            10_000,
        )
        .expect("valid integration configuration")
    }

    fn variational() -> VariationalConfiguration {
        VariationalConfiguration::new(
            Ratio::new::<ratio>(1.0e-11),
            Time::new::<second>(1.0e-9),
            InverseTime::from_per_second(1.0e-14),
        )
        .expect("valid variational configuration")
    }

    fn orbit(epoch: Epoch) -> Orbit<CartesianState> {
        Orbit::new(
            epoch,
            CartesianState::new(
                ReferenceFrame::GCRF,
                Position::from_metres(7_000_000.0, 0.0, 0.0),
                VelocityVector::from_metres_per_second(0.0, 7_500.0, 0.0),
            )
            .expect("finite state"),
        )
    }

    #[test]
    fn inertial_motion_has_exact_block_state_transition() {
        let epoch = Epoch::from_tai_seconds(1_000.0);
        let duration = 30.0;
        let identity = BogackiShampine32::new(InertialMotion, integration())
            .propagate_with_state_transition(orbit(epoch), epoch, variational())
            .expect("zero-duration variational identity");
        assert_eq!(identity.final_orbit().epoch(), epoch);
        assert_eq!(identity.state_transition().initial_epoch(), epoch);
        assert_eq!(identity.state_transition().final_epoch(), epoch);
        let result = BogackiShampine32::new(InertialMotion, integration())
            .propagate_with_state_transition(
                orbit(epoch),
                epoch + Duration::from_seconds(duration),
                variational(),
            )
            .expect("variational propagation");
        let transition = result.state_transition();
        for row in 0..3 {
            for column in 0..3 {
                let identity_entry = if row == column { 1.0 } else { 0.0 };
                assert!(
                    (transition.position_position()[row][column].get::<ratio>() - identity_entry)
                        .abs()
                        < 1.0e-13
                );
                assert!(
                    (transition.position_velocity()[row][column].get::<second>()
                        - identity_entry * duration)
                        .abs()
                        < 1.0e-12
                );
                assert!(
                    transition.velocity_position()[row][column]
                        .as_per_second()
                        .abs()
                        < 1.0e-15
                );
                assert!(
                    (transition.velocity_velocity()[row][column].get::<ratio>() - identity_entry)
                        .abs()
                        < 1.0e-13
                );
            }
        }
        let reverse = BogackiShampine32::new(InertialMotion, integration())
            .propagate_with_state_transition(
                orbit(epoch),
                epoch - Duration::from_seconds(duration),
                variational(),
            )
            .expect("reverse variational propagation");
        for index in 0..3 {
            assert!(
                (reverse.state_transition().position_velocity()[index][index].get::<second>()
                    + duration)
                    .abs()
                    < 1.0e-12
            );
        }
    }

    #[test]
    fn inertial_covariance_mapping_matches_closed_form() {
        let epoch = Epoch::from_tai_seconds(2_000.0);
        let duration = 20.0;
        let covariance = CartesianCovariance::from_standard_deviations(
            ReferenceFrame::GCRF,
            Position::from_metres(10.0, 20.0, 30.0),
            VelocityVector::from_metres_per_second(1.0, 2.0, 3.0),
        )
        .expect("positive covariance");
        let result = BogackiShampine32::new(InertialMotion, integration())
            .propagate_with_covariance(
                orbit(epoch),
                &covariance,
                epoch + Duration::from_seconds(duration),
                variational(),
            )
            .expect("covariance propagation");
        for index in 0..3 {
            let position_variance = [100.0, 400.0, 900.0][index];
            let velocity_variance = [1.0, 4.0, 9.0][index];
            assert!(
                (result.covariance().position_position()[index][index].get::<square_meter>()
                    - (position_variance + duration * duration * velocity_variance))
                    .abs()
                    < 1.0e-9
            );
            assert!(
                (result.covariance().position_velocity()[index][index]
                    .as_square_metres_per_second()
                    - duration * velocity_variance)
                    .abs()
                    < 1.0e-10
            );
        }
    }

    #[test]
    fn two_body_stm_matches_central_finite_differences() {
        let gravity: SharedCentralGravity = Arc::new(PointMass::new(
            FrameOrigin::Body(Body::EARTH),
            GravitationalParameter::try_from(3.986_004_418e14).expect("positive gravity"),
        ));
        let propagator = BogackiShampine32::new(
            TwoBodyDynamics::new(PointMassGravityModel::new(gravity)),
            integration(),
        );
        let epoch = Epoch::from_tai_seconds(3_000.0);
        let target = epoch + Duration::from_seconds(60.0);
        let initial = orbit(epoch);
        let result = propagator
            .propagate_with_state_transition(initial.clone(), target, variational())
            .expect("variational propagation");
        let transition = result.state_transition().raw();
        let nominal = state_to_array(*initial.as_ref());
        let perturbations = [1.0, 1.0, 1.0, 1.0e-3, 1.0e-3, 1.0e-3];
        for column in 0..6 {
            let mut plus = nominal;
            let mut minus = nominal;
            plus[column] += perturbations[column];
            minus[column] -= perturbations[column];
            let plus = propagator
                .propagate(
                    Orbit::new(
                        epoch,
                        array_to_state::<dynamics_two_bodies::TwoBodyEvaluationError>(
                            ReferenceFrame::GCRF,
                            plus,
                        )
                        .expect("positive perturbation"),
                    ),
                    target,
                )
                .expect("positive propagation");
            let minus = propagator
                .propagate(
                    Orbit::new(
                        epoch,
                        array_to_state::<dynamics_two_bodies::TwoBodyEvaluationError>(
                            ReferenceFrame::GCRF,
                            minus,
                        )
                        .expect("negative perturbation"),
                    ),
                    target,
                )
                .expect("negative propagation");
            let plus = state_to_array(*plus.as_ref());
            let minus = state_to_array(*minus.as_ref());
            for row in 0..6 {
                let finite_difference = (plus[row] - minus[row]) / (2.0 * perturbations[column]);
                let tolerance = if row < 3 { 2.0e-5 } else { 2.0e-8 };
                assert!(
                    (transition[row][column] - finite_difference).abs() < tolerance,
                    "row={row} column={column} variational={} finite_difference={finite_difference}",
                    transition[row][column]
                );
            }
        }
    }

    #[test]
    fn variational_configuration_and_covariance_frames_are_validated() {
        assert_eq!(
            VariationalConfiguration::new(
                Ratio::new::<ratio>(0.0),
                Time::new::<second>(1.0),
                InverseTime::from_per_second(1.0),
            ),
            Err(VariationalConfigurationError::InvalidDimensionlessTolerance)
        );
        assert_eq!(
            VariationalConfiguration::new(
                Ratio::new::<ratio>(1.0),
                Time::new::<second>(f64::NAN),
                InverseTime::from_per_second(1.0),
            ),
            Err(VariationalConfigurationError::InvalidPositionVelocityTolerance)
        );
        let covariance = CartesianCovariance::from_standard_deviations(
            ReferenceFrame::EME2000,
            Position::from_metres(1.0, 1.0, 1.0),
            VelocityVector::from_metres_per_second(1.0, 1.0, 1.0),
        )
        .expect("positive covariance");
        assert!(matches!(
            BogackiShampine32::new(InertialMotion, integration()).propagate_with_covariance(
                orbit(Epoch::from_tai_seconds(4_000.0)),
                &covariance,
                Epoch::from_tai_seconds(4_001.0),
                variational(),
            ),
            Err(VariationalPropagationError::FrameMismatch { .. })
        ));
    }
}
