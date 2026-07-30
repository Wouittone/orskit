//! Scheduled impulsive and constant-thrust maneuver propagation.

use std::error::Error;

#[cfg(feature = "attitude")]
use attitude::AttitudeProvider;
use frames::ReferenceFrame;
use hifitime::{Duration, Epoch};
use orbits::cartesian::{CartesianState, FramedAcceleration};
#[cfg(feature = "attitude")]
use orskit_core::{Attitude, FramedForce, OrientationForceError};
use orskit_core::{Orbit, SpacecraftBodyFrame};
use thiserror::Error;
use units::uom::si::{
    f64::{Force, MassRate},
    force::newton,
    mass::kilogram,
    mass_rate::kilogram_per_second,
};
use units::{AccelerationVector, Mass, VelocityVector};

use crate::{BogackiShampine32, CartesianDynamics, NumericalPropagationError};
use dynamics::Propagator;

/// Three force components expressed in one declared reference frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThrustVector {
    components: [Force; 3],
}

/// Axes in which a finite-burn thrust vector remains constant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThrustFrame {
    /// Components are already expressed in the propagated Cartesian frame.
    Reference(ReferenceFrame),
    /// Components are fixed in one spacecraft-owned body frame.
    Body(SpacecraftBodyFrame),
}

impl ThrustFrame {
    /// Returns the frame identity attached to the thrust components.
    #[must_use]
    pub const fn reference_frame(&self) -> ReferenceFrame {
        match self {
            Self::Reference(frame) => *frame,
            Self::Body(frame) => frame.reference_frame(),
        }
    }

    /// Returns whether the thrust requires an attitude provider.
    #[must_use]
    pub const fn is_body_fixed(&self) -> bool {
        matches!(self, Self::Body(_))
    }
}

impl ThrustVector {
    /// Constructs a typed thrust vector.
    #[must_use]
    pub const fn new(x: Force, y: Force, z: Force) -> Self {
        Self {
            components: [x, y, z],
        }
    }

    /// Constructs a thrust vector from an explicit raw-SI boundary.
    #[must_use]
    pub fn from_newtons(x: f64, y: f64, z: f64) -> Self {
        Self::new(
            Force::new::<newton>(x),
            Force::new::<newton>(y),
            Force::new::<newton>(z),
        )
    }

    /// Returns the typed x/y/z components.
    #[must_use]
    pub const fn components(self) -> [Force; 3] {
        self.components
    }

    /// Extracts raw newton components for numerical interoperability.
    #[must_use]
    pub fn to_newtons(self) -> [f64; 3] {
        self.components.map(|component| component.get::<newton>())
    }

    fn is_finite(self) -> bool {
        self.to_newtons().into_iter().all(f64::is_finite)
    }

    fn is_zero(self) -> bool {
        self.to_newtons()
            .into_iter()
            .all(|component| component == 0.0)
    }
}

/// Epoch-qualified Cartesian state with strictly positive spacecraft mass.
#[derive(Debug, Clone, PartialEq)]
pub struct CartesianMassState {
    orbit: Orbit<CartesianState>,
    mass: Mass,
}

impl CartesianMassState {
    /// Composes a Cartesian orbit and the spacecraft mass valid at its epoch.
    pub fn new(
        orbit: Orbit<CartesianState>,
        mass: Mass,
    ) -> Result<Self, ManeuverConfigurationError> {
        validate_positive_mass(mass)?;
        Ok(Self { orbit, mass })
    }

    /// Returns the epoch-qualified Cartesian orbit.
    #[must_use]
    pub const fn orbit(&self) -> &Orbit<CartesianState> {
        &self.orbit
    }

    /// Returns the spacecraft mass at the orbit epoch.
    #[must_use]
    pub const fn mass(&self) -> Mass {
        self.mass
    }

    /// Consumes the state into its orbit and mass.
    #[must_use]
    pub fn into_parts(self) -> (Orbit<CartesianState>, Mass) {
        (self.orbit, self.mass)
    }
}

/// One instantaneous velocity and mass discontinuity.
///
/// `delta_velocity` and `propellant_mass` describe the forward-time jump.
/// Reverse propagation applies their exact inverse.
#[derive(Debug, Clone, PartialEq)]
pub struct ImpulsiveManeuver {
    name: Box<str>,
    epoch: Epoch,
    frame: ReferenceFrame,
    delta_velocity: VelocityVector,
    propellant_mass: Mass,
}

impl ImpulsiveManeuver {
    /// Creates one scheduled impulse expressed in `frame`.
    pub fn new(
        name: impl Into<Box<str>>,
        epoch: Epoch,
        frame: ReferenceFrame,
        delta_velocity: VelocityVector,
        propellant_mass: Mass,
    ) -> Result<Self, ManeuverConfigurationError> {
        let name = name.into();
        validate_name(&name)?;
        if !delta_velocity.is_finite() {
            return Err(ManeuverConfigurationError::NonFiniteDeltaVelocity);
        }
        validate_non_negative_mass(propellant_mass)?;
        Ok(Self {
            name,
            epoch,
            frame,
            delta_velocity,
            propellant_mass,
        })
    }

    /// Returns the stable diagnostic name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the instantaneous maneuver epoch.
    #[must_use]
    pub const fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Returns the frame of the velocity jump.
    #[must_use]
    pub const fn frame(&self) -> ReferenceFrame {
        self.frame
    }

    /// Returns the forward-time velocity jump.
    #[must_use]
    pub const fn delta_velocity(&self) -> VelocityVector {
        self.delta_velocity
    }

    /// Returns the forward-time propellant consumption.
    #[must_use]
    pub const fn propellant_mass(&self) -> Mass {
        self.propellant_mass
    }
}

/// Constant frame-resolved thrust and propellant flow over `[start, end]`.
///
/// The thrust components remain constant in the selected reference or body
/// frame. Body-fixed thrust requires
/// [`BogackiShampine32::propagate_with_attitude_maneuvers`] and is rotated at
/// every numerical stage using the caller-selected attitude provider. Mass
/// follows the exact linear law `dm/dt = -mass_flow_rate`, while translational
/// acceleration uses `thrust / mass(epoch)` at every numerical stage.
///
/// The force and variable-mass relation follows D. M. Goebel and I. Katz,
/// [*Fundamentals of Electric Propulsion: Ion and Hall
/// Thrusters*](https://descanso.jpl.nasa.gov/SciTechBook/series1/Goebel_02_Chap2_thruster.pdf),
/// JPL Space Science and Technology Series, 2008, chapter 2.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstantThrustManeuver {
    name: Box<str>,
    start: Epoch,
    end: Epoch,
    thrust_frame: ThrustFrame,
    thrust: ThrustVector,
    mass_flow_rate: MassRate,
}

impl ConstantThrustManeuver {
    /// Creates one finite burn.
    pub fn new(
        name: impl Into<Box<str>>,
        start: Epoch,
        end: Epoch,
        frame: ReferenceFrame,
        thrust: ThrustVector,
        mass_flow_rate: MassRate,
    ) -> Result<Self, ManeuverConfigurationError> {
        let name = name.into();
        validate_name(&name)?;
        if end <= start {
            return Err(ManeuverConfigurationError::InvalidFiniteInterval);
        }
        if !thrust.is_finite() {
            return Err(ManeuverConfigurationError::NonFiniteThrust);
        }
        if thrust.is_zero() {
            return Err(ManeuverConfigurationError::ZeroThrust);
        }
        let flow = mass_flow_rate.get::<kilogram_per_second>();
        if !flow.is_finite() || flow <= 0.0 {
            return Err(ManeuverConfigurationError::InvalidMassFlowRate);
        }
        Ok(Self {
            name,
            start,
            end,
            thrust_frame: ThrustFrame::Reference(frame),
            thrust,
            mass_flow_rate,
        })
    }

