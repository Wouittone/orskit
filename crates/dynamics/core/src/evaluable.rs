//! Object-safe evaluable Cartesian force-model composition.
//!
//! [`EvaluableCartesianForceModel`] adds an object-safe acceleration
//! evaluation capability to the descriptive [`ForceModel`](crate::ForceModel)
//! contract. [`ComposedCartesianDynamics`] assembles an ordered collection of
//! such models into one [`CartesianDynamics`] system, summing their
//! accelerations in declaration order. [`ComposedCartesianVariationalDynamics`]
//! additionally requires every model to supply an acceleration-partial
//! contribution and sums those too, so a composed Jacobian is only ever
//! produced when every constituent model can honestly supply one; see
//! `.agent/decisions/0044-compose-evaluable-cartesian-force-models.md`.
//!
//! [`CartesianDynamicsForceModel`] adapts any existing whole
//! [`CartesianDynamics`] implementation (for example
//! `dynamics_two_bodies::TwoBodyDynamics` or `dynamics_harmonics::J2Dynamics`)
//! into one evaluable force-model contribution, so existing named topologies
//! can be combined with additional force models without rewriting them.

use std::{fmt, sync::Arc};

use hifitime::Epoch;
use orbits::cartesian::{CartesianState, FramedAcceleration};
use thiserror::Error;
use units::{AccelerationVector, InverseTime, InverseTimeSquared};

use crate::{
    CartesianAccelerationJacobian, CartesianDynamics, CartesianVariationalDynamics, Force,
    ForceModel, SpacecraftStateRequirements,
};

/// Erased per-model Cartesian force-model evaluation failure.
///
/// Heterogeneous force models generally have unrelated typed error enums, so
/// an object-safe composition boundary must erase them. This preserves the
/// failing model's name for diagnostics and the original error as
/// [`std::error::Error::source`], per the project's domain-error policy: an
/// error is erased behind `Box<dyn Error + Send + Sync>` only at a genuine
/// object-safe boundary, and a source is never discarded.
#[derive(Debug, Error)]
#[error("Cartesian force model {model_name:?} failed to evaluate")]
pub struct CartesianForceModelError {
    model_name: String,
    #[source]
    source: Box<dyn std::error::Error + Send + Sync + 'static>,
}

impl CartesianForceModelError {
    /// Erases one model's typed evaluation failure behind this boundary.
    pub fn new(
        model_name: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            model_name: model_name.into(),
            source: Box::new(source),
        }
    }

    /// Returns the name of the model that failed to evaluate.
    #[must_use]
    pub fn model_name(&self) -> &str {
        &self.model_name
    }
}

/// Object-safe evaluable Cartesian force-model contract.
///
/// The input state carries its own reference frame and `epoch` identifies the
/// instant of evaluation, matching [`CartesianDynamics`]. Implementations own
/// or borrow every immutable provider they require; this contract performs no
/// ambient data lookup. Errors are erased behind [`CartesianForceModelError`]
/// so heterogeneous model implementations can share one ordered, object-safe
/// collection in [`ComposedCartesianDynamics`].
pub trait EvaluableCartesianForceModel: ForceModel {
    /// Validates the state frame, origin, and model data before evaluation.
    fn validate_cartesian(&self, state: &CartesianState) -> Result<(), CartesianForceModelError>;

