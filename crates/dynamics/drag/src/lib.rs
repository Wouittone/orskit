#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! A provider-driven cannonball atmospheric-drag force model.
//!
//! This crate deliberately contains no atmosphere data and does not choose a
//! body rotation model. The atmosphere density provider and the
//! [`AtmosphereRelativeVelocityProvider`] are both required at construction
//! time. In particular, a velocity in an inertial state is never silently
//! treated as atmosphere-relative: the latter provider is the explicit
//! co-rotation/relative-motion boundary.

use std::{fmt, sync::Arc};

use atmosphere::{AtmosphereDensityInput, SharedAtmosphereDensityProvider};
use dynamics::{
    CartesianForceModelError, EvaluableCartesianForceModel, Force, ForceModel,
    NonConservativeForceModel, SpacecraftStateRequirements,
};
use frames::{Body, ReferenceFrame};
use hifitime::Epoch;
use orbits::cartesian::{CartesianState, FramedAcceleration};
use thiserror::Error;
use units::uom::si::{area::square_meter, mass::kilogram, ratio::ratio};
use units::{AccelerationVector, Area, Mass, Ratio, VelocityVector};

/// A validated positive spacecraft reference area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragArea(Area);

impl DragArea {
    /// Creates an area in square metres.
    pub fn new(value: Area) -> Result<Self, DragInputError> {
        let value = value.get::<square_meter>();
        if !value.is_finite() {
            return Err(DragInputError::NonFiniteArea);
        }
        if value <= 0.0 {
            return Err(DragInputError::NonPositiveArea);
        }
        Ok(Self(Area::new::<square_meter>(value)))
    }

    /// Returns the validated area.
    #[must_use]
    pub const fn value(self) -> Area {
        self.0
    }
}

/// A validated positive spacecraft mass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragMass(Mass);

impl DragMass {
    /// Creates a mass in kilograms.
    pub fn new(value: Mass) -> Result<Self, DragInputError> {
        let value = value.get::<kilogram>();
        if !value.is_finite() {
            return Err(DragInputError::NonFiniteMass);
        }
        if value <= 0.0 {
            return Err(DragInputError::NonPositiveMass);
        }
        Ok(Self(Mass::new::<kilogram>(value)))
    }

    /// Returns the validated mass.
    #[must_use]
    pub const fn value(self) -> Mass {
        self.0
    }
}

/// A validated positive, finite cannonball drag coefficient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragCoefficient(Ratio);

impl DragCoefficient {
    /// Creates a dimensionless coefficient.
    pub fn new(value: Ratio) -> Result<Self, DragInputError> {
        let value = value.get::<ratio>();
        if !value.is_finite() {
            return Err(DragInputError::NonFiniteCoefficient);
        }
        if value <= 0.0 {
            return Err(DragInputError::NonPositiveCoefficient);
        }
        Ok(Self(Ratio::new::<ratio>(value)))
    }

    /// Returns the validated coefficient.
    #[must_use]
    pub const fn value(self) -> Ratio {
        self.0
    }
}

/// A finite velocity explicitly declared relative to the atmosphere.
///
/// The provider creating this value owns the convention for atmospheric
/// motion, including co-rotation. No Earth rotation, body rotation, or frame
/// transform is inferred by this crate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtmosphereRelativeVelocity {
    value: VelocityVector,
    frame: ReferenceFrame,
}

impl AtmosphereRelativeVelocity {
    /// Attaches an atmosphere-relative velocity to its expressing frame.
    pub fn new(
        value: VelocityVector,
        frame: ReferenceFrame,
    ) -> Result<Self, RelativeVelocityError> {
        if !value.is_finite() {
            return Err(RelativeVelocityError::NonFinite);
        }
        Ok(Self { value, frame })
    }

    /// Returns the atmosphere-relative velocity.
    #[must_use]
    pub const fn value(self) -> VelocityVector {
        self.value
    }

    /// Returns the expressing frame.
    #[must_use]
    pub const fn frame(self) -> ReferenceFrame {
        self.frame
    }
}

