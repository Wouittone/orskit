#![forbid(unsafe_code)]

//! Adaptive numerical propagation of translational Cartesian states.
//!
//! The private six-component SI kernel uses Fehlberg's embedded RK4(5)
//! formula 2 from Table III of
//! [NASA TR R-315](https://ntrs.nasa.gov/citations/19690021375). Public
//! boundaries remain epoch-, frame-, and unit-qualified. The fifth-order
//! estimate is accepted and its difference from the fourth-order estimate
//! controls the next step. This is local error control; it is not a bound on
//! accumulated global trajectory error.
//!
//! [`DenseEphemeris`] adds endpoint-consistent cubic-Hermite interpolation over
//! accepted RKF45 steps. Event detectors operate on those typed dense states,
//! with bounded bracketed root localization and explicit ordering/handler
//! semantics.
//!
//! RK stage epochs use exact rational fractions of Hifitime's integer
//! nanosecond [`Duration`]. A fractional-nanosecond stage offset is truncated
//! toward zero, symmetrically for forward and backward propagation, while
//! state-stage arithmetic uses the complete signed step in seconds. Dynamics
//! with meaningful sub-nanosecond time variation are outside this capability's
//! supported regime.
//!
//! ```
//! use std::{convert::Infallible, num::NonZeroU64};
//!
//! use dynamics_core::Propagator;
//! use dynamics::numerical::{
//!     AdaptiveRungeKuttaConfig, AdaptiveRungeKuttaFehlberg, AdaptiveStepBounds,
//!     AdaptiveStepLimits, CartesianDynamics, CartesianTolerances,
//! };
//! use frames::ReferenceFrame;
//! use hifitime::{Duration, Epoch};
//! use orbits::cartesian::CartesianState;
//! use orskit_core::Orbit;
//! use units::uom::si::{length::meter, ratio::ratio, velocity::meter_per_second};
//! use units::{AccelerationVector, Length, Position, Ratio, Velocity, VelocityVector};
//!
//! #[derive(Debug)]
//! struct Coast;
//!
//! impl CartesianDynamics for Coast {
//!     type Error = Infallible;
//!
//!     fn frame(&self) -> ReferenceFrame {
//!         ReferenceFrame::GCRF
//!     }
//!
//!     fn acceleration(
//!         &self,
//!         _epoch: Epoch,
//!         _state: CartesianState,
//!     ) -> Result<AccelerationVector, Self::Error> {
//!         Ok(AccelerationVector::from_metres_per_second_squared(0.0, 0.0, 0.0))
//!     }
//! }
//!
//! let config = AdaptiveRungeKuttaConfig::new(
//!     CartesianTolerances::new(
//!         Length::new::<meter>(1.0e-3),
//!         Velocity::new::<meter_per_second>(1.0e-6),
//!         Ratio::new::<ratio>(1.0e-12),
//!     )?,
//!     AdaptiveStepBounds::new(
//!         Duration::from_seconds(1.0e-3),
//!         Duration::from_seconds(60.0),
//!         Duration::from_seconds(10.0),
//!     )?,
//!     AdaptiveStepLimits::new(
//!         NonZeroU64::new(10_000).unwrap(),
//!         NonZeroU64::new(1_000).unwrap(),
//!     ),
//! );
//! let propagator = AdaptiveRungeKuttaFehlberg::new(Coast, config)?;
//! let epoch = Epoch::from_tai_seconds(0.0);
//! let target = epoch + Duration::from_seconds(30.0);
//! let result = propagator.propagate(
//!     Orbit::new(
//!         epoch,
//!         CartesianState::new(
//!             ReferenceFrame::GCRF,
//!             Position::from_metres(7_000_000.0, 0.0, 0.0),
//!             VelocityVector::from_metres_per_second(0.0, 7_500.0, 0.0),
//!         )?,
//!     ),
//!     target,
//! )?;
//!
//! assert_eq!(result.epoch(), target);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::{error::Error as StdError, fmt, num::NonZeroU64};

use dynamics_core::{Propagator, SpacecraftStateRequirements};
use frames::ReferenceFrame;
use hifitime::{Duration, Epoch};
use orbits::cartesian::{CartesianState, StateError};
use orskit_core::{Orbit, OrbitParts};
use thiserror::Error;
use units::uom::si::{length::meter, ratio::ratio, velocity::meter_per_second};
use units::{AccelerationVector, Length, Ratio, Velocity};

mod dense;

use dense::DenseSegment;
pub use dense::{
    DenseEphemeris, DenseOutputError, EphemerisInterval, EventAction, EventDetector,
    EventDirection, EventOccurrence, EventSearchConfig, EventSearchConfigError, EventSearchError,
    EventSearchOutcome, EventStage,
};

const COMPONENT_COUNT: f64 = 6.0;
const SAFETY_FACTOR: f64 = 0.9;
const MINIMUM_SCALE_FACTOR: f64 = 0.2;
const MAXIMUM_SCALE_FACTOR: f64 = 5.0;
const ERROR_EXPONENT: f64 = -0.2;

/// Evaluable acceleration model for one Cartesian frame.
///
/// The model owns or borrows every physical provider it needs. Implementations
/// receive a typed state rather than the integrator's private SI layout.
pub trait CartesianDynamics: fmt::Debug + Send + Sync {
    /// Model/provider failure.
    type Error: StdError + Send + Sync + 'static;

    /// Frame in which this model evaluates position, velocity, and acceleration.
    fn frame(&self) -> ReferenceFrame;

    /// State components required by this model.
    ///
    /// Requirements beyond position and velocity are rejected when the
    /// numerical propagator is constructed.
    fn state_requirements(&self) -> SpacecraftStateRequirements {
        SpacecraftStateRequirements::POSITION.union(SpacecraftStateRequirements::VELOCITY)
    }

    /// Evaluates Cartesian acceleration at `epoch`.
    fn acceleration(
        &self,
        epoch: Epoch,
        state: CartesianState,
    ) -> Result<AccelerationVector, Self::Error>;
}

/// Absolute and relative error scales for Cartesian integration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CartesianTolerances {
    position: Length,
    velocity: Velocity,
    relative: Ratio,
}

impl CartesianTolerances {
    /// Constructs finite, strictly positive component tolerances.
    pub fn new(
        position: Length,
        velocity: Velocity,
        relative: Ratio,
    ) -> Result<Self, ToleranceError> {
        require_positive_finite(position.get::<meter>(), ToleranceComponent::Position)?;
        require_positive_finite(
            velocity.get::<meter_per_second>(),
            ToleranceComponent::Velocity,
        )?;
        require_positive_finite(relative.get::<ratio>(), ToleranceComponent::Relative)?;
        Ok(Self {
            position,
            velocity,
            relative,
        })
    }

    /// Absolute position tolerance applied independently to x, y, and z.
    #[must_use]
    pub const fn position(self) -> Length {
        self.position
    }

    /// Absolute velocity tolerance applied independently to vx, vy, and vz.
    #[must_use]
    pub const fn velocity(self) -> Velocity {
        self.velocity
    }

    /// Dimensionless relative tolerance applied to every component.
    #[must_use]
    pub const fn relative(self) -> Ratio {
        self.relative
    }
}

/// Positive minimum, maximum, and initial step durations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdaptiveStepBounds {
    minimum: Duration,
    maximum: Duration,
    initial: Duration,
}

impl AdaptiveStepBounds {
    /// Constructs ordered positive step bounds with `initial` inside them.
    pub fn new(
        minimum: Duration,
        maximum: Duration,
        initial: Duration,
    ) -> Result<Self, StepBoundsError> {
        if minimum <= Duration::ZERO {
            return Err(StepBoundsError::NonPositiveMinimum);
        }
        if maximum < minimum {
            return Err(StepBoundsError::MaximumBeforeMinimum);
        }
        if initial < minimum || initial > maximum {
            return Err(StepBoundsError::InitialOutsideBounds);
        }
        Ok(Self {
            minimum,
            maximum,
            initial,
        })
    }

    /// Smallest ordinary adaptive step.
    #[must_use]
    pub const fn minimum(self) -> Duration {
        self.minimum
    }

    /// Largest adaptive step.
    #[must_use]
    pub const fn maximum(self) -> Duration {
        self.maximum
    }