    /// Evaluates this model's acceleration contribution at `epoch`,
    /// expressed in the state's frame.
    fn cartesian_acceleration(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, CartesianForceModelError>;
}

/// Object-safe evaluable Cartesian force model with acceleration partials.
///
/// A model implements this only when it can honestly supply an acceleration
/// Jacobian for every state it accepts. [`ComposedCartesianVariationalDynamics`]
/// requires every constituent model to implement this trait, so a composed
/// Jacobian never silently omits a model's contribution.
pub trait EvaluableCartesianVariationalForceModel: EvaluableCartesianForceModel {
    /// Evaluates this model's acceleration-Jacobian contribution.
    fn cartesian_acceleration_jacobian(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<CartesianAccelerationJacobian, CartesianForceModelError>;
}

/// Shared handle for one evaluable Cartesian force-model contribution.
pub type EvaluableCartesianForceModelHandle =
    Arc<dyn EvaluableCartesianForceModel + Send + Sync + 'static>;

/// Shared handle for one evaluable, Jacobian-capable Cartesian force-model
/// contribution.
pub type EvaluableCartesianVariationalForceModelHandle =
    Arc<dyn EvaluableCartesianVariationalForceModel + Send + Sync + 'static>;

/// Adapts one whole [`CartesianDynamics`] implementation into one evaluable
/// force-model contribution.
///
/// This lets an existing strict, named topology (for example a point-mass or
/// point-mass-plus-`J2` system) participate in [`ComposedCartesianDynamics`]
/// as a single ordered contribution, without changing its own public API.
pub struct CartesianDynamicsForceModel<D> {
    model_name: String,
    force: Arc<dyn Force>,
    state_requirements: SpacecraftStateRequirements,
    dynamics: D,
}

impl<D> CartesianDynamicsForceModel<D> {
    /// Adapts `dynamics` into one named, described force-model contribution.
    #[must_use]
    pub fn new(
        model_name: impl Into<String>,
        force: Arc<dyn Force>,
        state_requirements: SpacecraftStateRequirements,
        dynamics: D,
    ) -> Self {
        Self {
            model_name: model_name.into(),
            force,
            state_requirements,
            dynamics,
        }
    }

    /// Returns the adapted dynamics value.
    #[must_use]
    pub const fn dynamics(&self) -> &D {
        &self.dynamics
    }
}

impl<D: fmt::Debug> fmt::Debug for CartesianDynamicsForceModel<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CartesianDynamicsForceModel")
            .field("model_name", &self.model_name)
            .field("dynamics", &self.dynamics)
            .finish()
    }
}

impl<D: fmt::Debug + Send + Sync> ForceModel for CartesianDynamicsForceModel<D> {
    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn force(&self) -> &dyn Force {
        self.force.as_ref()
    }

    fn state_requirements(&self) -> SpacecraftStateRequirements {
        self.state_requirements
    }
}

impl<D: CartesianDynamics> EvaluableCartesianForceModel for CartesianDynamicsForceModel<D> {
    fn validate_cartesian(&self, state: &CartesianState) -> Result<(), CartesianForceModelError> {
        self.dynamics
            .validate(state)
            .map_err(|source| CartesianForceModelError::new(self.model_name.clone(), source))
    }

    fn cartesian_acceleration(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, CartesianForceModelError> {
        self.dynamics
            .acceleration(epoch, state)
            .map_err(|source| CartesianForceModelError::new(self.model_name.clone(), source))
    }
}

impl<D: CartesianVariationalDynamics> EvaluableCartesianVariationalForceModel
    for CartesianDynamicsForceModel<D>
{
    fn cartesian_acceleration_jacobian(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<CartesianAccelerationJacobian, CartesianForceModelError> {
        self.dynamics
            .acceleration_jacobian(epoch, state)
            .map_err(|source| CartesianForceModelError::new(self.model_name.clone(), source))
    }
}

/// Recoverable failure while validating or evaluating composed Cartesian
/// dynamics.
///
/// This error is never coupled to a specific numerical integrator; it only
/// reports composition-level and per-model evaluation failures.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ComposedCartesianDynamicsError {
    /// The composition has no force models to evaluate.
    #[error("composed Cartesian dynamics has no force models")]
    NoForceModels,
    /// One constituent force model failed to evaluate.
    #[error(transparent)]
    Model(#[from] CartesianForceModelError),
    /// One constituent force model returned an acceleration expressed in a
    /// different frame than the input state.
    #[error(
        "force model {model_name:?} returned an acceleration in a different frame than the input state"
    )]
    FrameMismatch {
        /// Name of the model that returned a mismatched frame.
        model_name: String,
    },
    /// The summed acceleration became non-finite.
    #[error("composed Cartesian acceleration is not finite")]
    NonFiniteAcceleration,
}

fn validate_all<M: EvaluableCartesianForceModel + ?Sized>(
    models: &[Arc<M>],
    state: &CartesianState,
) -> Result<(), ComposedCartesianDynamicsError> {
    if models.is_empty() {
        return Err(ComposedCartesianDynamicsError::NoForceModels);
    }
    for model in models {
        model.validate_cartesian(state)?;
    }
    Ok(())
}