/// Supplies the explicit atmosphere-relative velocity used by drag.
pub trait AtmosphereRelativeVelocityProvider: fmt::Debug + Send + Sync {
    /// Provider-specific failure.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Computes relative velocity in the supplied state frame.
    fn relative_velocity(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<AtmosphereRelativeVelocity, Self::Error>;
}

/// Input validation or relative-velocity construction failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum DragInputError {
    /// Area was NaN or infinite.
    #[error("drag area must be finite")]
    NonFiniteArea,
    /// Area was zero or negative.
    #[error("drag area must be positive")]
    NonPositiveArea,
    /// Mass was NaN or infinite.
    #[error("drag mass must be finite")]
    NonFiniteMass,
    /// Mass was zero or negative.
    #[error("drag mass must be positive")]
    NonPositiveMass,
    /// Coefficient was NaN or infinite.
    #[error("drag coefficient must be finite")]
    NonFiniteCoefficient,
    /// Coefficient was zero or negative.
    #[error("drag coefficient must be positive")]
    NonPositiveCoefficient,
}

/// Invalid atmosphere-relative velocity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum RelativeVelocityError {
    /// At least one velocity component was not finite.
    #[error("atmosphere-relative velocity must be finite")]
    NonFinite,
}

/// Failure evaluating a cannonball drag contribution.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DragEvaluationError<DE, VE>
where
    DE: std::error::Error + Send + Sync + 'static,
    VE: std::error::Error + Send + Sync + 'static,
{
    /// The state frame differs from the frame used for density.
    #[error("state frame does not match configured atmosphere frame")]
    FrameMismatch,
    /// Density provider failed.
    #[error("atmosphere density evaluation failed")]
    Density(#[source] DE),
    /// Relative-velocity provider failed.
    #[error("atmosphere-relative velocity evaluation failed")]
    RelativeVelocity(#[source] VE),
    /// Relative velocity was expressed in another frame.
    #[error("atmosphere-relative velocity frame does not match state frame")]
    RelativeVelocityFrameMismatch,
    /// The computed acceleration was not finite.
    #[error("atmospheric drag acceleration is not finite")]
    NonFiniteAcceleration,
}

/// Physical atmospheric-drag interaction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct AtmosphericDragForce;

impl Force for AtmosphericDragForce {
    fn name(&self) -> &str {
        "atmospheric drag"
    }
}

static DRAG_FORCE: AtmosphericDragForce = AtmosphericDragForce;

/// Cannonball drag using explicit density and atmosphere-relative-velocity
/// providers.
#[derive(Debug)]
pub struct CannonballDragModel<D, V> {
    atmosphere_body: Body,
    atmosphere_frame: ReferenceFrame,
    density: SharedAtmosphereDensityProvider<D>,
    relative_velocity: Arc<V>,
    area: DragArea,
    mass: DragMass,
    coefficient: DragCoefficient,
}

impl<D, V> CannonballDragModel<D, V>
where
    D: std::error::Error + Send + Sync + 'static,
    V: AtmosphereRelativeVelocityProvider,
{
    /// Constructs a model with every atmosphere and spacecraft input explicit.
    #[must_use]
    pub fn new(
        atmosphere_body: Body,
        atmosphere_frame: ReferenceFrame,
        density: SharedAtmosphereDensityProvider<D>,
        relative_velocity: Arc<V>,
        area: DragArea,
        mass: DragMass,
        coefficient: DragCoefficient,
    ) -> Self {
        Self {
            atmosphere_body,
            atmosphere_frame,
            density,
            relative_velocity,
            area,
            mass,
            coefficient,
        }
    }

    /// Returns the frame used for the density query and relative velocity.
    #[must_use]
    pub const fn atmosphere_frame(&self) -> ReferenceFrame {
        self.atmosphere_frame
    }

    /// Returns the density provider.
    #[must_use]
    pub fn density_provider(&self) -> &SharedAtmosphereDensityProvider<D> {
        &self.density
    }

    fn evaluate_typed(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, DragEvaluationError<D, V::Error>> {
        if state.frame() != self.atmosphere_frame {
            return Err(DragEvaluationError::FrameMismatch);
        }
        let input = AtmosphereDensityInput::new(
            epoch,
            self.atmosphere_body,
            state.position(),
            self.atmosphere_frame,
        )
        .map_err(|_| DragEvaluationError::FrameMismatch)?;
        let density = self
            .density
            .density(input)
            .map_err(DragEvaluationError::Density)?;
        let relative = self
            .relative_velocity
            .relative_velocity(epoch, state)
            .map_err(DragEvaluationError::RelativeVelocity)?;
        if relative.frame() != state.frame() {
            return Err(DragEvaluationError::RelativeVelocityFrameMismatch);
        }
        let rho = density
            .value()
            .get::<units::uom::si::mass_density::kilogram_per_cubic_meter>();
        let v = relative.value().to_metres_per_second();
        let speed = v
            .iter()
            .map(|component| component * component)
            .sum::<f64>()
            .sqrt();
        let scale = -0.5
            * rho
            * self.coefficient.value().get::<ratio>()
            * self.area.value().get::<square_meter>()
            / self.mass.value().get::<kilogram>()
            * speed;
        let acceleration = AccelerationVector::from_metres_per_second_squared(
            scale * v[0],
            scale * v[1],
            scale * v[2],
        );
        if !acceleration.is_finite() {
            return Err(DragEvaluationError::NonFiniteAcceleration);
        }
        FramedAcceleration::new(acceleration, state.frame())
            .map_err(|_| DragEvaluationError::NonFiniteAcceleration)
    }
}

impl<D, V> ForceModel for CannonballDragModel<D, V>
where
    D: std::error::Error + Send + Sync + 'static,
    V: AtmosphereRelativeVelocityProvider,
{
    fn model_name(&self) -> &str {
        "cannonball atmospheric drag model"
    }
    fn force(&self) -> &dyn Force {
        &DRAG_FORCE
    }
    fn state_requirements(&self) -> SpacecraftStateRequirements {
        SpacecraftStateRequirements::POSITION.union(SpacecraftStateRequirements::VELOCITY)
    }
}

impl<D, V> NonConservativeForceModel for CannonballDragModel<D, V>
where
    D: std::error::Error + Send + Sync + 'static,
    V: AtmosphereRelativeVelocityProvider,
{
}

impl<D, V> EvaluableCartesianForceModel for CannonballDragModel<D, V>
where
    D: std::error::Error + Send + Sync + 'static,
    V: AtmosphereRelativeVelocityProvider + 'static,
{
    fn validate_cartesian(&self, state: &CartesianState) -> Result<(), CartesianForceModelError> {
        if state.frame() != self.atmosphere_frame {
            return Err(CartesianForceModelError::new(
                self.model_name(),
                DragEvaluationError::<D, V::Error>::FrameMismatch,
            ));
        }
        Ok(())
    }

    fn cartesian_acceleration(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, CartesianForceModelError> {
        self.evaluate_typed(epoch, state)
            .map_err(|source| CartesianForceModelError::new(self.model_name(), source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atmosphere::{AtmosphereDataContext, AtmosphereDensity, AtmosphereDensityProvider};
    use frames::Body;
    use units::uom::si::{
        area::square_meter, mass::kilogram, mass_density::kilogram_per_cubic_meter,
    };
    use units::{MassDensity, Position};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
    #[error("test provider error")]
    struct TestError;

    #[derive(Debug)]
    struct ConstantDensity {
        context: AtmosphereDataContext,
        density: AtmosphereDensity,
    }

    impl AtmosphereDensityProvider for ConstantDensity {
        type Error = TestError;

        fn data_context(&self) -> &AtmosphereDataContext {
            &self.context
        }

        fn density(
            &self,
            _input: AtmosphereDensityInput,
        ) -> Result<AtmosphereDensity, Self::Error> {
            Ok(self.density)
        }
    }

    #[derive(Debug)]
    struct ConstantRelativeVelocity(VelocityVector);

    impl AtmosphereRelativeVelocityProvider for ConstantRelativeVelocity {
        type Error = TestError;

        fn relative_velocity(
            &self,
            _epoch: Epoch,
            state: &CartesianState,
        ) -> Result<AtmosphereRelativeVelocity, Self::Error> {
            AtmosphereRelativeVelocity::new(self.0, state.frame()).map_err(|_| TestError)
        }
    }

    fn model(velocity: VelocityVector) -> CannonballDragModel<TestError, ConstantRelativeVelocity> {
        let context = AtmosphereDataContext::new("test", "reference").expect("context");
        let density = AtmosphereDensity::new(MassDensity::new::<kilogram_per_cubic_meter>(2.0))
            .expect("density");
        CannonballDragModel::new(
            Body::EARTH,
            ReferenceFrame::GCRF,
            Arc::new(ConstantDensity { context, density }),
            Arc::new(ConstantRelativeVelocity(velocity)),
            DragArea::new(Area::new::<square_meter>(3.0)).expect("area"),
            DragMass::new(Mass::new::<kilogram>(4.0)).expect("mass"),
            DragCoefficient::new(Ratio::new::<ratio>(0.5)).expect("coefficient"),
        )
    }

    fn state(velocity: VelocityVector) -> CartesianState {
        CartesianState::new(
            ReferenceFrame::GCRF,
            Position::from_metres(7.0e6, 0.0, 0.0),
            velocity,
        )
        .expect("state")
    }

    #[test]
    fn reference_cannonball_equation_and_direction() {
        let model = model(VelocityVector::from_metres_per_second(10.0, 0.0, 0.0));
        let acceleration = model
            .cartesian_acceleration(
                Epoch::from_tai_seconds(0.0),
                &state(VelocityVector::from_metres_per_second(0.0, 0.0, 0.0)),
            )
            .expect("acceleration")
            .value()
            .to_metres_per_second_squared();
        // -1/2 * 2 * .5 * 3/4 * |10| * (10,0,0).
        assert_eq!(acceleration, [-37.5, 0.0, 0.0]);
    }

    #[test]
    fn validated_inputs_reject_nonphysical_values() {
        assert_eq!(
            DragMass::new(Mass::new::<kilogram>(0.0)),
            Err(DragInputError::NonPositiveMass)
        );
        assert_eq!(
            DragArea::new(Area::new::<square_meter>(f64::NAN)),
            Err(DragInputError::NonFiniteArea)
        );
        assert_eq!(
            DragCoefficient::new(Ratio::new::<ratio>(-1.0)),
            Err(DragInputError::NonPositiveCoefficient)
        );
    }

    #[test]
    fn zero_relative_velocity_has_zero_drag() {
        let model = model(VelocityVector::from_metres_per_second(0.0, 0.0, 0.0));
        let acceleration = model
            .cartesian_acceleration(
                Epoch::from_tai_seconds(0.0),
                &state(VelocityVector::from_metres_per_second(0.0, 0.0, 0.0)),
            )
            .expect("acceleration")
            .value()
            .to_metres_per_second_squared();
        assert_eq!(acceleration, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn drag_scales_quadratically_with_relative_speed() {
        let slow = model(VelocityVector::from_metres_per_second(10.0, 0.0, 0.0))
            .cartesian_acceleration(
                Epoch::from_tai_seconds(0.0),
                &state(VelocityVector::from_metres_per_second(0.0, 0.0, 0.0)),
            )
            .expect("slow acceleration")
            .value()
            .to_metres_per_second_squared();
        let fast = model(VelocityVector::from_metres_per_second(20.0, 0.0, 0.0))
            .cartesian_acceleration(
                Epoch::from_tai_seconds(0.0),
                &state(VelocityVector::from_metres_per_second(0.0, 0.0, 0.0)),
            )
            .expect("fast acceleration")
            .value()
            .to_metres_per_second_squared();

        assert_eq!(fast[0], slow[0] * 4.0);
        assert_eq!(fast[1], 0.0);
        assert_eq!(fast[2], 0.0);
    }

    #[test]
    fn model_can_cross_object_safe_force_model_boundary() {
        let model = model(VelocityVector::from_metres_per_second(10.0, 0.0, 0.0));
        let _: dynamics::EvaluableCartesianForceModelHandle = Arc::new(model);
    }
}