    /// First attempted step.
    #[must_use]
    pub const fn initial(self) -> Duration {
        self.initial
    }
}

/// Deterministic accepted-step and rejected-step limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdaptiveStepLimits {
    maximum_steps: NonZeroU64,
    maximum_rejections: NonZeroU64,
}

impl AdaptiveStepLimits {
    /// Constructs explicit non-zero solver limits.
    #[must_use]
    pub const fn new(maximum_steps: NonZeroU64, maximum_rejections: NonZeroU64) -> Self {
        Self {
            maximum_steps,
            maximum_rejections,
        }
    }

    /// Maximum accepted steps.
    #[must_use]
    pub const fn maximum_steps(self) -> NonZeroU64 {
        self.maximum_steps
    }

    /// Maximum rejected attempts over one propagation.
    #[must_use]
    pub const fn maximum_rejections(self) -> NonZeroU64 {
        self.maximum_rejections
    }
}

/// Complete Fehlberg RK4(5) adaptive configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdaptiveRungeKuttaConfig {
    tolerances: CartesianTolerances,
    step_bounds: AdaptiveStepBounds,
    step_limits: AdaptiveStepLimits,
}

impl AdaptiveRungeKuttaConfig {
    /// Combines independently validated tolerances, bounds, and limits.
    #[must_use]
    pub const fn new(
        tolerances: CartesianTolerances,
        step_bounds: AdaptiveStepBounds,
        step_limits: AdaptiveStepLimits,
    ) -> Self {
        Self {
            tolerances,
            step_bounds,
            step_limits,
        }
    }

    /// Component error tolerances.
    #[must_use]
    pub const fn tolerances(self) -> CartesianTolerances {
        self.tolerances
    }

    /// Step-size bounds.
    #[must_use]
    pub const fn step_bounds(self) -> AdaptiveStepBounds {
        self.step_bounds
    }

    /// Step and rejection limits.
    #[must_use]
    pub const fn step_limits(self) -> AdaptiveStepLimits {
        self.step_limits
    }
}

/// Adaptive Fehlberg RK4(5) propagator owning one evaluable dynamics model.
///
/// The scaled root-mean-square error norm uses
/// `absolute + relative * max(|initial|, |candidate|)` independently for all
/// three position and three velocity components. Accepted steps use the
/// fifth-order estimate. The controller is
/// `0.9 * error^(-1/5)`, clamped to `[0.2, 5]`.
#[derive(Debug)]
pub struct AdaptiveRungeKuttaFehlberg<D> {
    dynamics: D,
    config: AdaptiveRungeKuttaConfig,
}

impl<D: CartesianDynamics> AdaptiveRungeKuttaFehlberg<D> {
    /// Constructs a propagator after rejecting unsupported component needs.
    pub fn new(
        dynamics: D,
        config: AdaptiveRungeKuttaConfig,
    ) -> Result<Self, NumericalPropagatorBuildError> {
        let requirements = dynamics.state_requirements();
        let supported =
            SpacecraftStateRequirements::POSITION.union(SpacecraftStateRequirements::VELOCITY);
        if !supported.contains(requirements) {
            return Err(
                NumericalPropagatorBuildError::UnsupportedStateRequirements { requirements },
            );
        }
        Ok(Self { dynamics, config })
    }

    /// Owned evaluable dynamics.
    #[must_use]
    pub const fn dynamics(&self) -> &D {
        &self.dynamics
    }

    /// Validated solver configuration.
    #[must_use]
    pub const fn config(&self) -> AdaptiveRungeKuttaConfig {
        self.config
    }

    /// Generates a dense ephemeris over the complete directional interval.
    ///
    /// Accepted RKF45 endpoints retain the existing fifth-order endpoint
    /// solution. Each accepted step receives an endpoint-consistent cubic
    /// Hermite extension using the dynamics derivatives at both ends.
    pub fn generate_ephemeris(
        &self,
        initial: Orbit<CartesianState>,
        target: Epoch,
    ) -> Result<DenseEphemeris, NumericalPropagationError<D::Error>> {
        self.integrate(initial, target, true)
            .map(|(_, ephemeris)| ephemeris.expect("dense collection requested"))
    }

    fn derivative(
        &self,
        epoch: Epoch,
        frame: ReferenceFrame,
        state: StateVector,
    ) -> Result<StateVector, NumericalPropagationError<D::Error>> {
        if !state.into_iter().all(f64::is_finite) {
            return Err(NumericalPropagationError::NonFiniteState);
        }
        let typed = CartesianState::new(
            frame,
            units::Position::from_metres(state[0], state[1], state[2]),
            units::VelocityVector::from_metres_per_second(state[3], state[4], state[5]),
        )?;
        let acceleration = self.dynamics.acceleration(epoch, typed).map_err(|source| {
            NumericalPropagationError::Dynamics {
                source: Box::new(source),
            }
        })?;
        if !acceleration.is_finite() {
            return Err(NumericalPropagationError::NonFiniteDerivative);
        }
        let acceleration = acceleration.to_metres_per_second_squared();
        Ok([
            state[3],
            state[4],
            state[5],
            acceleration[0],
            acceleration[1],
            acceleration[2],
        ])
    }

    fn attempt(
        &self,
        epoch: Epoch,
        frame: ReferenceFrame,
        state: StateVector,
        step: Duration,
    ) -> Result<StepEstimate, NumericalPropagationError<D::Error>> {
        let h = step.to_seconds();
        let k1 = self.derivative(epoch, frame, state)?;
        let k2 = self.derivative(
            epoch + fractional_duration(step, 1, 4),
            frame,
            combine(state, h, &[(1.0 / 4.0, k1)]),
        )?;
        let k3 = self.derivative(
            epoch + fractional_duration(step, 3, 8),
            frame,
            combine(state, h, &[(3.0 / 32.0, k1), (9.0 / 32.0, k2)]),
        )?;
        let k4 = self.derivative(
            epoch + fractional_duration(step, 12, 13),
            frame,
            combine(
                state,
                h,
                &[
                    (1932.0 / 2197.0, k1),
                    (-7200.0 / 2197.0, k2),
                    (7296.0 / 2197.0, k3),
                ],
            ),
        )?;
        let k5 = self.derivative(
            epoch + step,
            frame,
            combine(
                state,
                h,
                &[
                    (439.0 / 216.0, k1),
                    (-8.0, k2),
                    (3680.0 / 513.0, k3),
                    (-845.0 / 4104.0, k4),
                ],
            ),
        )?;
        let k6 = self.derivative(
            epoch + fractional_duration(step, 1, 2),
            frame,
            combine(
                state,
                h,
                &[
                    (-8.0 / 27.0, k1),
                    (2.0, k2),
                    (-3544.0 / 2565.0, k3),
                    (1859.0 / 4104.0, k4),
                    (-11.0 / 40.0, k5),
                ],
            ),
        )?;

        let fourth = combine(
            state,
            h,
            &[
                (25.0 / 216.0, k1),
                (1408.0 / 2565.0, k3),
                (2197.0 / 4104.0, k4),
                (-1.0 / 5.0, k5),
            ],
        );
        let fifth = combine(
            state,
            h,
            &[
                (16.0 / 135.0, k1),
                (6656.0 / 12825.0, k3),
                (28561.0 / 56430.0, k4),
                (-9.0 / 50.0, k5),
                (2.0 / 55.0, k6),
            ],
        );
        if !fourth.into_iter().chain(fifth).all(f64::is_finite) {
            return Err(NumericalPropagationError::NonFiniteState);
        }
        Ok(StepEstimate {
            fifth,
            initial_derivative: k1,
            error_norm: self.error_norm(state, fourth, fifth)?,
        })
    }