    /// Creates one finite burn whose thrust is constant in spacecraft body axes.
    pub fn body_fixed(
        name: impl Into<Box<str>>,
        start: Epoch,
        end: Epoch,
        body_frame: SpacecraftBodyFrame,
        thrust: ThrustVector,
        mass_flow_rate: MassRate,
    ) -> Result<Self, ManeuverConfigurationError> {
        let mut burn = Self::new(
            name,
            start,
            end,
            body_frame.reference_frame(),
            thrust,
            mass_flow_rate,
        )?;
        burn.thrust_frame = ThrustFrame::Body(body_frame);
        Ok(burn)
    }

    /// Returns the stable diagnostic name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the burn start epoch.
    #[must_use]
    pub const fn start(&self) -> Epoch {
        self.start
    }

    /// Returns the burn end epoch.
    #[must_use]
    pub const fn end(&self) -> Epoch {
        self.end
    }

    /// Returns the frame in which thrust is constant.
    #[must_use]
    pub const fn frame(&self) -> ReferenceFrame {
        self.thrust_frame.reference_frame()
    }

    /// Returns whether thrust is reference-frame or spacecraft-body fixed.
    #[must_use]
    pub const fn thrust_frame(&self) -> &ThrustFrame {
        &self.thrust_frame
    }

    /// Returns the constant thrust vector.
    #[must_use]
    pub const fn thrust(&self) -> ThrustVector {
        self.thrust
    }

    /// Returns the positive forward-time propellant flow magnitude.
    #[must_use]
    pub const fn mass_flow_rate(&self) -> MassRate {
        self.mass_flow_rate
    }

    fn contains_open_interval(&self, left: Epoch, right: Epoch) -> bool {
        let midpoint = left + Duration::from_seconds(0.5 * (right - left).to_seconds());
        midpoint > self.start && midpoint < self.end
    }
}

/// Validated deterministic schedule of non-overlapping finite burns and impulses.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ManeuverSchedule {
    impulses: Vec<ImpulsiveManeuver>,
    finite_burns: Vec<ConstantThrustManeuver>,
}

impl ManeuverSchedule {
    /// Validates and orders a complete maneuver schedule.
    ///
    /// Simultaneous impulses preserve registration order. Finite burns may
    /// touch at an endpoint but may not overlap.
    pub fn new(
        mut impulses: Vec<ImpulsiveManeuver>,
        mut finite_burns: Vec<ConstantThrustManeuver>,
    ) -> Result<Self, ManeuverConfigurationError> {
        impulses.sort_by_key(ImpulsiveManeuver::epoch);
        finite_burns.sort_by_key(ConstantThrustManeuver::start);
        for pair in finite_burns.windows(2) {
            if pair[1].start < pair[0].end {
                return Err(ManeuverConfigurationError::OverlappingFiniteBurns);
            }
        }
        Ok(Self {
            impulses,
            finite_burns,
        })
    }

    /// Returns scheduled impulses in epoch and registration order.
    #[must_use]
    pub fn impulses(&self) -> &[ImpulsiveManeuver] {
        &self.impulses
    }

    /// Returns finite burns in chronological order.
    #[must_use]
    pub fn finite_burns(&self) -> &[ConstantThrustManeuver] {
        &self.finite_burns
    }

    fn boundary_epochs(&self, initial: Epoch, target: Epoch) -> Vec<Epoch> {
        let (coverage_start, coverage_end) = if initial <= target {
            (initial, target)
        } else {
            (target, initial)
        };
        let mut epochs = Vec::new();
        for impulse in &self.impulses {
            if impulse.epoch >= coverage_start
                && impulse.epoch <= coverage_end
                && impulse.epoch != initial
            {
                epochs.push(impulse.epoch);
            }
        }
        for burn in &self.finite_burns {
            if burn.start >= coverage_start && burn.start <= coverage_end && burn.start != initial {
                epochs.push(burn.start);
            }
            if burn.end >= coverage_start && burn.end <= coverage_end && burn.end != initial {
                epochs.push(burn.end);
            }
        }
        epochs.push(target);
        epochs.sort();
        epochs.dedup();
        if target < initial {
            epochs.reverse();
        }
        epochs
    }

    fn active_burn(&self, left: Epoch, right: Epoch) -> Option<&ConstantThrustManeuver> {
        self.finite_burns
            .iter()
            .find(|burn| burn.contains_open_interval(left, right))
    }
}

/// Kind of one executed schedule portion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ManeuverExecutionKind {
    /// Instantaneous forward or inverse velocity/mass jump.
    Impulse,
    /// One uninterrupted arc under a finite burn.
    FiniteBurnArc,
}

/// Auditable record of one applied impulse or propagated burn arc.
#[derive(Debug, Clone, PartialEq)]
pub struct ManeuverExecution {
    name: Box<str>,
    kind: ManeuverExecutionKind,
    start: Epoch,
    end: Epoch,
    mass_before: Mass,
    mass_after: Mass,
}

impl ManeuverExecution {
    /// Returns the maneuver name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns whether this record is an impulse or finite-burn arc.
    #[must_use]
    pub const fn kind(&self) -> ManeuverExecutionKind {
        self.kind
    }

    /// Returns the execution start epoch.
    #[must_use]
    pub const fn start(&self) -> Epoch {
        self.start
    }

    /// Returns the execution end epoch.
    #[must_use]
    pub const fn end(&self) -> Epoch {
        self.end
    }

    /// Returns mass immediately before this propagation-order execution.
    #[must_use]
    pub const fn mass_before(&self) -> Mass {
        self.mass_before
    }

    /// Returns mass immediately after this propagation-order execution.
    #[must_use]
    pub const fn mass_after(&self) -> Mass {
        self.mass_after
    }
}

/// Final mass-qualified state and deterministic execution log.
#[derive(Debug, Clone, PartialEq)]
pub struct ManeuverPropagation {
    final_state: CartesianMassState,
    executions: Vec<ManeuverExecution>,
}

impl ManeuverPropagation {
    /// Returns the final state.
    #[must_use]
    pub const fn final_state(&self) -> &CartesianMassState {
        &self.final_state
    }

    /// Returns executions in propagation order.
    #[must_use]
    pub fn executions(&self) -> &[ManeuverExecution] {
        &self.executions
    }

    /// Consumes the result into its state and execution log.
    #[must_use]
    pub fn into_parts(self) -> (CartesianMassState, Vec<ManeuverExecution>) {
        (self.final_state, self.executions)
    }
}