fn accumulate_acceleration<M: EvaluableCartesianForceModel + ?Sized>(
    models: &[Arc<M>],
    epoch: Epoch,
    state: &CartesianState,
    total: &mut AccelerationVector,
) -> Result<(), ComposedCartesianDynamicsError> {
    let frame = state.frame();
    for model in models {
        let contribution = model.cartesian_acceleration(epoch, state)?;
        if contribution.frame() != frame {
            return Err(ComposedCartesianDynamicsError::FrameMismatch {
                model_name: model.model_name().to_owned(),
            });
        }
        *total = *total + contribution.value();
    }
    Ok(())
}

/// Ordered, deterministic composition of evaluable Cartesian force models.
///
/// Accelerations are summed in declaration order, matching the order models
/// are appended with [`Self::with_model`]. Composition performs no additional
/// reordering, retry, or fallback: the first failing model's typed error is
/// returned immediately.
#[derive(Debug, Clone, Default)]
pub struct ComposedCartesianDynamics {
    name: String,
    models: Vec<EvaluableCartesianForceModelHandle>,
}

impl ComposedCartesianDynamics {
    /// Creates an empty, named composed Cartesian dynamics system.
    #[must_use]
    pub fn new(name: String) -> Self {
        Self {
            name,
            models: Vec::new(),
        }
    }

    /// Appends one force model, preserving declaration order.
    #[must_use]
    pub fn with_model(mut self, model: EvaluableCartesianForceModelHandle) -> Self {
        self.models.push(model);
        self
    }

    /// Returns this system's diagnostic name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the composed force models in declaration order.
    #[must_use]
    pub fn models(&self) -> &[EvaluableCartesianForceModelHandle] {
        &self.models
    }
}

impl CartesianDynamics for ComposedCartesianDynamics {
    type Error = ComposedCartesianDynamicsError;

    fn validate(&self, state: &CartesianState) -> Result<(), Self::Error> {
        validate_all(&self.models, state)
    }

    fn acceleration(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, Self::Error> {
        self.validate(state)?;
        let frame = state.frame();
        let mut total = AccelerationVector::from_metres_per_second_squared(0.0, 0.0, 0.0);
        accumulate_acceleration(&self.models, epoch, state, &mut total)?;
        if !total.is_finite() {
            return Err(ComposedCartesianDynamicsError::NonFiniteAcceleration);
        }
        FramedAcceleration::new(total, frame)
            .map_err(|_| ComposedCartesianDynamicsError::NonFiniteAcceleration)
    }
}

/// Ordered, deterministic composition of Jacobian-capable evaluable
/// Cartesian force models.
///
/// Every constituent model must implement
/// [`EvaluableCartesianVariationalForceModel`], so the composed acceleration
/// Jacobian always covers every summed acceleration contribution.
#[derive(Debug, Clone, Default)]
pub struct ComposedCartesianVariationalDynamics {
    name: String,
    models: Vec<EvaluableCartesianVariationalForceModelHandle>,
}

impl ComposedCartesianVariationalDynamics {
    /// Creates an empty, named composed variational Cartesian dynamics
    /// system.
    #[must_use]
    pub fn new(name: String) -> Self {
        Self {
            name,
            models: Vec::new(),
        }
    }

    /// Appends one Jacobian-capable force model, preserving declaration
    /// order.
    #[must_use]
    pub fn with_model(mut self, model: EvaluableCartesianVariationalForceModelHandle) -> Self {
        self.models.push(model);
        self
    }

    /// Returns this system's diagnostic name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the composed force models in declaration order.
    #[must_use]
    pub fn models(&self) -> &[EvaluableCartesianVariationalForceModelHandle] {
        &self.models
    }
}

impl CartesianDynamics for ComposedCartesianVariationalDynamics {
    type Error = ComposedCartesianDynamicsError;

    fn validate(&self, state: &CartesianState) -> Result<(), Self::Error> {
        validate_all(&self.models, state)
    }