    fn error_norm(
        &self,
        initial: StateVector,
        fourth: StateVector,
        fifth: StateVector,
    ) -> Result<f64, NumericalPropagationError<D::Error>> {
        let tolerances = self.config.tolerances;
        let relative = tolerances.relative.get::<ratio>();
        let position_absolute = tolerances.position.get::<meter>();
        let velocity_absolute = tolerances.velocity.get::<meter_per_second>();
        let mut sum = 0.0;
        for index in 0..6 {
            let absolute = if index < 3 {
                position_absolute
            } else {
                velocity_absolute
            };
            let scale = absolute + relative * initial[index].abs().max(fifth[index].abs());
            if !scale.is_finite() || scale <= 0.0 {
                return Err(NumericalPropagationError::NonFiniteErrorEstimate);
            }
            let normalized = (fifth[index] - fourth[index]) / scale;
            if !normalized.is_finite() {
                return Err(NumericalPropagationError::NonFiniteErrorEstimate);
            }
            sum = normalized.mul_add(normalized, sum);
        }
        let norm = (sum / COMPONENT_COUNT).sqrt();
        if norm.is_finite() {
            Ok(norm)
        } else {
            Err(NumericalPropagationError::NonFiniteErrorEstimate)
        }
    }

    fn integrate(
        &self,
        initial: Orbit<CartesianState>,
        target: Epoch,
        collect_dense: bool,
    ) -> Result<IntegrationResult, NumericalPropagationError<D::Error>> {
        let OrbitParts { epoch, state } = initial.into();
        let frame = state.frame();
        let dynamics_frame = self.dynamics.frame();
        if frame != dynamics_frame {
            return Err(NumericalPropagationError::FrameMismatch {
                frames: Box::new((frame, dynamics_frame)),
            });
        }
        let initial_vector = state_vector(state);
        if target == epoch {
            let ephemeris = collect_dense.then(|| {
                DenseEphemeris::new(
                    EphemerisInterval::new(epoch, target),
                    frame,
                    initial_vector,
                    Vec::new(),
                )
            });
            return Ok((Orbit::new(target, state), ephemeris));
        }

        let mut current_epoch = epoch;
        let mut current = initial_vector;
        let mut next_step = self.config.step_bounds.initial;
        let mut accepted_steps = 0_u64;
        let mut rejected_steps = 0_u64;
        let forward = target > epoch;
        let mut segments = Vec::new();

        while current_epoch != target {
            if accepted_steps >= self.config.step_limits.maximum_steps.get() {
                return Err(NumericalPropagationError::StepLimitExceeded {
                    maximum: self.config.step_limits.maximum_steps,
                });
            }
            let remaining = if forward {
                target - current_epoch
            } else {
                current_epoch - target
            };
            let step_magnitude = next_step.min(remaining);
            let signed_step = if forward {
                step_magnitude
            } else {
                -step_magnitude
            };
            let estimate = self.attempt(current_epoch, frame, current, signed_step)?;
            let factor = controller_factor(estimate.error_norm);

            if estimate.error_norm <= 1.0 {
                let next_epoch = if step_magnitude == remaining {
                    target
                } else {
                    current_epoch + signed_step
                };
                if collect_dense {
                    let end_derivative = self.derivative(next_epoch, frame, estimate.fifth)?;
                    segments.push(DenseSegment::new(
                        current_epoch,
                        next_epoch,
                        current,
                        estimate.fifth,
                        estimate.initial_derivative,
                        end_derivative,
                    ));
                }
                current = estimate.fifth;
                current_epoch = next_epoch;
                accepted_steps += 1;
                next_step = scaled_step_bounded(
                    step_magnitude,
                    factor,
                    self.config.step_bounds.minimum,
                    self.config.step_bounds.maximum,
                );
            } else {
                rejected_steps += 1;
                if rejected_steps > self.config.step_limits.maximum_rejections.get() {
                    return Err(NumericalPropagationError::RejectionLimitExceeded {
                        maximum: self.config.step_limits.maximum_rejections,
                    });
                }
                let reduced = scaled_step_bounded(
                    step_magnitude,
                    factor,
                    Duration::ZERO,
                    self.config.step_bounds.maximum,
                );
                if reduced < self.config.step_bounds.minimum {
                    return Err(NumericalPropagationError::MinimumStepExhausted {
                        minimum: self.config.step_bounds.minimum,
                    });
                }
                next_step = reduced;
            }
        }

        let propagated = CartesianState::new(
            frame,
            units::Position::from_metres(current[0], current[1], current[2]),
            units::VelocityVector::from_metres_per_second(current[3], current[4], current[5]),
        )?;
        let ephemeris = collect_dense.then(|| {
            DenseEphemeris::new(
                EphemerisInterval::new(epoch, target),
                frame,
                initial_vector,
                segments,
            )
        });
        Ok((Orbit::new(target, propagated), ephemeris))
    }
}

impl<D: CartesianDynamics> Propagator<CartesianState> for AdaptiveRungeKuttaFehlberg<D> {
    type Error = NumericalPropagationError<D::Error>;

    fn propagate(
        &self,
        initial: Orbit<CartesianState>,
        target: Epoch,
    ) -> Result<Orbit<CartesianState>, Self::Error> {
        self.integrate(initial, target, false)
            .map(|(endpoint, _)| endpoint)
    }
}

/// Invalid Cartesian tolerance configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ToleranceError {
    /// A tolerance was zero, negative, NaN, or infinite.
    #[error("{component:?} tolerance must be finite and strictly positive")]
    NonPositiveOrNonFinite {
        /// Invalid tolerance component.
        component: ToleranceComponent,
    },
}

/// One configured tolerance component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToleranceComponent {
    /// Absolute position tolerance.
    Position,
    /// Absolute velocity tolerance.
    Velocity,
    /// Dimensionless relative tolerance.
    Relative,
}

/// Invalid adaptive step bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum StepBoundsError {
    /// Minimum duration was zero or negative.
    #[error("minimum integration step must be positive")]
    NonPositiveMinimum,
    /// Maximum duration preceded the minimum.
    #[error("maximum integration step must not be smaller than the minimum")]
    MaximumBeforeMinimum,
    /// Initial duration was outside the inclusive bounds.
    #[error("initial integration step must lie within the configured bounds")]
    InitialOutsideBounds,
}

/// Failure to construct a Cartesian numerical propagator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum NumericalPropagatorBuildError {
    /// The dynamics requires state components outside position and velocity.
    #[error("Cartesian numerical propagation cannot satisfy requirements {requirements:?}")]
    UnsupportedStateRequirements {
        /// Requirements declared by the dynamics.
        requirements: SpacecraftStateRequirements,
    },
}

/// Failure during adaptive Cartesian propagation.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum NumericalPropagationError<E: StdError + Send + Sync + 'static> {
    /// State and dynamics frames differ.
    #[error("Cartesian state/dynamics frame pair is incompatible: {frames:?}")]
    FrameMismatch {
        /// State frame followed by dynamics frame.
        frames: Box<(ReferenceFrame, ReferenceFrame)>,
    },
    /// The dynamics provider failed.
    #[error("Cartesian dynamics evaluation failed")]
    Dynamics {
        /// Provider-specific source.
        #[source]
        source: Box<E>,
    },
    /// A state entering or leaving the numerical kernel was non-finite.
    #[error("numerical Cartesian state is non-finite")]
    NonFiniteState,
    /// An acceleration evaluation returned NaN or infinity.
    #[error("Cartesian dynamics returned a non-finite acceleration")]
    NonFiniteDerivative,
    /// Scaled local error could not be represented finitely.
    #[error("scaled Runge-Kutta local-error estimate is non-finite")]
    NonFiniteErrorEstimate,
    /// Error control requested a step below the configured minimum.
    #[error("local error cannot be controlled at minimum step {minimum}")]
    MinimumStepExhausted {
        /// Configured lower step bound.
        minimum: Duration,
    },
    /// Accepted-step limit was exhausted before reaching the target.
    #[error("accepted-step limit {maximum} exhausted before the target epoch")]
    StepLimitExceeded {
        /// Configured accepted-step limit.
        maximum: NonZeroU64,
    },
    /// Rejected-step limit was exhausted before reaching the target.
    #[error("rejected-step limit {maximum} exhausted before the target epoch")]
    RejectionLimitExceeded {
        /// Configured rejected-step limit.
        maximum: NonZeroU64,
    },
    /// A typed Cartesian state could not be reconstructed.
    #[error(transparent)]
    InvalidState(#[from] StateError),
}

type StateVector = [f64; 6];
type IntegrationResult = (Orbit<CartesianState>, Option<DenseEphemeris>);

#[derive(Debug, Clone, Copy)]
struct StepEstimate {
    fifth: StateVector,
    initial_derivative: StateVector,
    error_norm: f64,
}