impl<P> BogackiShampine32<P>
where
    P: CartesianDynamics,
{
    /// Propagates a mass-qualified Cartesian state through a maneuver schedule.
    ///
    /// At an initial or target epoch shared with an impulse, the input is
    /// interpreted on the pre-impulse side for forward propagation and the
    /// post-impulse side for reverse propagation. A zero-duration request is
    /// identity and applies no impulse.
    pub fn propagate_with_maneuvers(
        &self,
        initial: CartesianMassState,
        target: Epoch,
        schedule: &ManeuverSchedule,
    ) -> Result<ManeuverPropagation, ManeuverPropagationError<P::Error>> {
        let (mut orbit, mut mass) = initial.into_parts();
        let initial_epoch = orbit.epoch();
        if target == initial_epoch {
            return Ok(ManeuverPropagation {
                final_state: CartesianMassState { orbit, mass },
                executions: Vec::new(),
            });
        }
        self.problem()
            .validate(orbit.as_ref())
            .map_err(ManeuverPropagationError::Dynamics)?;
        let direction = if target > initial_epoch { 1.0 } else { -1.0 };
        let frame = orbit.as_ref().frame();
        validate_schedule_frame(schedule, frame)?;
        let boundaries = schedule.boundary_epochs(initial_epoch, target);
        let mut executions = Vec::new();
        let mut current_epoch = initial_epoch;

        for next_epoch in boundaries {
            apply_impulses(
                schedule,
                current_epoch,
                direction,
                &mut orbit,
                &mut mass,
                &mut executions,
            )?;
            if next_epoch == current_epoch {
                continue;
            }

            let active_burn = schedule.active_burn(current_epoch, next_epoch);
            let mass_before = mass;
            let next_mass = mass_after_interval(mass, current_epoch, next_epoch, active_burn)?;
            let dynamics = ManeuverDynamics {
                base: self.problem(),
                burn: active_burn,
                reference_epoch: current_epoch,
                reference_mass: mass,
            };
            let segment_propagator = BogackiShampine32::new(dynamics, self.configuration());
            orbit = segment_propagator
                .propagate(orbit, next_epoch)
                .map_err(ManeuverPropagationError::Numerical)?;
            mass = next_mass;
            if let Some(burn) = active_burn {
                executions.push(ManeuverExecution {
                    name: burn.name.clone(),
                    kind: ManeuverExecutionKind::FiniteBurnArc,
                    start: current_epoch,
                    end: next_epoch,
                    mass_before,
                    mass_after: mass,
                });
            }
            current_epoch = next_epoch;
        }

        apply_impulses(
            schedule,
            target,
            direction,
            &mut orbit,
            &mut mass,
            &mut executions,
        )?;
        Ok(ManeuverPropagation {
            final_state: CartesianMassState { orbit, mass },
            executions,
        })
    }
}

#[cfg(feature = "attitude")]
impl<P> BogackiShampine32<P>
where
    P: CartesianDynamics,
{
    /// Propagates maneuvers with body-fixed thrust resolved by `attitude_provider`.
    ///
    /// Reference-frame burns retain their existing behavior. For every
    /// numerical stage inside a body-fixed burn, the provider is evaluated at
    /// that stage's complete epoch-qualified Cartesian orbit, and its
    /// body-to-reference orientation rotates the thrust before `F / m` is
    /// added to the base acceleration. Provider errors retain their source.
    pub fn propagate_with_attitude_maneuvers<A>(
        &self,
        initial: CartesianMassState,
        target: Epoch,
        schedule: &ManeuverSchedule,
        attitude_provider: &A,
    ) -> Result<ManeuverPropagation, AttitudeManeuverPropagationError<P::Error, A::Error>>
    where
        A: AttitudeProvider<CartesianState>,
    {
        let (mut orbit, mut mass) = initial.into_parts();
        let initial_epoch = orbit.epoch();
        if target == initial_epoch {
            return Ok(ManeuverPropagation {
                final_state: CartesianMassState { orbit, mass },
                executions: Vec::new(),
            });
        }
        self.problem()
            .validate(orbit.as_ref())
            .map_err(AttitudeManeuverPropagationError::Dynamics)?;
        let direction = if target > initial_epoch { 1.0 } else { -1.0 };
        let frame = orbit.as_ref().frame();
        validate_attitude_schedule_frame(schedule, frame)?;
        let boundaries = schedule.boundary_epochs(initial_epoch, target);
        let mut executions = Vec::new();
        let mut current_epoch = initial_epoch;

        for next_epoch in boundaries {
            apply_attitude_impulses(
                schedule,
                current_epoch,
                direction,
                &mut orbit,
                &mut mass,
                &mut executions,
            )?;
            if next_epoch == current_epoch {
                continue;
            }

            let active_burn = schedule.active_burn(current_epoch, next_epoch);
            let mass_before = mass;
            let next_mass =
                attitude_mass_after_interval(mass, current_epoch, next_epoch, active_burn)?;
            let dynamics = AttitudeManeuverDynamics {
                base: self.problem(),
                burn: active_burn,
                attitude_provider,
                reference_epoch: current_epoch,
                reference_mass: mass,
            };
            let segment_propagator = BogackiShampine32::new(dynamics, self.configuration());
            orbit = segment_propagator
                .propagate(orbit, next_epoch)
                .map_err(AttitudeManeuverPropagationError::Numerical)?;
            mass = next_mass;
            if let Some(burn) = active_burn {
                executions.push(ManeuverExecution {
                    name: burn.name.clone(),
                    kind: ManeuverExecutionKind::FiniteBurnArc,
                    start: current_epoch,
                    end: next_epoch,
                    mass_before,
                    mass_after: mass,
                });
            }
            current_epoch = next_epoch;
        }

        apply_attitude_impulses(
            schedule,
            target,
            direction,
            &mut orbit,
            &mut mass,
            &mut executions,
        )?;
        Ok(ManeuverPropagation {
            final_state: CartesianMassState { orbit, mass },
            executions,
        })
    }
}

#[cfg(feature = "attitude")]
fn apply_attitude_impulses<E, A>(
    schedule: &ManeuverSchedule,
    epoch: Epoch,
    direction: f64,
    orbit: &mut Orbit<CartesianState>,
    mass: &mut Mass,
    executions: &mut Vec<ManeuverExecution>,
) -> Result<(), AttitudeManeuverPropagationError<E, A>>
where
    E: Error + Send + Sync + 'static,
    A: Error + Send + Sync + 'static,
{
    for impulse in schedule
        .impulses
        .iter()
        .filter(|impulse| impulse.epoch == epoch)
    {
        let mass_before = *mass;
        let propellant = impulse.propellant_mass.get::<kilogram>();
        let mass_after_kg = mass_before.get::<kilogram>() - direction * propellant;
        if !mass_after_kg.is_finite() || mass_after_kg <= 0.0 {
            return Err(AttitudeManeuverPropagationError::MassExhausted {
                maneuver: impulse.name.clone(),
                epoch,
            });
        }
        let velocity = orbit.as_ref().velocity();
        let signed_delta = impulse.delta_velocity.to_metres_per_second().map(|value| {
            if direction > 0.0 {
                value
            } else {
                -value
            }
        });
        let [vx, vy, vz] = velocity.to_metres_per_second();
        let state = CartesianState::new(
            orbit.as_ref().frame(),
            orbit.as_ref().position(),
            VelocityVector::from_metres_per_second(
                vx + signed_delta[0],
                vy + signed_delta[1],
                vz + signed_delta[2],
            ),
        )
        .map_err(|_| AttitudeManeuverPropagationError::NonFiniteManeuverState)?;
        *mass = Mass::new::<kilogram>(mass_after_kg);
        *orbit = Orbit::new(epoch, state);
        executions.push(ManeuverExecution {
            name: impulse.name.clone(),
            kind: ManeuverExecutionKind::Impulse,
            start: epoch,
            end: epoch,
            mass_before,
            mass_after: *mass,
        });
    }
    Ok(())
}

#[cfg(feature = "attitude")]
fn attitude_mass_after_interval<E, A>(
    mass: Mass,
    start: Epoch,
    end: Epoch,
    burn: Option<&ConstantThrustManeuver>,
) -> Result<Mass, AttitudeManeuverPropagationError<E, A>>
where
    E: Error + Send + Sync + 'static,
    A: Error + Send + Sync + 'static,
{
    let Some(burn) = burn else {
        return Ok(mass);
    };
    let mass_kg = mass.get::<kilogram>()
        - burn.mass_flow_rate.get::<kilogram_per_second>() * (end - start).to_seconds();
    if !mass_kg.is_finite() || mass_kg <= 0.0 {
        return Err(AttitudeManeuverPropagationError::MassExhausted {
            maneuver: burn.name.clone(),
            epoch: end,
        });
    }
    Ok(Mass::new::<kilogram>(mass_kg))
}