    fn acceleration(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, Self::Error> {
        self.validate(state)?;
        let frame = state.frame();
        let mut total = AccelerationVector::from_metres_per_second_squared(0.0, 0.0, 0.0);
        accumulate_acceleration(&self.models, epoch, state, &mut total)?;
        if !total.is_finite() {
            return Err(ComposedCartesianDynamicsError::NonFiniteAcceleration);
        }
        FramedAcceleration::new(total, frame)
            .map_err(|_| ComposedCartesianDynamicsError::NonFiniteAcceleration)
    }
}

impl CartesianVariationalDynamics for ComposedCartesianVariationalDynamics {
    fn acceleration_jacobian(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<CartesianAccelerationJacobian, Self::Error> {
        self.validate(state)?;
        let mut position = [[InverseTimeSquared::from_per_square_second(0.0); 3]; 3];
        let mut velocity = [[InverseTime::from_per_second(0.0); 3]; 3];
        for model in &self.models {
            let jacobian = model.cartesian_acceleration_jacobian(epoch, state)?;
            let model_position = jacobian.position();
            let model_velocity = jacobian.velocity();
            for row in 0..3 {
                for column in 0..3 {
                    position[row][column] = InverseTimeSquared::from_per_square_second(
                        position[row][column].as_per_square_second()
                            + model_position[row][column].as_per_square_second(),
                    );
                    velocity[row][column] = InverseTime::from_per_second(
                        velocity[row][column].as_per_second()
                            + model_velocity[row][column].as_per_second(),
                    );
                }
            }
        }
        CartesianAccelerationJacobian::new(position, velocity)
            .map_err(|_| ComposedCartesianDynamicsError::NonFiniteAcceleration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frames::ReferenceFrame;
    use units::{Position, VelocityVector};

    /// Standard gravitational parameter used only as a numeric test fixture,
    /// matching the value already used by the `dynamics-two-bodies` and
    /// `dynamics-harmonics` crates' own reference-vector tests (WGS84 Earth
    /// GM, public standard data).
    const TEST_MU: f64 = 3.986_004_418e14;
    const TEST_EQUATORIAL_RADIUS_METRES: f64 = 6_378_137.0;
    const TEST_J2: f64 = 1.082_63e-3;

    #[derive(Debug)]
    struct TestForce(String);
    impl Force for TestForce {
        fn name(&self) -> &str {
            &self.0
        }
    }

    fn sample_state(x: f64, y: f64, z: f64, vx: f64, vy: f64, vz: f64) -> CartesianState {
        CartesianState::new(
            ReferenceFrame::GCRF,
            Position::from_metres(x, y, z),
            VelocityVector::from_metres_per_second(vx, vy, vz),
        )
        .expect("finite Cartesian state")
    }

    fn adapt<D>(model_name: &str, dynamics: D) -> EvaluableCartesianForceModelHandle
    where
        D: CartesianDynamics + 'static,
    {
        Arc::new(CartesianDynamicsForceModel::new(
            model_name.to_owned(),
            Arc::new(TestForce(model_name.to_owned())) as Arc<dyn Force>,
            SpacecraftStateRequirements::POSITION,
            dynamics,
        ))
    }

    fn variational_adapt<D>(
        model_name: &str,
        dynamics: D,
    ) -> EvaluableCartesianVariationalForceModelHandle
    where
        D: CartesianVariationalDynamics + 'static,
    {
        Arc::new(CartesianDynamicsForceModel::new(
            model_name.to_owned(),
            Arc::new(TestForce(model_name.to_owned())) as Arc<dyn Force>,
            SpacecraftStateRequirements::POSITION,
            dynamics,
        ))
    }

    /// Independently coded point-mass test model, reproducing the exact
    /// public closed-form equation `dynamics_two_bodies::TwoBodyDynamics` is
    /// independently tested against (NASA Technical Memorandum 2004-213230),
    /// so composition can be validated without a cyclic dev-dependency back
    /// onto a crate that itself depends on `dynamics-core`.
    #[derive(Debug, Clone, Copy)]
    struct PointMassOnly {
        mu: f64,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Error)]
    #[error("point-mass test acceleration is not finite")]
    struct PointMassOnlyError;

    impl CartesianDynamics for PointMassOnly {
        type Error = PointMassOnlyError;

        fn validate(&self, state: &CartesianState) -> Result<(), Self::Error> {
            if state
                .position()
                .norm()
                .get::<units::uom::si::length::meter>()
                == 0.0
            {
                return Err(PointMassOnlyError);
            }
            Ok(())
        }

        fn acceleration(
            &self,
            _epoch: Epoch,
            state: &CartesianState,
        ) -> Result<FramedAcceleration, Self::Error> {
            self.validate(state)?;
            let position = state.position().to_metres();
            let radius_squared = position
                .into_iter()
                .fold(0.0, |sum, component| component.mul_add(component, sum));
            let radius = radius_squared.sqrt();
            let scale = -self.mu / (radius_squared * radius);
            let acceleration = AccelerationVector::from_metres_per_second_squared(
                scale * position[0],
                scale * position[1],
                scale * position[2],
            );
            FramedAcceleration::new(acceleration, state.frame()).map_err(|_| PointMassOnlyError)
        }
    }

    impl CartesianVariationalDynamics for PointMassOnly {
        fn acceleration_jacobian(
            &self,
            _epoch: Epoch,
            state: &CartesianState,
        ) -> Result<CartesianAccelerationJacobian, Self::Error> {
            self.validate(state)?;
            let position = state.position().to_metres();
            let radius_squared = position
                .into_iter()
                .fold(0.0, |sum, component| component.mul_add(component, sum));
            let radius = radius_squared.sqrt();
            let inverse_radius_cubed = 1.0 / (radius_squared * radius);
            let position_partials = std::array::from_fn(|row| {
                std::array::from_fn(|column| {
                    let identity = if row == column { 1.0 } else { 0.0 };
                    InverseTimeSquared::from_per_square_second(
                        self.mu
                            * inverse_radius_cubed
                            * (3.0 * position[row] * position[column] / radius_squared - identity),
                    )
                })
            });
            let velocity_partials = [[InverseTime::from_per_second(0.0); 3]; 3];
            CartesianAccelerationJacobian::new(position_partials, velocity_partials)
                .map_err(|_| PointMassOnlyError)
        }
    }

    /// Independently coded zonal `J2` correction-only test model, reproducing
    /// the same public closed-form equation used by `dynamics_harmonics`
    /// (NASA GMAT *Mathematical Specifications*, 2007).
    #[derive(Debug, Clone, Copy)]
    struct J2CorrectionOnly {
        mu: f64,
        equatorial_radius_metres: f64,
        j2: f64,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Error)]
    #[error("J2-only test acceleration is not finite")]
    struct J2CorrectionOnlyError;

    impl CartesianDynamics for J2CorrectionOnly {
        type Error = J2CorrectionOnlyError;

        fn validate(&self, _state: &CartesianState) -> Result<(), Self::Error> {
            Ok(())
        }

        fn acceleration(
            &self,
            _epoch: Epoch,
            state: &CartesianState,
        ) -> Result<FramedAcceleration, Self::Error> {
            let position = state.position().to_metres();
            let radius_squared = position
                .into_iter()
                .fold(0.0, |sum, component| component.mul_add(component, sum));
            let radius = radius_squared.sqrt();
            let z_over_r_squared = (position[2] * position[2]) / radius_squared;
            let radius_fifth = radius_squared * radius_squared * radius;
            let factor = -1.5
                * self.j2
                * self.mu
                * self.equatorial_radius_metres
                * self.equatorial_radius_metres
                / radius_fifth;
            let horizontal_term = 1.0 - 5.0 * z_over_r_squared;
            let vertical_term = 3.0 - 5.0 * z_over_r_squared;
            let acceleration = AccelerationVector::from_metres_per_second_squared(
                factor * position[0] * horizontal_term,
                factor * position[1] * horizontal_term,
                factor * position[2] * vertical_term,
            );
            FramedAcceleration::new(acceleration, state.frame()).map_err(|_| J2CorrectionOnlyError)
        }
    }

    /// Simple linear restoring test model (`a = -k r`) with a trivially known
    /// analytic acceleration Jacobian, used only to validate composed
    /// Jacobian summation.
    #[derive(Debug, Clone, Copy)]
    struct LinearRestoring {
        k: f64,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Error)]
    #[error("linear restoring test acceleration is not finite")]
    struct LinearRestoringError;