fn require_positive_finite(
    value: f64,
    component: ToleranceComponent,
) -> Result<(), ToleranceError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(ToleranceError::NonPositiveOrNonFinite { component })
    }
}

fn state_vector(state: CartesianState) -> StateVector {
    let position = state.position().to_metres();
    let velocity = state.velocity().to_metres_per_second();
    [
        position[0],
        position[1],
        position[2],
        velocity[0],
        velocity[1],
        velocity[2],
    ]
}

fn combine(initial: StateVector, step: f64, stages: &[(f64, StateVector)]) -> StateVector {
    std::array::from_fn(|component| {
        stages
            .iter()
            .fold(initial[component], |value, (coefficient, stage)| {
                (step * coefficient).mul_add(stage[component], value)
            })
    })
}

fn controller_factor(error_norm: f64) -> f64 {
    if error_norm == 0.0 {
        MAXIMUM_SCALE_FACTOR
    } else {
        (SAFETY_FACTOR * error_norm.powf(ERROR_EXPONENT))
            .clamp(MINIMUM_SCALE_FACTOR, MAXIMUM_SCALE_FACTOR)
    }
}

fn scaled_step_bounded(
    step: Duration,
    factor: f64,
    minimum: Duration,
    maximum: Duration,
) -> Duration {
    let seconds = step.to_seconds() * factor;
    if !seconds.is_finite() || seconds >= maximum.to_seconds() {
        maximum
    } else if seconds <= minimum.to_seconds() {
        minimum
    } else {
        Duration::from_seconds(seconds)
    }
}

fn fractional_duration(duration: Duration, numerator: i128, denominator: i128) -> Duration {
    debug_assert!(denominator > 0 && numerator.abs() <= denominator);
    let total = duration.total_nanoseconds();
    let quotient = total / denominator;
    let remainder = total % denominator;
    Duration::from_total_nanoseconds(quotient * numerator + remainder * numerator / denominator)
}

#[cfg(test)]
mod tests {
    use std::{
        convert::Infallible,
        f64::consts::PI,
        sync::{
            atomic::{AtomicU64, Ordering},
            Arc, Mutex,
        },
    };

    use bodies::Body;
    use dynamics_two_bodies::{EllipticKeplerPropagator, PointMassGravityModel, TwoBodyDynamics};
    use frames::{FrameOrigin, InertialFrame};
    use gravity::{PointMass, SharedCentralGravity};
    use orbits::keplerian::KeplerianState;
    use units::uom::si::{
        acceleration::meter_per_second_squared, angle::radian, length::meter, ratio::ratio,
        velocity::meter_per_second,
    };
    use units::{Acceleration, Angle, GravitationalParameter, Position, VelocityVector};

    use super::*;

    fn limits(maximum_steps: u64, maximum_rejections: u64) -> AdaptiveStepLimits {
        AdaptiveStepLimits::new(
            NonZeroU64::new(maximum_steps).expect("non-zero step limit"),
            NonZeroU64::new(maximum_rejections).expect("non-zero rejection limit"),
        )
    }

    fn config(
        position_metres: f64,
        velocity_metres_per_second: f64,
        relative: f64,
        minimum_seconds: f64,
        maximum_seconds: f64,
        initial_seconds: f64,
    ) -> AdaptiveRungeKuttaConfig {
        AdaptiveRungeKuttaConfig::new(
            CartesianTolerances::new(
                Length::new::<meter>(position_metres),
                Velocity::new::<meter_per_second>(velocity_metres_per_second),
                Ratio::new::<ratio>(relative),
            )
            .expect("valid tolerances"),
            AdaptiveStepBounds::new(
                Duration::from_seconds(minimum_seconds),
                Duration::from_seconds(maximum_seconds),
                Duration::from_seconds(initial_seconds),
            )
            .expect("valid bounds"),
            limits(100_000, 100_000),
        )
    }

    fn orbit(position: [f64; 3], velocity: [f64; 3]) -> Orbit<CartesianState> {
        Orbit::new(
            Epoch::from_tai_seconds(1_000.0),
            CartesianState::new(
                ReferenceFrame::GCRF,
                Position::from_metres(position[0], position[1], position[2]),
                VelocityVector::from_metres_per_second(velocity[0], velocity[1], velocity[2]),
            )
            .expect("finite state"),
        )
    }

    #[derive(Debug)]
    struct ConstantAcceleration([f64; 3]);

    impl CartesianDynamics for ConstantAcceleration {
        type Error = Infallible;

        fn frame(&self) -> ReferenceFrame {
            ReferenceFrame::GCRF
        }

        fn acceleration(
            &self,
            _epoch: Epoch,
            _state: CartesianState,
        ) -> Result<AccelerationVector, Self::Error> {
            Ok(AccelerationVector::from_metres_per_second_squared(
                self.0[0], self.0[1], self.0[2],
            ))
        }
    }

    #[derive(Debug, Default)]
    struct HarmonicOscillator {
        calls: AtomicU64,
    }

    impl CartesianDynamics for HarmonicOscillator {
        type Error = Infallible;

        fn frame(&self) -> ReferenceFrame {
            ReferenceFrame::GCRF
        }