#[cfg(feature = "attitude")]
fn validate_attitude_schedule_frame<E, A>(
    schedule: &ManeuverSchedule,
    frame: ReferenceFrame,
) -> Result<(), AttitudeManeuverPropagationError<E, A>>
where
    E: Error + Send + Sync + 'static,
    A: Error + Send + Sync + 'static,
{
    for impulse in &schedule.impulses {
        if impulse.frame != frame {
            return Err(AttitudeManeuverPropagationError::FrameMismatch {
                maneuver: impulse.name.clone(),
                maneuver_frame: Box::new(impulse.frame),
                state_frame: Box::new(frame),
            });
        }
    }
    for burn in &schedule.finite_burns {
        if let ThrustFrame::Reference(burn_frame) = &burn.thrust_frame {
            if *burn_frame != frame {
                return Err(AttitudeManeuverPropagationError::FrameMismatch {
                    maneuver: burn.name.clone(),
                    maneuver_frame: Box::new(*burn_frame),
                    state_frame: Box::new(frame),
                });
            }
        }
    }
    Ok(())
}

fn apply_impulses<E>(
    schedule: &ManeuverSchedule,
    epoch: Epoch,
    direction: f64,
    orbit: &mut Orbit<CartesianState>,
    mass: &mut Mass,
    executions: &mut Vec<ManeuverExecution>,
) -> Result<(), ManeuverPropagationError<E>>
where
    E: Error + Send + Sync + 'static,
{
    for impulse in schedule
        .impulses
        .iter()
        .filter(|impulse| impulse.epoch == epoch)
    {
        let mass_before = *mass;
        let propellant = impulse.propellant_mass.get::<kilogram>();
        let mass_after_kg = mass_before.get::<kilogram>() - direction * propellant;
        if !mass_after_kg.is_finite() || mass_after_kg <= 0.0 {
            return Err(ManeuverPropagationError::MassExhausted {
                maneuver: impulse.name.clone(),
                epoch,
            });
        }
        let velocity = orbit.as_ref().velocity();
        let signed_delta = impulse.delta_velocity.to_metres_per_second().map(|value| {
            if direction > 0.0 {
                value
            } else {
                -value
            }
        });
        let [vx, vy, vz] = velocity.to_metres_per_second();
        let state = CartesianState::new(
            orbit.as_ref().frame(),
            orbit.as_ref().position(),
            VelocityVector::from_metres_per_second(
                vx + signed_delta[0],
                vy + signed_delta[1],
                vz + signed_delta[2],
            ),
        )
        .map_err(|_| ManeuverPropagationError::NonFiniteManeuverState)?;
        *mass = Mass::new::<kilogram>(mass_after_kg);
        *orbit = Orbit::new(epoch, state);
        executions.push(ManeuverExecution {
            name: impulse.name.clone(),
            kind: ManeuverExecutionKind::Impulse,
            start: epoch,
            end: epoch,
            mass_before,
            mass_after: *mass,
        });
    }
    Ok(())
}

fn mass_after_interval<E>(
    mass: Mass,
    start: Epoch,
    end: Epoch,
    burn: Option<&ConstantThrustManeuver>,
) -> Result<Mass, ManeuverPropagationError<E>>
where
    E: Error + Send + Sync + 'static,
{
    let Some(burn) = burn else {
        return Ok(mass);
    };
    let mass_kg = mass.get::<kilogram>()
        - burn.mass_flow_rate.get::<kilogram_per_second>() * (end - start).to_seconds();
    if !mass_kg.is_finite() || mass_kg <= 0.0 {
        return Err(ManeuverPropagationError::MassExhausted {
            maneuver: burn.name.clone(),
            epoch: end,
        });
    }
    Ok(Mass::new::<kilogram>(mass_kg))
}

fn validate_schedule_frame<E>(
    schedule: &ManeuverSchedule,
    frame: ReferenceFrame,
) -> Result<(), ManeuverPropagationError<E>>
where
    E: Error + Send + Sync + 'static,
{
    for impulse in &schedule.impulses {
        if impulse.frame != frame {
            return Err(ManeuverPropagationError::FrameMismatch {
                maneuver: impulse.name.clone(),
                maneuver_frame: Box::new(impulse.frame),
                state_frame: Box::new(frame),
            });
        }
    }
    for burn in &schedule.finite_burns {
        match &burn.thrust_frame {
            ThrustFrame::Reference(burn_frame) if *burn_frame != frame => {
                return Err(ManeuverPropagationError::FrameMismatch {
                    maneuver: burn.name.clone(),
                    maneuver_frame: Box::new(*burn_frame),
                    state_frame: Box::new(frame),
                });
            }
            ThrustFrame::Body(_) => {
                return Err(ManeuverPropagationError::AttitudeProviderRequired {
                    maneuver: burn.name.clone(),
                });
            }
            ThrustFrame::Reference(_) => {}
        }
    }
    Ok(())
}

#[derive(Debug)]
struct ManeuverDynamics<'a, P> {
    base: &'a P,
    burn: Option<&'a ConstantThrustManeuver>,
    reference_epoch: Epoch,
    reference_mass: Mass,
}

impl<P> CartesianDynamics for ManeuverDynamics<'_, P>
where
    P: CartesianDynamics,
{
    type Error = ManeuverDynamicsError<P::Error>;

    fn validate(&self, state: &CartesianState) -> Result<(), Self::Error> {
        self.base
            .validate(state)
            .map_err(ManeuverDynamicsError::Dynamics)?;
        if let Some(burn) = self.burn {
            if burn.frame() != state.frame() {
                return Err(ManeuverDynamicsError::FrameMismatch);
            }
        }
        Ok(())
    }

    fn acceleration(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, Self::Error> {
        let base = self
            .base
            .acceleration(epoch, state)
            .map_err(ManeuverDynamicsError::Dynamics)?;
        if base.frame() != state.frame() {
            return Err(ManeuverDynamicsError::BaseAccelerationFrameMismatch);
        }
        let Some(burn) = self.burn else {
            return Ok(base);
        };
        let mass_kg = self.reference_mass.get::<kilogram>()
            - burn.mass_flow_rate.get::<kilogram_per_second>()
                * (epoch - self.reference_epoch).to_seconds();
        if !mass_kg.is_finite() || mass_kg <= 0.0 {
            return Err(ManeuverDynamicsError::NonPositiveMass);
        }
        let [base_x, base_y, base_z] = base.value().to_metres_per_second_squared();
        let [thrust_x, thrust_y, thrust_z] = burn.thrust.to_newtons();
        FramedAcceleration::new(
            AccelerationVector::from_metres_per_second_squared(
                base_x + thrust_x / mass_kg,
                base_y + thrust_y / mass_kg,
                base_z + thrust_z / mass_kg,
            ),
            state.frame(),
        )
        .map_err(|_| ManeuverDynamicsError::NonFiniteAcceleration)
    }
}

#[cfg(feature = "attitude")]
#[derive(Debug)]
struct AttitudeManeuverDynamics<'a, P, A> {
    base: &'a P,
    burn: Option<&'a ConstantThrustManeuver>,
    attitude_provider: &'a A,
    reference_epoch: Epoch,
    reference_mass: Mass,
}