    impl CartesianDynamics for LinearRestoring {
        type Error = LinearRestoringError;

        fn validate(&self, _state: &CartesianState) -> Result<(), Self::Error> {
            Ok(())
        }

        fn acceleration(
            &self,
            _epoch: Epoch,
            state: &CartesianState,
        ) -> Result<FramedAcceleration, Self::Error> {
            let position = state.position().to_metres();
            let acceleration = AccelerationVector::from_metres_per_second_squared(
                -self.k * position[0],
                -self.k * position[1],
                -self.k * position[2],
            );
            FramedAcceleration::new(acceleration, state.frame()).map_err(|_| LinearRestoringError)
        }
    }

    impl CartesianVariationalDynamics for LinearRestoring {
        fn acceleration_jacobian(
            &self,
            _epoch: Epoch,
            _state: &CartesianState,
        ) -> Result<CartesianAccelerationJacobian, Self::Error> {
            let position_partials = std::array::from_fn(|row| {
                std::array::from_fn(|column| {
                    let identity = if row == column { 1.0 } else { 0.0 };
                    InverseTimeSquared::from_per_square_second(-self.k * identity)
                })
            });
            let velocity_partials = [[InverseTime::from_per_second(0.0); 3]; 3];
            CartesianAccelerationJacobian::new(position_partials, velocity_partials)
                .map_err(|_| LinearRestoringError)
        }
    }