        fn acceleration(
            &self,
            _epoch: Epoch,
            state: CartesianState,
        ) -> Result<AccelerationVector, Self::Error> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            let position = state.position().to_metres();
            Ok(AccelerationVector::from_metres_per_second_squared(
                -position[0],
                -position[1],
                -position[2],
            ))
        }
    }

    #[test]
    fn configuration_rejects_invalid_tolerances_and_bounds() {
        assert_eq!(
            CartesianTolerances::new(
                Length::new::<meter>(0.0),
                Velocity::new::<meter_per_second>(1.0),
                Ratio::new::<ratio>(1.0e-9),
            ),
            Err(ToleranceError::NonPositiveOrNonFinite {
                component: ToleranceComponent::Position,
            })
        );
        assert_eq!(
            AdaptiveStepBounds::new(
                Duration::from_seconds(2.0),
                Duration::from_seconds(1.0),
                Duration::from_seconds(1.0),
            ),
            Err(StepBoundsError::MaximumBeforeMinimum)
        );
    }

    #[test]
    fn constant_acceleration_reaches_exact_targets_forward_and_backward() {
        let propagator = AdaptiveRungeKuttaFehlberg::new(
            ConstantAcceleration([2.0, -1.0, 0.5]),
            config(1.0e-9, 1.0e-9, 1.0e-12, 0.001, 7.0, 3.0),
        )
        .expect("Cartesian requirements");
        let initial = orbit([10.0, 20.0, -5.0], [3.0, -4.0, 2.0]);
        let initial_epoch = initial.epoch();

        for elapsed in [10.25, -10.25] {
            let target = initial_epoch + Duration::from_seconds(elapsed);
            let propagated = propagator
                .propagate(initial.clone(), target)
                .expect("polynomial solution");
            assert_eq!(propagated.epoch(), target);
            let expected_position = [
                10.0 + 3.0 * elapsed + elapsed * elapsed,
                20.0 - 4.0 * elapsed - 0.5 * elapsed * elapsed,
                -5.0 + 2.0 * elapsed + 0.25 * elapsed * elapsed,
            ];
            let expected_velocity = [3.0 + 2.0 * elapsed, -4.0 - elapsed, 2.0 + 0.5 * elapsed];
            assert_components_close(
                propagated.as_ref().position().to_metres(),
                expected_position,
                2.0e-11,
            );
            assert_components_close(
                propagated.as_ref().velocity().to_metres_per_second(),
                expected_velocity,
                2.0e-12,
            );
        }
    }

    #[test]
    fn dense_output_is_endpoint_consistent_and_exact_for_quadratic_motion() {
        let propagator = AdaptiveRungeKuttaFehlberg::new(
            ConstantAcceleration([2.0, -1.0, 0.5]),
            config(1.0e-9, 1.0e-9, 1.0e-12, 0.001, 2.0, 2.0),
        )
        .expect("Cartesian requirements");
        let initial = orbit([10.0, 20.0, -5.0], [3.0, -4.0, 2.0]);
        let start = initial.epoch();

        for elapsed in [5.0, -5.0] {
            let target = start + Duration::from_seconds(elapsed);
            let ephemeris = propagator
                .generate_ephemeris(initial.clone(), target)
                .expect("dense polynomial solution");
            assert_eq!(ephemeris.interval(), EphemerisInterval::new(start, target));
            assert_eq!(
                ephemeris
                    .state_at(start)
                    .expect("initial dense endpoint")
                    .as_ref(),
                initial.as_ref()
            );
            let endpoint = propagator
                .propagate(initial.clone(), target)
                .expect("ordinary endpoint");
            assert_eq!(
                ephemeris
                    .state_at(target)
                    .expect("final dense endpoint")
                    .as_ref(),
                endpoint.as_ref()
            );

            let sample_elapsed = elapsed * 0.37;
            let sample = ephemeris
                .state_at(start + Duration::from_seconds(sample_elapsed))
                .expect("interior dense state");
            assert_components_close(
                sample.as_ref().position().to_metres(),
                [
                    10.0 + 3.0 * sample_elapsed + sample_elapsed * sample_elapsed,
                    20.0 - 4.0 * sample_elapsed - 0.5 * sample_elapsed * sample_elapsed,
                    -5.0 + 2.0 * sample_elapsed + 0.25 * sample_elapsed * sample_elapsed,
                ],
                3.0e-12,
            );
        }
    }

    #[test]
    fn cubic_dense_extension_shows_fourth_order_interpolation_error() {
        fn error(step: f64) -> f64 {
            let propagator = AdaptiveRungeKuttaFehlberg::new(
                HarmonicOscillator::default(),
                config(1.0e20, 1.0e20, 1.0, step, step, step),
            )
            .expect("Cartesian requirements");
            let initial = orbit([1.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
            let sample_seconds = step * 0.37;
            let ephemeris = propagator
                .generate_ephemeris(
                    initial.clone(),
                    initial.epoch() + Duration::from_seconds(step),
                )
                .expect("single dense step");
            let sample = ephemeris
                .state_at(initial.epoch() + Duration::from_seconds(sample_seconds))
                .expect("interior state");
            (sample.as_ref().position().x().get::<meter>() - sample_seconds.cos()).abs()
        }

        let refinement = error(0.4) / error(0.2);
        assert!(
            refinement > 12.0,
            "observed dense refinement ratio was {refinement}"
        );
    }

    #[derive(Debug, Error)]
    #[error("event callback failed")]
    struct EventFailure;

    #[derive(Debug)]
    struct XCrossing {
        root_metres: f64,
        direction: EventDirection,
        action: EventAction,
        fail: bool,
        log: Arc<Mutex<Vec<usize>>>,
    }

    impl EventDetector for XCrossing {
        type Error = EventFailure;

        fn direction(&self) -> EventDirection {
            self.direction
        }

        fn value(&mut self, state: &Orbit<CartesianState>) -> Result<f64, Self::Error> {
            if self.fail {
                return Err(EventFailure);
            }
            Ok(state.as_ref().position().x().get::<meter>() - self.root_metres)
        }

        fn handle(&mut self, event: &EventOccurrence) -> Result<EventAction, Self::Error> {
            self.log
                .lock()
                .expect("event log")
                .push(event.detector_index());
            Ok(self.action)
        }
    }

    fn event_config(iterations: u64, events: u64) -> EventSearchConfig {
        EventSearchConfig::new(
            Duration::from_nanoseconds(1.0),
            NonZeroU64::new(iterations).expect("non-zero iteration limit"),
            NonZeroU64::new(events).expect("non-zero event limit"),
        )
        .expect("positive root tolerance")
    }

    #[test]
    fn event_direction_is_physical_time_for_forward_and_backward_ephemerides() {
        let propagator = AdaptiveRungeKuttaFehlberg::new(
            ConstantAcceleration([0.0; 3]),
            config(1.0e-9, 1.0e-9, 1.0e-12, 10.0, 10.0, 10.0),
        )
        .expect("Cartesian requirements");
        let initial = orbit([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);

        for (elapsed, root) in [(10.0, 3.25), (-10.0, -3.25)] {
            let ephemeris = propagator
                .generate_ephemeris(
                    initial.clone(),
                    initial.epoch() + Duration::from_seconds(elapsed),
                )
                .expect("linear ephemeris");
            let log = Arc::new(Mutex::new(Vec::new()));
            let mut increasing = [XCrossing {
                root_metres: root,
                direction: EventDirection::Increasing,
                action: EventAction::Continue,
                fail: false,
                log: Arc::clone(&log),
            }];
            let outcome = ephemeris
                .find_events(&mut increasing, event_config(64, 4))
                .expect("increasing root");
            assert_eq!(outcome.events().len(), 1);
            assert_eq!(outcome.events()[0].direction(), EventDirection::Increasing);
            assert!(
                (outcome.events()[0].epoch() - (initial.epoch() + Duration::from_seconds(root)))
                    .to_seconds()
                    .abs()
                    <= 1.0e-9
            );

            let mut decreasing = [XCrossing {
                root_metres: root,
                direction: EventDirection::Decreasing,
                action: EventAction::Continue,
                fail: false,
                log: Arc::clone(&log),
            }];
            assert!(ephemeris
                .find_events(&mut decreasing, event_config(64, 4))
                .expect("filtered search")
                .events()
                .is_empty());
        }
    }

    #[test]
    fn simultaneous_handlers_are_ordered_and_all_run_before_stop() {
        let propagator = AdaptiveRungeKuttaFehlberg::new(
            ConstantAcceleration([0.0; 3]),
            config(1.0e-9, 1.0e-9, 1.0e-12, 10.0, 10.0, 10.0),
        )
        .expect("Cartesian requirements");
        let initial = orbit([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        let ephemeris = propagator
            .generate_ephemeris(
                initial.clone(),
                initial.epoch() + Duration::from_seconds(10.0),
            )
            .expect("linear ephemeris");
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut detectors = [
            XCrossing {
                root_metres: 5.0,
                direction: EventDirection::Any,
                action: EventAction::Stop,
                fail: false,
                log: Arc::clone(&log),
            },
            XCrossing {
                root_metres: 5.0,
                direction: EventDirection::Any,
                action: EventAction::Continue,
                fail: false,
                log: Arc::clone(&log),
            },
            XCrossing {
                root_metres: 8.0,
                direction: EventDirection::Any,
                action: EventAction::Continue,
                fail: false,
                log: Arc::clone(&log),
            },
        ];
        let outcome = ephemeris
            .find_events(&mut detectors, event_config(64, 8))
            .expect("deterministic event handling");
        assert!(outcome.stopped());
        assert_eq!(
            outcome
                .events()
                .iter()
                .map(EventOccurrence::detector_index)
                .collect::<Vec<_>>(),
            [0, 1]
        );
        assert_eq!(*log.lock().expect("event log"), [0, 1]);
    }

    #[test]
    fn simultaneous_handlers_cross_step_boundaries_in_both_directions() {
        let propagator = AdaptiveRungeKuttaFehlberg::new(
            ConstantAcceleration([0.0; 3]),
            config(1.0e-9, 1.0e-9, 1.0e-12, 5.0, 5.0, 5.0),
        )
        .expect("Cartesian requirements");
        let initial = orbit([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);

        for elapsed in [10.0, -10.0] {
            let ephemeris = propagator
                .generate_ephemeris(
                    initial.clone(),
                    initial.epoch() + Duration::from_seconds(elapsed),
                )
                .expect("two-segment linear ephemeris");
            assert_eq!(ephemeris.segment_count(), 2);
            let boundary = elapsed / 2.0;
            let propagation_offset = elapsed.signum() * 1.0e-10;
            let log = Arc::new(Mutex::new(Vec::new()));
            let mut detectors = [
                XCrossing {
                    root_metres: boundary - propagation_offset,
                    direction: EventDirection::Any,
                    action: EventAction::Stop,
                    fail: false,
                    log: Arc::clone(&log),
                },
                XCrossing {
                    root_metres: boundary + propagation_offset,
                    direction: EventDirection::Any,
                    action: EventAction::Continue,
                    fail: false,
                    log: Arc::clone(&log),
                },
            ];

            let outcome = ephemeris
                .find_events(&mut detectors, event_config(64, 4))
                .expect("cross-boundary simultaneous group");

            assert!(outcome.stopped());
            assert_eq!(
                outcome
                    .events()
                    .iter()
                    .map(EventOccurrence::detector_index)
                    .collect::<Vec<_>>(),
                [0, 1]
            );
            assert_eq!(*log.lock().expect("event log"), [0, 1]);
        }
    }

    #[test]
    fn root_localization_accepts_tolerance_reached_on_final_iteration() {
        let propagator = AdaptiveRungeKuttaFehlberg::new(
            ConstantAcceleration([0.0; 3]),
            config(1.0e-9, 1.0e-9, 1.0e-12, 2.0e-9, 2.0e-9, 2.0e-9),
        )
        .expect("Cartesian requirements");
        let initial = orbit([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        let ephemeris = propagator
            .generate_ephemeris(
                initial.clone(),
                initial.epoch() + Duration::from_nanoseconds(2.0),
            )
            .expect("two-nanosecond linear ephemeris");
        let mut detector = [XCrossing {
            root_metres: 0.5e-9,
            direction: EventDirection::Any,
            action: EventAction::Continue,
            fail: false,
            log: Arc::new(Mutex::new(Vec::new())),
        }];

        let outcome = ephemeris
            .find_events(&mut detector, event_config(1, 1))
            .expect("last permitted bisection reaches tolerance");

        assert_eq!(outcome.events().len(), 1);
    }

    #[test]
    fn simultaneous_event_limit_failure_is_atomic() {
        let propagator = AdaptiveRungeKuttaFehlberg::new(
            ConstantAcceleration([0.0; 3]),
            config(1.0e-9, 1.0e-9, 1.0e-12, 10.0, 10.0, 10.0),
        )
        .expect("Cartesian requirements");
        let initial = orbit([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        let ephemeris = propagator
            .generate_ephemeris(
                initial.clone(),
                initial.epoch() + Duration::from_seconds(10.0),
            )
            .expect("linear ephemeris");
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut detectors = [
            XCrossing {
                root_metres: 5.0,
                direction: EventDirection::Any,
                action: EventAction::Continue,
                fail: false,
                log: Arc::clone(&log),
            },
            XCrossing {
                root_metres: 5.0,
                direction: EventDirection::Any,
                action: EventAction::Continue,
                fail: false,
                log: Arc::clone(&log),
            },
        ];

        assert!(matches!(
            ephemeris.find_events(&mut detectors, event_config(64, 1)),
            Err(EventSearchError::EventLimitExceeded { .. })
        ));
        assert!(log.lock().expect("event log").is_empty());
    }

    #[test]
    fn event_callback_and_root_limit_failures_are_typed() {
        use std::error::Error as _;

        let propagator = AdaptiveRungeKuttaFehlberg::new(
            ConstantAcceleration([0.0; 3]),
            config(1.0e-9, 1.0e-9, 1.0e-12, 10.0, 10.0, 10.0),
        )
        .expect("Cartesian requirements");
        let initial = orbit([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        let ephemeris = propagator
            .generate_ephemeris(
                initial.clone(),
                initial.epoch() + Duration::from_seconds(10.0),
            )
            .expect("linear ephemeris");
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut failing = [XCrossing {
            root_metres: 3.0,
            direction: EventDirection::Any,
            action: EventAction::Continue,
            fail: true,
            log: Arc::clone(&log),
        }];
        let error = ephemeris
            .find_events(&mut failing, event_config(64, 4))
            .expect_err("detector fails");
        assert!(matches!(
            error,
            EventSearchError::Detector {
                stage: EventStage::Evaluation,
                ..
            }
        ));
        assert!(error.source().is_some());

        let mut limited = [XCrossing {
            root_metres: 3.0,
            direction: EventDirection::Any,
            action: EventAction::Continue,
            fail: false,
            log,
        }];
        assert!(matches!(
            ephemeris.find_events(&mut limited, event_config(1, 4)),
            Err(EventSearchError::RootIterationLimitExceeded { .. })
        ));
    }

    #[test]
    fn dense_and_event_search_boundaries_are_typed() {
        #[derive(Debug)]
        struct NonFiniteEvent;

        impl EventDetector for NonFiniteEvent {
            type Error = Infallible;

            fn value(&mut self, _state: &Orbit<CartesianState>) -> Result<f64, Self::Error> {
                Ok(f64::NAN)
            }
        }

        assert_eq!(
            EventSearchConfig::new(
                Duration::ZERO,
                NonZeroU64::new(1).expect("non-zero"),
                NonZeroU64::new(1).expect("non-zero"),
            ),
            Err(EventSearchConfigError::NonPositiveEpochTolerance)
        );

        let propagator = AdaptiveRungeKuttaFehlberg::new(
            ConstantAcceleration([0.0; 3]),
            config(1.0e-9, 1.0e-9, 1.0e-12, 10.0, 10.0, 10.0),
        )
        .expect("Cartesian requirements");
        let initial = orbit([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        let ephemeris = propagator
            .generate_ephemeris(
                initial.clone(),
                initial.epoch() + Duration::from_seconds(10.0),
            )
            .expect("linear ephemeris");
        assert!(matches!(
            ephemeris.state_at(initial.epoch() - Duration::from_nanoseconds(1.0)),
            Err(DenseOutputError::OutsideInterval { .. })
        ));

        assert!(matches!(
            ephemeris.find_events(&mut [NonFiniteEvent], event_config(64, 4)),
            Err(EventSearchError::NonFiniteValue { .. })
        ));

        let log = Arc::new(Mutex::new(Vec::new()));
        let mut detectors = [
            XCrossing {
                root_metres: 2.0,
                direction: EventDirection::Any,
                action: EventAction::Continue,
                fail: false,
                log: Arc::clone(&log),
            },
            XCrossing {
                root_metres: 8.0,
                direction: EventDirection::Any,
                action: EventAction::Continue,
                fail: false,
                log,
            },
        ];
        assert!(matches!(
            ephemeris.find_events(&mut detectors, event_config(64, 1)),
            Err(EventSearchError::EventLimitExceeded { .. })
        ));
    }

    #[test]
    fn fifth_order_solution_shows_expected_global_convergence() {
        let coarse = harmonic_error(0.2);
        let fine = harmonic_error(0.1);
        assert!(
            coarse / fine > 24.0,
            "observed refinement ratio was {}",
            coarse / fine
        );
    }

    fn harmonic_error(step_seconds: f64) -> f64 {
        let config = AdaptiveRungeKuttaConfig::new(
            CartesianTolerances::new(
                Length::new::<meter>(1.0e20),
                Velocity::new::<meter_per_second>(1.0e20),
                Ratio::new::<ratio>(1.0),
            )
            .expect("loose finite tolerances"),
            AdaptiveStepBounds::new(
                Duration::from_seconds(step_seconds),
                Duration::from_seconds(step_seconds),
                Duration::from_seconds(step_seconds),
            )
            .expect("fixed step"),
            limits(100, 1),
        );
        let propagator = AdaptiveRungeKuttaFehlberg::new(HarmonicOscillator::default(), config)
            .expect("Cartesian requirements");
        let result = propagator
            .propagate(
                orbit([1.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
                Epoch::from_tai_seconds(1_001.0),
            )
            .expect("fixed-step propagation");
        let x_error = (result.as_ref().position().x().get::<meter>() - 1.0_f64.cos()).abs();
        let v_error =
            (result.as_ref().velocity().x().get::<meter_per_second>() + 1.0_f64.sin()).abs();
        x_error.hypot(v_error)
    }

    #[test]
    fn tight_tolerance_rejects_without_mutating_the_accepted_state() {
        let propagator = AdaptiveRungeKuttaFehlberg::new(
            HarmonicOscillator::default(),
            config(1.0e-10, 1.0e-10, 1.0e-12, 1.0e-6, 1.0, 1.0),
        )
        .expect("Cartesian requirements");
        let result = propagator
            .propagate(
                orbit([1.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
                Epoch::from_tai_seconds(1_001.0),
            )
            .expect("adaptive propagation");
        assert!(
            propagator.dynamics().calls.load(Ordering::Relaxed) > 6,
            "at least one six-stage attempt should have been rejected"
        );
        assert!((result.as_ref().position().x().get::<meter>() - 1.0_f64.cos()).abs() < 2.0e-10);
    }

    #[derive(Debug)]
    struct UnsupportedDynamics;

    impl CartesianDynamics for UnsupportedDynamics {
        type Error = Infallible;

        fn frame(&self) -> ReferenceFrame {
            ReferenceFrame::GCRF
        }

        fn state_requirements(&self) -> SpacecraftStateRequirements {
            SpacecraftStateRequirements::POSITION.union(SpacecraftStateRequirements::MASS)
        }

        fn acceleration(
            &self,
            _epoch: Epoch,
            _state: CartesianState,
        ) -> Result<AccelerationVector, Self::Error> {
            unreachable!("construction rejects unsupported requirements")
        }
    }

    #[test]
    fn construction_rejects_non_translational_requirements() {
        assert!(matches!(
            AdaptiveRungeKuttaFehlberg::new(
                UnsupportedDynamics,
                config(1.0, 1.0, 1.0e-9, 0.1, 1.0, 1.0),
            ),
            Err(NumericalPropagatorBuildError::UnsupportedStateRequirements { .. })
        ));
    }

    #[derive(Debug)]
    struct NonFiniteDynamics;

    impl CartesianDynamics for NonFiniteDynamics {
        type Error = Infallible;

        fn frame(&self) -> ReferenceFrame {
            ReferenceFrame::GCRF
        }

        fn acceleration(
            &self,
            _epoch: Epoch,
            _state: CartesianState,
        ) -> Result<AccelerationVector, Self::Error> {
            Ok(AccelerationVector::new(
                Acceleration::new::<meter_per_second_squared>(f64::NAN),
                Acceleration::new::<meter_per_second_squared>(0.0),
                Acceleration::new::<meter_per_second_squared>(0.0),
            ))
        }
    }

    #[test]
    fn non_finite_derivative_and_step_limit_are_typed() {
        let non_finite = AdaptiveRungeKuttaFehlberg::new(
            NonFiniteDynamics,
            config(1.0, 1.0, 1.0e-9, 0.1, 1.0, 1.0),
        )
        .expect("Cartesian requirements");
        assert!(matches!(
            non_finite.propagate(
                orbit([1.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
                Epoch::from_tai_seconds(1_001.0),
            ),
            Err(NumericalPropagationError::NonFiniteDerivative)
        ));

        let one_step_config = AdaptiveRungeKuttaConfig::new(
            config(1.0, 1.0, 1.0e-9, 0.25, 0.25, 0.25).tolerances(),
            AdaptiveStepBounds::new(
                Duration::from_seconds(0.25),
                Duration::from_seconds(0.25),
                Duration::from_seconds(0.25),
            )
            .expect("fixed step"),
            limits(1, 1),
        );
        let one_step =
            AdaptiveRungeKuttaFehlberg::new(ConstantAcceleration([0.0; 3]), one_step_config)
                .expect("Cartesian requirements");
        assert!(matches!(
            one_step.propagate(
                orbit([1.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
                Epoch::from_tai_seconds(1_001.0),
            ),
            Err(NumericalPropagationError::StepLimitExceeded { .. })
        ));
    }

    #[test]
    fn non_finite_component_scale_cannot_false_accept_a_step() {
        let huge = 1.0e308;
        let propagator = AdaptiveRungeKuttaFehlberg::new(
            ConstantAcceleration([0.0; 3]),
            config(huge, huge, 1.0, 1.0, 1.0, 1.0),
        )
        .expect("Cartesian requirements");
        assert!(matches!(
            propagator.propagate(
                orbit([huge, huge, huge], [0.0, 0.0, 0.0]),
                Epoch::from_tai_seconds(1_001.0),
            ),
            Err(NumericalPropagationError::NonFiniteErrorEstimate)
        ));
    }

    #[derive(Debug, Error)]
    #[error("intentional dynamics failure")]
    struct DynamicsFailure;

    #[derive(Debug)]
    struct FailingDynamics;

    impl CartesianDynamics for FailingDynamics {
        type Error = DynamicsFailure;

        fn frame(&self) -> ReferenceFrame {
            ReferenceFrame::GCRF
        }

        fn acceleration(
            &self,
            _epoch: Epoch,
            _state: CartesianState,
        ) -> Result<AccelerationVector, Self::Error> {
            Err(DynamicsFailure)
        }
    }

    #[test]
    fn dynamics_failures_retain_their_source() {
        use std::error::Error as _;

        let propagator = AdaptiveRungeKuttaFehlberg::new(
            FailingDynamics,
            config(1.0, 1.0, 1.0e-9, 0.1, 1.0, 1.0),
        )
        .expect("Cartesian requirements");
        let error = propagator
            .propagate(
                orbit([1.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
                Epoch::from_tai_seconds(1_001.0),
            )
            .expect_err("model fails");
        assert!(matches!(error, NumericalPropagationError::Dynamics { .. }));
        assert!(error.source().is_some());
    }

    #[test]
    fn minimum_step_and_rejection_limits_are_typed() {
        let minimum_step = AdaptiveRungeKuttaFehlberg::new(
            HarmonicOscillator::default(),
            config(1.0e-30, 1.0e-30, 1.0e-30, 1.0, 1.0, 1.0),
        )
        .expect("Cartesian requirements");
        assert!(matches!(
            minimum_step.propagate(
                orbit([1.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
                Epoch::from_tai_seconds(1_001.0),
            ),
            Err(NumericalPropagationError::MinimumStepExhausted { .. })
        ));

        let rejection_config = AdaptiveRungeKuttaConfig::new(
            CartesianTolerances::new(
                Length::new::<meter>(1.0e-30),
                Velocity::new::<meter_per_second>(1.0e-30),
                Ratio::new::<ratio>(1.0e-30),
            )
            .expect("positive tolerances"),
            AdaptiveStepBounds::new(
                Duration::from_seconds(1.0e-9),
                Duration::from_seconds(1.0),
                Duration::from_seconds(1.0),
            )
            .expect("valid bounds"),
            limits(100_000, 1),
        );
        let rejection_limit =
            AdaptiveRungeKuttaFehlberg::new(HarmonicOscillator::default(), rejection_config)
                .expect("Cartesian requirements");
        assert!(matches!(
            rejection_limit.propagate(
                orbit([1.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
                Epoch::from_tai_seconds(1_001.0),
            ),
            Err(NumericalPropagationError::RejectionLimitExceeded { .. })
        ));
    }

    #[derive(Debug)]
    struct PointMassDynamics {
        mu: f64,
    }

    impl CartesianDynamics for PointMassDynamics {
        type Error = Infallible;

        fn frame(&self) -> ReferenceFrame {
            ReferenceFrame::GCRF
        }

        fn state_requirements(&self) -> SpacecraftStateRequirements {
            SpacecraftStateRequirements::POSITION
        }

        fn acceleration(
            &self,
            _epoch: Epoch,
            state: CartesianState,
        ) -> Result<AccelerationVector, Self::Error> {
            let position = state.position().to_metres();
            let radius_squared = position[0].mul_add(
                position[0],
                position[1].mul_add(position[1], position[2] * position[2]),
            );
            let factor = -self.mu / (radius_squared * radius_squared.sqrt());
            Ok(AccelerationVector::from_metres_per_second_squared(
                factor * position[0],
                factor * position[1],
                factor * position[2],
            ))
        }
    }

    #[test]
    fn point_mass_scenario_matches_the_independently_validated_analytical_solver() {
        let parameter = GravitationalParameter::try_from(3.986_004_418e14)
            .expect("IERS Earth parameter is positive");
        let gravity: SharedCentralGravity =
            Arc::new(PointMass::new(FrameOrigin::Body(Body::EARTH), parameter));
        let elements = KeplerianState::new(
            InertialFrame::GCRF,
            Arc::clone(&gravity),
            Length::new::<meter>(7_200_000.0),
            Ratio::new::<ratio>(0.1),
            Angle::new::<radian>(0.7),
            Angle::new::<radian>(1.1),
            Angle::new::<radian>(0.4),
            Angle::new::<radian>(2.0),
        )
        .expect("elliptic reference orbit");
        let cartesian = CartesianState::try_from(&elements).expect("Cartesian conversion");
        let initial_epoch = Epoch::from_tai_seconds(1_000.0);
        let target = initial_epoch + Duration::from_seconds(3_600.0);
        let initial = Orbit::new(initial_epoch, cartesian);
        let initial_energy = specific_energy(cartesian, parameter);
        let initial_angular_momentum = angular_momentum_norm(cartesian);

        let numerical = AdaptiveRungeKuttaFehlberg::new(
            PointMassDynamics {
                mu: parameter.as_cubic_metres_per_second_squared(),
            },
            config(1.0e-4, 1.0e-7, 1.0e-13, 1.0e-3, 120.0, 30.0),
        )
        .expect("Cartesian requirements")
        .propagate(initial.clone(), target)
        .expect("numerical propagation");
        let analytical = EllipticKeplerPropagator::new(TwoBodyDynamics::new(
            PointMassGravityModel::new(gravity),
        ))
        .propagate(initial, target)
        .expect("analytical propagation");

        assert_components_close(
            numerical.as_ref().position().to_metres(),
            analytical.as_ref().position().to_metres(),
            0.02,
        );
        assert_components_close(
            numerical.as_ref().velocity().to_metres_per_second(),
            analytical.as_ref().velocity().to_metres_per_second(),
            2.0e-5,
        );
        let energy_drift = (specific_energy(*numerical.as_ref(), parameter) - initial_energy).abs()
            / initial_energy.abs();
        let angular_momentum_drift =
            (angular_momentum_norm(*numerical.as_ref()) - initial_angular_momentum).abs()
                / initial_angular_momentum;
        assert!(
            energy_drift < 2.0e-10,
            "relative energy drift {energy_drift}"
        );
        assert!(
            angular_momentum_drift < 2.0e-10,
            "relative angular-momentum drift {angular_momentum_drift}"
        );
        // The analytical endpoint is separately checked against Orekit 13.1.6
        // black-box output in `dynamics-two-bodies`.
        assert_components_close(
            numerical.as_ref().position().to_metres(),
            [
                4.863_976_030_492_352e6,
                4.133_125_643_091_070_5e6,
                -2.072_064_351_084_958e6,
            ],
            0.02,
        );
    }

    #[test]
    fn exact_identity_target_still_checks_frame_compatibility() {
        #[derive(Debug)]
        struct WrongFrame;
        impl CartesianDynamics for WrongFrame {
            type Error = Infallible;

            fn frame(&self) -> ReferenceFrame {
                ReferenceFrame::EME2000
            }

            fn acceleration(
                &self,
                _epoch: Epoch,
                _state: CartesianState,
            ) -> Result<AccelerationVector, Self::Error> {
                Ok(AccelerationVector::from_metres_per_second_squared(
                    0.0, 0.0, 0.0,
                ))
            }
        }
        let propagator =
            AdaptiveRungeKuttaFehlberg::new(WrongFrame, config(1.0, 1.0, 1.0e-9, 0.1, 1.0, 1.0))
                .expect("Cartesian requirements");
        assert!(matches!(
            propagator.propagate(
                orbit([1.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
                Epoch::from_tai_seconds(1_000.0),
            ),
            Err(NumericalPropagationError::FrameMismatch { .. })
        ));
    }

    #[derive(Debug, Default)]
    struct EpochRecorder {
        epochs: Mutex<Vec<Epoch>>,
    }

    impl CartesianDynamics for EpochRecorder {
        type Error = Infallible;

        fn frame(&self) -> ReferenceFrame {
            ReferenceFrame::GCRF
        }

        fn acceleration(
            &self,
            epoch: Epoch,
            _state: CartesianState,
        ) -> Result<AccelerationVector, Self::Error> {
            self.epochs.lock().expect("recorder lock").push(epoch);
            Ok(AccelerationVector::from_metres_per_second_squared(
                0.0, 0.0, 0.0,
            ))
        }
    }

    #[test]
    fn fractional_nanosecond_stage_epochs_truncate_toward_zero() {
        let ten_nanoseconds = Duration::from_nanoseconds(10.0);
        let fixed = AdaptiveRungeKuttaConfig::new(
            CartesianTolerances::new(
                Length::new::<meter>(1.0),
                Velocity::new::<meter_per_second>(1.0),
                Ratio::new::<ratio>(1.0),
            )
            .expect("positive tolerances"),
            AdaptiveStepBounds::new(ten_nanoseconds, ten_nanoseconds, ten_nanoseconds)
                .expect("fixed nanosecond step"),
            limits(1, 1),
        );
        let initial = orbit([1.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        let epoch = initial.epoch();

        for (target, expected_offsets) in [
            (epoch + ten_nanoseconds, [0_i128, 2, 3, 9, 10, 5]),
            (epoch - ten_nanoseconds, [0_i128, -2, -3, -9, -10, -5]),
        ] {
            let propagator = AdaptiveRungeKuttaFehlberg::new(EpochRecorder::default(), fixed)
                .expect("Cartesian requirements");
            propagator
                .propagate(initial.clone(), target)
                .expect("single fixed step");
            let epochs = propagator.dynamics().epochs.lock().expect("recorder lock");
            let actual: Vec<_> = epochs
                .iter()
                .map(|stage| (*stage - epoch).total_nanoseconds())
                .collect();
            assert_eq!(actual, expected_offsets);
        }
    }

    #[test]
    fn fractional_duration_and_step_scaling_are_bounded_at_extremes() {
        for total in [i128::MAX, i128::MIN + 1] {
            let duration = Duration::from_total_nanoseconds(total);
            let total = duration.total_nanoseconds();
            let fraction = fractional_duration(duration, 12, 13);
            let expected = (total / 13) * 12 + (total % 13) * 12 / 13;
            assert_eq!(fraction.total_nanoseconds(), expected);
        }

        let maximum = Duration::from_total_nanoseconds(i128::MAX);
        assert_eq!(
            scaled_step_bounded(maximum, MAXIMUM_SCALE_FACTOR, Duration::ZERO, maximum),
            maximum
        );
        let minimum = Duration::from_nanoseconds(1.0);
        assert_eq!(scaled_step_bounded(minimum, 0.0, minimum, maximum), minimum);
    }

    fn assert_components_close(actual: [f64; 3], expected: [f64; 3], tolerance: f64) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() <= tolerance,
                "{actual:.16e} differs from {expected:.16e} by more than {tolerance:.3e}"
            );
        }
    }

    fn specific_energy(state: CartesianState, parameter: GravitationalParameter) -> f64 {
        let position = state.position().to_metres();
        let velocity = state.velocity().to_metres_per_second();
        let radius = position[0]
            .mul_add(
                position[0],
                position[1].mul_add(position[1], position[2] * position[2]),
            )
            .sqrt();
        let speed_squared = velocity[0].mul_add(
            velocity[0],
            velocity[1].mul_add(velocity[1], velocity[2] * velocity[2]),
        );
        speed_squared / 2.0 - parameter.as_cubic_metres_per_second_squared() / radius
    }

    fn angular_momentum_norm(state: CartesianState) -> f64 {
        let position = state.position().to_metres();
        let velocity = state.velocity().to_metres_per_second();
        let cross = [
            position[1] * velocity[2] - position[2] * velocity[1],
            position[2] * velocity[0] - position[0] * velocity[2],
            position[0] * velocity[1] - position[1] * velocity[0],
        ];
        cross[0]
            .mul_add(cross[0], cross[1].mul_add(cross[1], cross[2] * cross[2]))
            .sqrt()
    }

    #[test]
    fn quarter_period_harmonic_direction_is_sane() {
        let propagator = AdaptiveRungeKuttaFehlberg::new(
            HarmonicOscillator::default(),
            config(1.0e-9, 1.0e-9, 1.0e-12, 1.0e-4, 0.2, 0.1),
        )
        .expect("Cartesian requirements");
        let result = propagator
            .propagate(
                orbit([1.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
                Epoch::from_tai_seconds(1_000.0) + Duration::from_seconds(PI / 2.0),
            )
            .expect("harmonic propagation");
        assert!(result.as_ref().position().x().get::<meter>().abs() < 2.0e-9);
        assert!((result.as_ref().velocity().x().get::<meter_per_second>() + 1.0).abs() < 2.0e-9);
    }
}