#[cfg(feature = "attitude")]
impl<P, A> CartesianDynamics for AttitudeManeuverDynamics<'_, P, A>
where
    P: CartesianDynamics,
    A: AttitudeProvider<CartesianState>,
{
    type Error = AttitudeManeuverDynamicsError<P::Error, A::Error>;

    fn validate(&self, state: &CartesianState) -> Result<(), Self::Error> {
        self.base
            .validate(state)
            .map_err(AttitudeManeuverDynamicsError::Dynamics)?;
        if let Some(ConstantThrustManeuver {
            thrust_frame: ThrustFrame::Reference(frame),
            ..
        }) = self.burn
        {
            if *frame != state.frame() {
                return Err(AttitudeManeuverDynamicsError::FrameMismatch);
            }
        }
        Ok(())
    }

    fn acceleration(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, Self::Error> {
        let base = self
            .base
            .acceleration(epoch, state)
            .map_err(AttitudeManeuverDynamicsError::Dynamics)?;
        if base.frame() != state.frame() {
            return Err(AttitudeManeuverDynamicsError::BaseAccelerationFrameMismatch);
        }
        let Some(burn) = self.burn else {
            return Ok(base);
        };
        let mass_kg = self.reference_mass.get::<kilogram>()
            - burn.mass_flow_rate.get::<kilogram_per_second>()
                * (epoch - self.reference_epoch).to_seconds();
        if !mass_kg.is_finite() || mass_kg <= 0.0 {
            return Err(AttitudeManeuverDynamicsError::NonPositiveMass);
        }

        let thrust = match &burn.thrust_frame {
            ThrustFrame::Reference(frame) => {
                if *frame != state.frame() {
                    return Err(AttitudeManeuverDynamicsError::FrameMismatch);
                }
                burn.thrust.components()
            }
            ThrustFrame::Body(body_frame) => {
                let stage_orbit = Orbit::new(epoch, *state);
                let attitude = self
                    .attitude_provider
                    .attitude(&stage_orbit)
                    .map_err(AttitudeManeuverDynamicsError::AttitudeProvider)?;
                if attitude.angular_velocity().body_frame_capability() != body_frame
                    || attitude.orientation().source_frame() != body_frame.reference_frame()
                {
                    return Err(AttitudeManeuverDynamicsError::ProviderBodyFrameMismatch);
                }
                if attitude.orientation().target_frame() != state.frame() {
                    return Err(AttitudeManeuverDynamicsError::ProviderReferenceFrameMismatch);
                }
                let body_force =
                    FramedForce::new(burn.thrust.components(), body_frame.reference_frame())
                        .map_err(|_| AttitudeManeuverDynamicsError::NonFiniteAcceleration)?;
                attitude
                    .orientation()
                    .rotate_force(body_force)
                    .map_err(AttitudeManeuverDynamicsError::ForceRotation)?
                    .components()
            }
        };

        let [base_x, base_y, base_z] = base.value().to_metres_per_second_squared();
        let [thrust_x, thrust_y, thrust_z] = thrust.map(|component| component.get::<newton>());
        FramedAcceleration::new(
            AccelerationVector::from_metres_per_second_squared(
                base_x + thrust_x / mass_kg,
                base_y + thrust_y / mass_kg,
                base_z + thrust_z / mass_kg,
            ),
            state.frame(),
        )
        .map_err(|_| AttitudeManeuverDynamicsError::NonFiniteAcceleration)
    }
}

/// Invalid maneuver or mass-state configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ManeuverConfigurationError {
    /// A diagnostic maneuver name is blank.
    #[error("maneuver name must not be blank")]
    BlankName,
    /// State mass is NaN or infinite.
    #[error("spacecraft mass must be finite")]
    NonFiniteMass,
    /// State mass is zero or negative.
    #[error("spacecraft mass must be strictly positive")]
    NonPositiveMass,
    /// Propellant consumption is NaN, infinite, or negative.
    #[error("impulsive propellant mass must be finite and non-negative")]
    InvalidPropellantMass,
    /// A velocity-jump component is NaN or infinite.
    #[error("impulsive delta-velocity components must be finite")]
    NonFiniteDeltaVelocity,
    /// Finite-burn end does not follow its start.
    #[error("finite-burn end epoch must be later than its start")]
    InvalidFiniteInterval,
    /// A thrust component is NaN or infinite.
    #[error("thrust components must be finite")]
    NonFiniteThrust,
    /// All thrust components are zero.
    #[error("finite-burn thrust vector must be non-zero")]
    ZeroThrust,
    /// Mass flow is not positive and finite.
    #[error("finite-burn mass-flow rate must be positive and finite")]
    InvalidMassFlowRate,
    /// Two finite burns occupy a common open interval.
    #[error("finite burns must not overlap")]
    OverlappingFiniteBurns,
}

fn validate_name(name: &str) -> Result<(), ManeuverConfigurationError> {
    if name.trim().is_empty() {
        Err(ManeuverConfigurationError::BlankName)
    } else {
        Ok(())
    }
}

fn validate_positive_mass(mass: Mass) -> Result<(), ManeuverConfigurationError> {
    let mass_kg = mass.get::<kilogram>();
    if !mass_kg.is_finite() {
        Err(ManeuverConfigurationError::NonFiniteMass)
    } else if mass_kg <= 0.0 {
        Err(ManeuverConfigurationError::NonPositiveMass)
    } else {
        Ok(())
    }
}

fn validate_non_negative_mass(mass: Mass) -> Result<(), ManeuverConfigurationError> {
    let mass_kg = mass.get::<kilogram>();
    if !mass_kg.is_finite() || mass_kg < 0.0 {
        Err(ManeuverConfigurationError::InvalidPropellantMass)
    } else {
        Ok(())
    }
}