    /// Linear drag test model (`a = -k v`), used only to exercise a
    /// non-conservative, velocity-dependent contribution in composition.
    #[derive(Debug, Clone, Copy)]
    struct LinearDrag {
        k: f64,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Error)]
    #[error("linear drag test acceleration is not finite")]
    struct LinearDragError;

    impl CartesianDynamics for LinearDrag {
        type Error = LinearDragError;

        fn validate(&self, _state: &CartesianState) -> Result<(), Self::Error> {
            Ok(())
        }

        fn acceleration(
            &self,
            _epoch: Epoch,
            state: &CartesianState,
        ) -> Result<FramedAcceleration, Self::Error> {
            let velocity = state.velocity().to_metres_per_second();
            let acceleration = AccelerationVector::from_metres_per_second_squared(
                -self.k * velocity[0],
                -self.k * velocity[1],
                -self.k * velocity[2],
            );
            FramedAcceleration::new(acceleration, state.frame()).map_err(|_| LinearDragError)
        }
    }

    /// Test model that always fails, used to exercise typed per-model error
    /// composition.
    #[derive(Debug, Clone, Copy)]
    struct AlwaysFails;

    #[derive(Debug, Clone, Copy, PartialEq, Error)]
    #[error("always-failing test model")]
    struct AlwaysFailsError;

    impl CartesianDynamics for AlwaysFails {
        type Error = AlwaysFailsError;

        fn validate(&self, _state: &CartesianState) -> Result<(), Self::Error> {
            Ok(())
        }

        fn acceleration(
            &self,
            _epoch: Epoch,
            _state: &CartesianState,
        ) -> Result<FramedAcceleration, Self::Error> {
            Err(AlwaysFailsError)
        }
    }

    /// Test model that returns an acceleration in a foreign frame, used to
    /// exercise the composition-level frame-consistency check.
    #[derive(Debug, Clone, Copy)]
    struct WrongFrame;

    #[derive(Debug, Clone, Copy, PartialEq, Error)]
    #[error("wrong-frame test model")]
    struct WrongFrameError;

    impl CartesianDynamics for WrongFrame {
        type Error = WrongFrameError;

        fn validate(&self, _state: &CartesianState) -> Result<(), Self::Error> {
            Ok(())
        }

        fn acceleration(
            &self,
            _epoch: Epoch,
            _state: &CartesianState,
        ) -> Result<FramedAcceleration, Self::Error> {
            FramedAcceleration::new(
                AccelerationVector::from_metres_per_second_squared(0.0, 0.0, 0.0),
                ReferenceFrame::ITRF2020,
            )
            .map_err(|_| WrongFrameError)
        }
    }

    fn point_mass_only() -> PointMassOnly {
        PointMassOnly { mu: TEST_MU }
    }

    #[test]
    fn wrapping_a_point_mass_system_preserves_its_direct_acceleration() {
        let epoch = Epoch::from_tai_seconds(0.0);
        let state = sample_state(7_000_000.0, 1_000_000.0, 2_000_000.0, 0.0, 7_000.0, 1_000.0);
        let point_mass = point_mass_only();
        let expected = point_mass
            .acceleration(epoch, &state)
            .expect("direct evaluation");

        let composed = ComposedCartesianDynamics::new("wrapped point mass".to_owned())
            .with_model(adapt("point mass", point_mass));
        let actual = composed
            .acceleration(epoch, &state)
            .expect("composed evaluation");

        assert_eq!(actual, expected);
    }

    #[test]
    fn composing_point_mass_and_j2_matches_the_independent_closed_form_sum() {
        let epoch = Epoch::from_tai_seconds(0.0);
        let state = sample_state(7_000_000.0, 1_000_000.0, 2_000_000.0, 0.0, 7_000.0, 1_000.0);

        let point_mass = point_mass_only();
        let j2_only = J2CorrectionOnly {
            mu: TEST_MU,
            equatorial_radius_metres: TEST_EQUATORIAL_RADIUS_METRES,
            j2: TEST_J2,
        };
        let expected = point_mass
            .acceleration(epoch, &state)
            .expect("point mass")
            .value()
            + j2_only
                .acceleration(epoch, &state)
                .expect("J2 correction")
                .value();

        let composed = ComposedCartesianDynamics::new("point-mass + J2".to_owned())
            .with_model(adapt("point mass", point_mass))
            .with_model(adapt("J2 correction", j2_only));
        let actual = composed
            .acceleration(epoch, &state)
            .expect("composed evaluation");

        assert_eq!(actual.value(), expected);
    }

    #[test]
    fn declaration_order_does_not_change_the_two_model_sum() {
        let epoch = Epoch::from_tai_seconds(0.0);
        let state = sample_state(7_000_000.0, 1_000_000.0, 2_000_000.0, 0.0, 7_000.0, 1_000.0);

        let forward = ComposedCartesianDynamics::new("forward".to_owned())
            .with_model(adapt("point mass", point_mass_only()))
            .with_model(adapt("drag", LinearDrag { k: 1.0e-4 }));
        let backward = ComposedCartesianDynamics::new("backward".to_owned())
            .with_model(adapt("drag", LinearDrag { k: 1.0e-4 }))
            .with_model(adapt("point mass", point_mass_only()));

        assert_eq!(
            forward.acceleration(epoch, &state).expect("forward"),
            backward.acceleration(epoch, &state).expect("backward"),
        );
    }

    #[test]
    fn evaluation_is_deterministic_across_repeated_calls() {
        let epoch = Epoch::from_tai_seconds(0.0);
        let state = sample_state(7_000_000.0, 1_000_000.0, 2_000_000.0, 0.0, 7_000.0, 1_000.0);
        let composed = ComposedCartesianDynamics::new("repeatable".to_owned())
            .with_model(adapt("point mass", point_mass_only()))
            .with_model(adapt("drag", LinearDrag { k: 1.0e-4 }));

        let first = composed
            .acceleration(epoch, &state)
            .expect("first evaluation");
        let second = composed
            .acceleration(epoch, &state)
            .expect("second evaluation");
        assert_eq!(first, second);
    }

    #[test]
    fn mixed_conservative_and_non_conservative_models_sum_correctly() {
        let epoch = Epoch::from_tai_seconds(0.0);
        let state = sample_state(7_000_000.0, 1_000_000.0, 2_000_000.0, 0.0, 7_000.0, 1_000.0);
        let k = 1.0e-4;

        let point_mass_result = point_mass_only()
            .acceleration(epoch, &state)
            .expect("point-mass evaluation");
        let velocity = state.velocity().to_metres_per_second();
        let expected = point_mass_result.value()
            + AccelerationVector::from_metres_per_second_squared(
                -k * velocity[0],
                -k * velocity[1],
                -k * velocity[2],
            );

        let composed = ComposedCartesianDynamics::new("point-mass + drag".to_owned())
            .with_model(adapt("point mass", point_mass_only()))
            .with_model(adapt("drag", LinearDrag { k }));
        let actual = composed
            .acceleration(epoch, &state)
            .expect("composed evaluation");

        assert_eq!(actual.value(), expected);
        assert_eq!(actual.frame(), point_mass_result.frame());
    }