/// Failure while evaluating base dynamics plus an active finite burn.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ManeuverDynamicsError<E>
where
    E: Error + Send + Sync + 'static,
{
    /// The caller-selected base dynamics failed.
    #[error("base Cartesian dynamics evaluation failed")]
    Dynamics(#[source] E),
    /// The finite burn and Cartesian state use different frames.
    #[error("finite-burn frame does not match the Cartesian state frame")]
    FrameMismatch,
    /// Base dynamics returned acceleration in another frame during a burn.
    #[error("base acceleration frame does not match the Cartesian state frame")]
    BaseAccelerationFrameMismatch,
    /// A numerical stage reached zero or negative spacecraft mass.
    #[error("finite-burn stage reached non-positive spacecraft mass")]
    NonPositiveMass,
    /// Adding thrust to base dynamics produced non-finite acceleration.
    #[error("combined base and thrust acceleration is non-finite")]
    NonFiniteAcceleration,
}

/// Failure while evaluating base dynamics plus attitude-resolved thrust.
#[cfg(feature = "attitude")]
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AttitudeManeuverDynamicsError<E, A>
where
    E: Error + Send + Sync + 'static,
    A: Error + Send + Sync + 'static,
{
    /// The caller-selected base dynamics failed.
    #[error("base Cartesian dynamics evaluation failed")]
    Dynamics(#[source] E),
    /// The attitude provider failed at a numerical stage.
    #[error("attitude-provider evaluation failed during finite burn")]
    AttitudeProvider(#[source] A),
    /// A reference-frame burn and Cartesian state use different frames.
    #[error("finite-burn frame does not match the Cartesian state frame")]
    FrameMismatch,
    /// The provider returned another spacecraft's body axes.
    #[error("attitude-provider body frame does not match the body-fixed thrust frame")]
    ProviderBodyFrameMismatch,
    /// The provider returned an orientation relative to another frame.
    #[error("attitude-provider reference frame does not match the Cartesian state frame")]
    ProviderReferenceFrameMismatch,
    /// The provider orientation could not rotate the body-qualified force.
    #[error("body-fixed thrust rotation failed")]
    ForceRotation(#[source] OrientationForceError),
    /// Base dynamics returned acceleration in another frame during a burn.
    #[error("base acceleration frame does not match the Cartesian state frame")]
    BaseAccelerationFrameMismatch,
    /// A numerical stage reached zero or negative spacecraft mass.
    #[error("finite-burn stage reached non-positive spacecraft mass")]
    NonPositiveMass,
    /// Adding rotated thrust to base dynamics produced non-finite acceleration.
    #[error("combined base and attitude-resolved thrust acceleration is non-finite")]
    NonFiniteAcceleration,
}

/// Maneuver-aware propagation failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ManeuverPropagationError<E>
where
    E: Error + Send + Sync + 'static,
{
    /// Initial base-dynamics validation failed.
    #[error("Cartesian dynamics validation failed")]
    Dynamics(#[source] E),
    /// A maneuver is expressed in another frame.
    #[error("maneuver {maneuver} frame {maneuver_frame} does not match state frame {state_frame}")]
    FrameMismatch {
        /// Maneuver diagnostic name.
        maneuver: Box<str>,
        /// Frame of its delta-velocity or thrust.
        maneuver_frame: Box<ReferenceFrame>,
        /// Propagated Cartesian frame.
        state_frame: Box<ReferenceFrame>,
    },
    /// A body-fixed finite burn was evaluated without an attitude provider.
    #[error("body-fixed finite burn {maneuver} requires an attitude provider")]
    AttitudeProviderRequired {
        /// Maneuver diagnostic name.
        maneuver: Box<str>,
    },
    /// Forward execution would consume all remaining mass.
    #[error("maneuver {maneuver} exhausts spacecraft mass at {epoch}")]
    MassExhausted {
        /// Maneuver diagnostic name.
        maneuver: Box<str>,
        /// First detected non-positive-mass epoch.
        epoch: Epoch,
    },
    /// Applying an impulse produced a non-finite Cartesian state.
    #[error("impulsive maneuver produced a non-finite Cartesian state")]
    NonFiniteManeuverState,
    /// The adaptive segment propagator failed.
    #[error("maneuver propagation segment failed")]
    Numerical(#[source] NumericalPropagationError<ManeuverDynamicsError<E>>),
}

/// Attitude-resolved maneuver propagation failure.
#[cfg(feature = "attitude")]
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AttitudeManeuverPropagationError<E, A>
where
    E: Error + Send + Sync + 'static,
    A: Error + Send + Sync + 'static,
{
    /// Initial base-dynamics validation failed.
    #[error("Cartesian dynamics validation failed")]
    Dynamics(#[source] E),
    /// A reference-frame maneuver is expressed in another frame.
    #[error("maneuver {maneuver} frame {maneuver_frame} does not match state frame {state_frame}")]
    FrameMismatch {
        /// Maneuver diagnostic name.
        maneuver: Box<str>,
        /// Frame of its delta-velocity or thrust.
        maneuver_frame: Box<ReferenceFrame>,
        /// Propagated Cartesian frame.
        state_frame: Box<ReferenceFrame>,
    },
    /// Forward execution would consume all remaining mass.
    #[error("maneuver {maneuver} exhausts spacecraft mass at {epoch}")]
    MassExhausted {
        /// Maneuver diagnostic name.
        maneuver: Box<str>,
        /// First detected non-positive-mass epoch.
        epoch: Epoch,
    },
    /// Applying an impulse produced a non-finite Cartesian state.
    #[error("impulsive maneuver produced a non-finite Cartesian state")]
    NonFiniteManeuverState,
    /// The adaptive attitude-resolved segment propagator failed.
    #[error("attitude-resolved maneuver propagation segment failed")]
    Numerical(#[source] NumericalPropagationError<AttitudeManeuverDynamicsError<E, A>>),
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    #[cfg(feature = "attitude")]
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[cfg(feature = "attitude")]
    use attitude::{
        AttitudeProvider, AttitudeSample, FixedAttitudeProvider, FixedAttitudeProviderError,
        TabulatedAttitudeProvider, TabulatedAttitudeProviderError,
    };
    use hifitime::Unit;
    #[cfg(feature = "attitude")]
    use orskit_core::frames::{CustomFrameId, FrameMotion, FrameOrientation, FrameOrigin};
    #[cfg(feature = "attitude")]
    use orskit_core::{
        BodyAngularVelocity, Orientation, OrientationQuaternion, QuaternionAttitude,
    };
    use units::uom::si::{mass::kilogram, mass_rate::kilogram_per_second, ratio::ratio};
    use units::{Length, Position, Ratio, Velocity};

    use super::*;
    use crate::IntegrationConfiguration;

    #[derive(Debug, Clone, Copy)]
    struct ZeroDynamics;

    impl CartesianDynamics for ZeroDynamics {
        type Error = Infallible;

        fn validate(&self, _state: &CartesianState) -> Result<(), Self::Error> {
            Ok(())
        }

        fn acceleration(
            &self,
            _epoch: Epoch,
            state: &CartesianState,
        ) -> Result<FramedAcceleration, Self::Error> {
            Ok(FramedAcceleration::new(
                AccelerationVector::from_metres_per_second_squared(0.0, 0.0, 0.0),
                state.frame(),
            )
            .expect("finite zero acceleration"))
        }
    }

    fn propagator() -> BogackiShampine32<ZeroDynamics> {
        BogackiShampine32::new(
            ZeroDynamics,
            IntegrationConfiguration::new(
                Length::new::<units::uom::si::length::meter>(1.0e-9),
                Velocity::new::<units::uom::si::velocity::meter_per_second>(1.0e-12),
                Ratio::new::<ratio>(1.0e-12),
                1.0 * Unit::Millisecond,
                10.0 * Unit::Second,
                1.0 * Unit::Second,
                10_000,
                1_000,
            )
            .expect("valid integration configuration"),
        )
    }

    fn initial(epoch: Epoch, mass_kg: f64) -> CartesianMassState {
        CartesianMassState::new(
            Orbit::new(
                epoch,
                CartesianState::new(
                    ReferenceFrame::GCRF,
                    Position::from_metres(0.0, 0.0, 0.0),
                    VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
                )
                .expect("finite state"),
            ),
            Mass::new::<kilogram>(mass_kg),
        )
        .expect("positive mass")
    }

    #[cfg(feature = "attitude")]
    fn body(id: u64, spacecraft_id: &str) -> SpacecraftBodyFrame {
        let id = CustomFrameId::new(id);
        SpacecraftBodyFrame::new(
            spacecraft_id.to_owned(),
            ReferenceFrame::new(
                FrameOrigin::Custom(id),
                FrameOrientation::custom(id, FrameMotion::NonInertial),
            ),
        )
        .expect("spacecraft-owned body frame")
    }

    #[cfg(feature = "attitude")]
    fn quaternion_attitude(
        body: &SpacecraftBodyFrame,
        components: [f64; 4],
        target: ReferenceFrame,
    ) -> QuaternionAttitude {
        let orientation = Orientation::try_from(OrientationQuaternion {
            source_frame: body.reference_frame(),
            target_frame: target,
            components: components.map(Ratio::new::<ratio>),
        })
        .expect("valid orientation");
        let rate = BodyAngularVelocity::new(
            units::AngularVelocityVector::from_radians_per_second(0.0, 0.0, 0.0),
            body.clone(),
            target,
        )
        .expect("valid body rate");
        QuaternionAttitude::new(orientation, rate).expect("consistent attitude")
    }

    #[cfg(feature = "attitude")]
    #[derive(Debug)]
    struct CountingProvider {
        fixed: FixedAttitudeProvider,
        calls: AtomicUsize,
    }

    #[cfg(feature = "attitude")]
    impl AttitudeProvider<CartesianState> for CountingProvider {
        type Attitude = QuaternionAttitude;
        type Error = FixedAttitudeProviderError;

        fn attitude(&self, orbit: &Orbit<CartesianState>) -> Result<Self::Attitude, Self::Error> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.fixed.attitude(orbit)
        }
    }

    #[test]
    fn impulse_is_reversible_with_mass_and_execution_audit() {
        let epoch = Epoch::from_tai_seconds(1_000.0);
        let impulse = ImpulsiveManeuver::new(
            "TCM-1",
            epoch + 10.0 * Unit::Second,
            ReferenceFrame::GCRF,
            VelocityVector::from_metres_per_second(3.0, -2.0, 1.0),
            Mass::new::<kilogram>(2.0),
        )
        .expect("valid impulse");
        let schedule = ManeuverSchedule::new(vec![impulse], vec![]).expect("valid schedule");
        let forward = propagator()
            .propagate_with_maneuvers(
                initial(epoch, 100.0),
                epoch + 20.0 * Unit::Second,
                &schedule,
            )
            .expect("forward propagation");
        assert_eq!(
            forward
                .final_state()
                .orbit()
                .as_ref()
                .velocity()
                .to_metres_per_second(),
            [3.0, -2.0, 1.0]
        );
        assert_eq!(forward.final_state().mass(), Mass::new::<kilogram>(98.0));
        assert_eq!(forward.executions().len(), 1);
        assert_eq!(
            forward.executions()[0].kind(),
            ManeuverExecutionKind::Impulse
        );

        let backward = propagator()
            .propagate_with_maneuvers(forward.final_state().clone(), epoch, &schedule)
            .expect("reverse propagation");
        assert_eq!(
            backward
                .final_state()
                .orbit()
                .as_ref()
                .velocity()
                .to_metres_per_second(),
            [0.0, 0.0, 0.0]
        );
        assert_eq!(backward.final_state().mass(), Mass::new::<kilogram>(100.0));
    }

    #[test]
    fn impulse_at_initial_epoch_is_applied_once_and_zero_duration_is_identity() {
        let epoch = Epoch::from_tai_seconds(1_500.0);
        let impulse = ImpulsiveManeuver::new(
            "initial trim",
            epoch,
            ReferenceFrame::GCRF,
            VelocityVector::from_metres_per_second(1.0, 0.0, 0.0),
            Mass::new::<kilogram>(0.5),
        )
        .expect("valid impulse");
        let schedule = ManeuverSchedule::new(vec![impulse], vec![]).expect("valid schedule");
        let identity = propagator()
            .propagate_with_maneuvers(initial(epoch, 10.0), epoch, &schedule)
            .expect("zero-duration identity");
        assert!(identity.executions().is_empty());
        assert_eq!(identity.final_state().mass(), Mass::new::<kilogram>(10.0));

        let propagated = propagator()
            .propagate_with_maneuvers(initial(epoch, 10.0), epoch + 1.0 * Unit::Second, &schedule)
            .expect("initial impulse propagation");
        assert_eq!(propagated.executions().len(), 1);
        assert_eq!(
            propagated
                .final_state()
                .orbit()
                .as_ref()
                .velocity()
                .to_metres_per_second(),
            [1.0, 0.0, 0.0]
        );
        assert_eq!(propagated.final_state().mass(), Mass::new::<kilogram>(9.5));
    }

    #[test]
    fn constant_thrust_matches_analytic_variable_mass_solution() {
        let epoch = Epoch::from_tai_seconds(2_000.0);
        let burn = ConstantThrustManeuver::new(
            "finite-1",
            epoch,
            epoch + 10.0 * Unit::Second,
            ReferenceFrame::GCRF,
            ThrustVector::from_newtons(100.0, 0.0, 0.0),
            MassRate::new::<kilogram_per_second>(1.0),
        )
        .expect("valid finite burn");
        let schedule = ManeuverSchedule::new(vec![], vec![burn]).expect("valid schedule");
        let result = propagator()
            .propagate_with_maneuvers(
                initial(epoch, 100.0),
                epoch + 10.0 * Unit::Second,
                &schedule,
            )
            .expect("finite burn propagation");
        let final_state = result.final_state();
        let mass_ratio = 100.0_f64 / 90.0;
        let expected_velocity = 100.0 * mass_ratio.ln();
        let expected_position = 100.0 * (10.0 - 90.0 * mass_ratio.ln());
        let [x, _, _] = final_state.orbit().as_ref().position().to_metres();
        let [vx, _, _] = final_state
            .orbit()
            .as_ref()
            .velocity()
            .to_metres_per_second();
        assert!((vx - expected_velocity).abs() < 2.0e-8);
        assert!((x - expected_position).abs() < 2.0e-7);
        assert_eq!(final_state.mass(), Mass::new::<kilogram>(90.0));
        assert_eq!(result.executions().len(), 1);
        assert_eq!(
            result.executions()[0].kind(),
            ManeuverExecutionKind::FiniteBurnArc
        );
    }

    #[test]
    fn constant_thrust_round_trip_recovers_mass_and_state() {
        let epoch = Epoch::from_tai_seconds(2_500.0);
        let burn = ConstantThrustManeuver::new(
            "reversible finite",
            epoch,
            epoch + 10.0 * Unit::Second,
            ReferenceFrame::GCRF,
            ThrustVector::from_newtons(100.0, 0.0, 0.0),
            MassRate::new::<kilogram_per_second>(1.0),
        )
        .expect("valid finite burn");
        let schedule = ManeuverSchedule::new(vec![], vec![burn]).expect("valid schedule");
        let forward = propagator()
            .propagate_with_maneuvers(
                initial(epoch, 100.0),
                epoch + 10.0 * Unit::Second,
                &schedule,
            )
            .expect("forward finite burn");
        let backward = propagator()
            .propagate_with_maneuvers(forward.final_state().clone(), epoch, &schedule)
            .expect("reverse finite burn");
        let [x, y, z] = backward
            .final_state()
            .orbit()
            .as_ref()
            .position()
            .to_metres();
        let [vx, vy, vz] = backward
            .final_state()
            .orbit()
            .as_ref()
            .velocity()
            .to_metres_per_second();
        assert!(x.abs() < 5.0e-8);
        assert!(y.abs() < 1.0e-12);
        assert!(z.abs() < 1.0e-12);
        assert!(vx.abs() < 5.0e-9);
        assert!(vy.abs() < 1.0e-12);
        assert!(vz.abs() < 1.0e-12);
        assert_eq!(backward.final_state().mass(), Mass::new::<kilogram>(100.0));
    }

    #[test]
    fn finite_burn_and_impulse_execute_in_propagation_order() {
        let epoch = Epoch::from_tai_seconds(3_000.0);
        let burn = ConstantThrustManeuver::new(
            "burn",
            epoch,
            epoch + 10.0 * Unit::Second,
            ReferenceFrame::GCRF,
            ThrustVector::from_newtons(10.0, 0.0, 0.0),
            MassRate::new::<kilogram_per_second>(0.1),
        )
        .expect("valid burn");
        let impulse = ImpulsiveManeuver::new(
            "trim",
            epoch + 5.0 * Unit::Second,
            ReferenceFrame::GCRF,
            VelocityVector::from_metres_per_second(0.0, 1.0, 0.0),
            Mass::new::<kilogram>(1.0),
        )
        .expect("valid impulse");
        let schedule = ManeuverSchedule::new(vec![impulse], vec![burn]).expect("valid schedule");
        let result = propagator()
            .propagate_with_maneuvers(
                initial(epoch, 100.0),
                epoch + 10.0 * Unit::Second,
                &schedule,
            )
            .expect("combined maneuver propagation");
        assert_eq!(
            result
                .executions()
                .iter()
                .map(ManeuverExecution::name)
                .collect::<Vec<_>>(),
            vec!["burn", "trim", "burn"]
        );
        assert!((result.final_state().mass().get::<kilogram>() - 98.0).abs() < 1.0e-12);
    }

    #[test]
    fn schedule_and_mass_failures_are_explicit() {
        let epoch = Epoch::from_tai_seconds(4_000.0);
        let burn = |name, start, end| {
            ConstantThrustManeuver::new(
                name,
                start,
                end,
                ReferenceFrame::GCRF,
                ThrustVector::from_newtons(1.0, 0.0, 0.0),
                MassRate::new::<kilogram_per_second>(1.0),
            )
            .expect("valid burn")
        };
        assert_eq!(
            ManeuverSchedule::new(
                vec![],
                vec![
                    burn("one", epoch, epoch + 10.0 * Unit::Second),
                    burn(
                        "two",
                        epoch + 9.0 * Unit::Second,
                        epoch + 20.0 * Unit::Second,
                    ),
                ],
            ),
            Err(ManeuverConfigurationError::OverlappingFiniteBurns)
        );

        let schedule = ManeuverSchedule::new(
            vec![],
            vec![burn("exhaust", epoch, epoch + 10.0 * Unit::Second)],
        )
        .expect("valid schedule");
        assert!(matches!(
            propagator().propagate_with_maneuvers(
                initial(epoch, 5.0),
                epoch + 10.0 * Unit::Second,
                &schedule,
            ),
            Err(ManeuverPropagationError::MassExhausted { .. })
        ));
    }

    #[test]
    fn maneuver_frames_are_checked_before_execution() {
        let epoch = Epoch::from_tai_seconds(5_000.0);
        let impulse = ImpulsiveManeuver::new(
            "wrong frame",
            epoch + 1.0 * Unit::Second,
            ReferenceFrame::EME2000,
            VelocityVector::from_metres_per_second(1.0, 0.0, 0.0),
            Mass::new::<kilogram>(0.0),
        )
        .expect("valid impulse");
        let schedule = ManeuverSchedule::new(vec![impulse], vec![]).expect("valid schedule");
        assert!(matches!(
            propagator().propagate_with_maneuvers(
                initial(epoch, 10.0),
                epoch + 2.0 * Unit::Second,
                &schedule,
            ),
            Err(ManeuverPropagationError::FrameMismatch { .. })
        ));
    }

    #[cfg(feature = "attitude")]
    #[test]
    fn body_fixed_thrust_is_rotated_at_numerical_stages() {
        let epoch = Epoch::from_tai_seconds(6_000.0);
        let body = body(60, "body-burn");
        let provider = CountingProvider {
            fixed: FixedAttitudeProvider::new(quaternion_attitude(
                &body,
                [
                    std::f64::consts::FRAC_1_SQRT_2,
                    0.0,
                    0.0,
                    std::f64::consts::FRAC_1_SQRT_2,
                ],
                ReferenceFrame::GCRF,
            ))
            .expect("zero-rate fixed attitude"),
            calls: AtomicUsize::new(0),
        };
        let burn = ConstantThrustManeuver::body_fixed(
            "body +x",
            epoch,
            epoch + 10.0 * Unit::Second,
            body,
            ThrustVector::from_newtons(100.0, 0.0, 0.0),
            MassRate::new::<kilogram_per_second>(1.0),
        )
        .expect("valid body-fixed burn");
        let schedule = ManeuverSchedule::new(vec![], vec![burn]).expect("valid schedule");

        let result = propagator()
            .propagate_with_attitude_maneuvers(
                initial(epoch, 100.0),
                epoch + 10.0 * Unit::Second,
                &schedule,
                &provider,
            )
            .expect("attitude-resolved propagation");
        let [vx, vy, vz] = result
            .final_state()
            .orbit()
            .as_ref()
            .velocity()
            .to_metres_per_second();
        let expected = 100.0 * (100.0_f64 / 90.0).ln();
        assert!(vx.abs() < 2.0e-12);
        assert!((vy - expected).abs() < 2.0e-8);
        assert!(vz.abs() < 2.0e-12);
        assert!(
            provider.calls.load(Ordering::Relaxed) > 4,
            "provider must be sampled across Runge--Kutta stages"
        );

        let backward = propagator()
            .propagate_with_attitude_maneuvers(
                result.final_state().clone(),
                epoch,
                &schedule,
                &provider,
            )
            .expect("reverse attitude-resolved propagation");
        let recovered_velocity = backward
            .final_state()
            .orbit()
            .as_ref()
            .velocity()
            .to_metres_per_second();
        assert!(recovered_velocity
            .into_iter()
            .all(|value| value.abs() < 5.0e-9));
        assert_eq!(backward.final_state().mass(), Mass::new::<kilogram>(100.0));
    }

    #[cfg(feature = "attitude")]
    #[test]
    fn body_fixed_thrust_requires_the_attitude_aware_entry_point() {
        let epoch = Epoch::from_tai_seconds(7_000.0);
        let body = body(70, "provider-required");
        let burn = ConstantThrustManeuver::body_fixed(
            "body burn",
            epoch,
            epoch + 1.0 * Unit::Second,
            body,
            ThrustVector::from_newtons(1.0, 0.0, 0.0),
            MassRate::new::<kilogram_per_second>(0.1),
        )
        .expect("valid body burn");
        let schedule = ManeuverSchedule::new(vec![], vec![burn]).expect("valid schedule");

        assert!(matches!(
            propagator().propagate_with_maneuvers(
                initial(epoch, 10.0),
                epoch + 1.0 * Unit::Second,
                &schedule
            ),
            Err(ManeuverPropagationError::AttitudeProviderRequired { .. })
        ));
    }

    #[cfg(feature = "attitude")]
    #[test]
    fn tabulated_provider_coverage_failure_retains_its_typed_source() {
        let epoch = Epoch::from_tai_seconds(8_000.0);
        let body = body(80, "coverage");
        let provider = TabulatedAttitudeProvider::new(vec![AttitudeSample::new(
            epoch,
            quaternion_attitude(&body, [1.0, 0.0, 0.0, 0.0], ReferenceFrame::GCRF),
        )])
        .expect("one-point table");
        let burn = ConstantThrustManeuver::body_fixed(
            "outlive attitude",
            epoch,
            epoch + 1.0 * Unit::Second,
            body,
            ThrustVector::from_newtons(1.0, 0.0, 0.0),
            MassRate::new::<kilogram_per_second>(0.1),
        )
        .expect("valid body burn");
        let schedule = ManeuverSchedule::new(vec![], vec![burn]).expect("valid schedule");

        assert!(matches!(
            propagator().propagate_with_attitude_maneuvers(
                initial(epoch, 10.0),
                epoch + 1.0 * Unit::Second,
                &schedule,
                &provider,
            ),
            Err(AttitudeManeuverPropagationError::Numerical(
                NumericalPropagationError::Dynamics(
                    AttitudeManeuverDynamicsError::AttitudeProvider(
                        TabulatedAttitudeProviderError::OutsideCoverage { .. }
                    )
                )
            ))
        ));
    }
}