    #[test]
    fn failing_model_reports_a_named_typed_error() {
        let epoch = Epoch::from_tai_seconds(0.0);
        let state = sample_state(7_000_000.0, 1_000_000.0, 2_000_000.0, 0.0, 7_000.0, 1_000.0);
        let composed = ComposedCartesianDynamics::new("with failure".to_owned())
            .with_model(adapt("point mass", point_mass_only()))
            .with_model(adapt("always fails", AlwaysFails));

        let error = composed
            .acceleration(epoch, &state)
            .expect_err("composed evaluation must fail");
        match error {
            ComposedCartesianDynamicsError::Model(model_error) => {
                assert_eq!(model_error.model_name(), "always fails");
                assert!(std::error::Error::source(&model_error).is_some());
            }
            other => panic!("expected a wrapped model error, got {other:?}"),
        }
    }

    #[test]
    fn empty_composition_is_rejected() {
        let epoch = Epoch::from_tai_seconds(0.0);
        let state = sample_state(7_000_000.0, 1_000_000.0, 2_000_000.0, 0.0, 7_000.0, 1_000.0);
        let composed = ComposedCartesianDynamics::new("empty".to_owned());

        assert!(matches!(
            composed.acceleration(epoch, &state),
            Err(ComposedCartesianDynamicsError::NoForceModels)
        ));
    }

    #[test]
    fn mismatched_model_frame_is_rejected() {
        let epoch = Epoch::from_tai_seconds(0.0);
        let state = sample_state(7_000_000.0, 1_000_000.0, 2_000_000.0, 0.0, 7_000.0, 1_000.0);
        let composed = ComposedCartesianDynamics::new("wrong frame".to_owned())
            .with_model(adapt("point mass", point_mass_only()))
            .with_model(adapt("wrong frame", WrongFrame));

        let error = composed
            .acceleration(epoch, &state)
            .expect_err("composed evaluation must fail");
        match error {
            ComposedCartesianDynamicsError::FrameMismatch { model_name } => {
                assert_eq!(model_name, "wrong frame");
            }
            other => panic!("expected a frame mismatch, got {other:?}"),
        }
    }

    #[test]
    fn composed_variational_dynamics_sums_accelerations_and_jacobians() {
        let epoch = Epoch::from_tai_seconds(0.0);
        let state = sample_state(7_000_000.0, 1_000_000.0, 2_000_000.0, 0.0, 7_000.0, 1_000.0);
        let point_mass = point_mass_only();
        let restoring = LinearRestoring { k: 1.0e-6 };

        let expected_acceleration = point_mass
            .acceleration(epoch, &state)
            .expect("point-mass evaluation")
            .value()
            + restoring
                .acceleration(epoch, &state)
                .expect("restoring evaluation")
                .value();
        let expected_jacobian_position = point_mass
            .acceleration_jacobian(epoch, &state)
            .expect("point-mass Jacobian")
            .position();

        let composed = ComposedCartesianVariationalDynamics::new("variational".to_owned())
            .with_model(variational_adapt("point mass", point_mass))
            .with_model(variational_adapt("restoring", restoring));

        let acceleration = composed
            .acceleration(epoch, &state)
            .expect("composed acceleration");
        assert_eq!(acceleration.value(), expected_acceleration);

        let jacobian = composed
            .acceleration_jacobian(epoch, &state)
            .expect("composed Jacobian");
        let composed_position = jacobian.position();
        for (row, (expected_row, composed_row)) in expected_jacobian_position
            .iter()
            .zip(composed_position.iter())
            .enumerate()
        {
            for (column, (expected_value, composed_value)) in
                expected_row.iter().zip(composed_row.iter()).enumerate()
            {
                let expected = expected_value.as_per_square_second()
                    - restoring.k * if row == column { 1.0 } else { 0.0 };
                assert!((composed_value.as_per_square_second() - expected).abs() < 1.0e-18);
            }
        }
    }
}
