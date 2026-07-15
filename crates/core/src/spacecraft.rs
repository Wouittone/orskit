use std::sync::Arc;

use frames::{FrameMotion, FrameOrientation, FrameOrigin, ReferenceFrame};
use hifitime::Epoch;
use nalgebra::{Matrix3, Quaternion, UnitQuaternion};
use thiserror::Error;
#[cfg(feature = "standard-shapes")]
use units::uom::si::length::meter;
use units::uom::si::{
    angle::radian, mass::kilogram, moment_of_inertia::kilogram_square_meter, ratio::ratio,
};
#[cfg(feature = "standard-shapes")]
use units::Length;
use units::{Angle, AngularVelocity, AngularVelocityVector, Mass, MomentOfInertia, Ratio};

use crate::{Orbit, SpacecraftState};

/// A normalized rotation between two explicitly identified frames.
///
/// The rotation maps coordinate components from `from_frame` into `to_frame`.
#[derive(Debug, Clone, PartialEq)]
pub struct Orientation {
    rotation: UnitQuaternion<f64>,
    from_frame: ReferenceFrame,
    to_frame: ReferenceFrame,
}

/// Explicit frames and scalar/i/j/k quaternion components for an [`Orientation`].
pub type OrientationQuaternionInput = (ReferenceFrame, ReferenceFrame, [Ratio; 4]);

impl TryFrom<OrientationQuaternionInput> for Orientation {
    type Error = OrientationError;

    fn try_from(
        (source_frame, target_frame, components): OrientationQuaternionInput,
    ) -> Result<Self, Self::Error> {
        let values = components.map(|component| component.get::<ratio>());
        if !values.into_iter().all(f64::is_finite) {
            return Err(OrientationError::NonFinite);
        }

        let quaternion = Quaternion::new(values[0], values[1], values[2], values[3]);
        if quaternion.norm_squared() <= f64::EPSILON {
            return Err(OrientationError::ZeroNorm);
        }

        Ok(Self {
            rotation: UnitQuaternion::new_normalize(quaternion),
            from_frame: source_frame,
            to_frame: target_frame,
        })
    }
}

impl Orientation {
    /// Identity rotation between two explicitly identified frames.
    #[must_use]
    pub fn identity(from_frame: ReferenceFrame, to_frame: ReferenceFrame) -> Self {
        Self {
            rotation: UnitQuaternion::identity(),
            from_frame,
            to_frame,
        }
    }

    /// Returns the frame whose components this rotation consumes.
    #[must_use]
    pub const fn source_frame(&self) -> ReferenceFrame {
        self.from_frame
    }

    /// Returns the frame whose components this rotation produces.
    #[must_use]
    pub const fn target_frame(&self) -> ReferenceFrame {
        self.to_frame
    }

    /// Returns normalized scalar/i/j/k quaternion components.
    #[must_use]
    pub fn quaternion(&self) -> [Ratio; 4] {
        let quaternion = self.rotation.quaternion();
        [
            Ratio::new::<ratio>(quaternion.w),
            Ratio::new::<ratio>(quaternion.i),
            Ratio::new::<ratio>(quaternion.j),
            Ratio::new::<ratio>(quaternion.k),
        ]
    }

    /// Returns intrinsic roll/pitch/yaw angles about x/y/z, in that order.
    ///
    /// The quaternion remains the canonical representation; these Euler angles
    /// are a convenience view and inherit the usual pitch singularity.
    #[must_use]
    pub fn angles(&self) -> [Angle; 3] {
        let (roll, pitch, yaw) = self.rotation.euler_angles();
        [
            Angle::new::<radian>(roll),
            Angle::new::<radian>(pitch),
            Angle::new::<radian>(yaw),
        ]
    }
}

/// Body angular velocity relative to a reference frame, expressed in body axes.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyAngularVelocity {
    value: AngularVelocityVector,
    body_frame: SpacecraftBodyFrame,
    relative_to: ReferenceFrame,
}

impl BodyAngularVelocity {
    /// Declares body angular velocity relative to `relative_to`, expressed in `body_frame`.
    pub fn new(
        value: AngularVelocityVector,
        body_frame: SpacecraftBodyFrame,
        relative_to: ReferenceFrame,
    ) -> Result<Self, AttitudeError> {
        if !value.is_finite() {
            return Err(AttitudeError::NonFiniteAngularVelocity);
        }
        Ok(Self {
            value,
            body_frame,
            relative_to,
        })
    }

    /// Returns the angular-velocity vector.
    #[must_use]
    pub const fn value(&self) -> AngularVelocityVector {
        self.value
    }

    /// Returns the expression frame.
    #[must_use]
    pub const fn body_frame(&self) -> ReferenceFrame {
        self.body_frame.reference_frame()
    }

    /// Returns the spacecraft/body ownership capability.
    #[must_use]
    pub const fn body_frame_capability(&self) -> &SpacecraftBodyFrame {
        &self.body_frame
    }

    /// Returns the reference frame relative to which the body rotates.
    #[must_use]
    pub const fn relative_to(&self) -> ReferenceFrame {
        self.relative_to
    }

    /// Returns angular speeds about x/y/z.
    #[must_use]
    pub const fn components(&self) -> [AngularVelocity; 3] {
        self.value.components()
    }
}

/// Epoch-specific spacecraft attitude contract.
///
/// Applications may implement this for an estimator state, a tabulated
/// attitude, or another representation. The view validates frame ownership
/// through this contract without choosing a representation.
pub trait Attitude: std::fmt::Debug + Send + Sync {
    /// Returns the body-to-reference orientation.
    fn orientation(&self) -> &Orientation;

    /// Returns angular velocity expressed in body axes.
    fn angular_velocity(&self) -> &BodyAngularVelocity;

    /// Returns intrinsic roll/pitch/yaw angles about x/y/z.
    #[must_use]
    fn angles(&self) -> [Angle; 3] {
        self.orientation().angles()
    }

    /// Returns angular speeds about body x/y/z.
    #[must_use]
    fn angular_speeds(&self) -> [AngularVelocity; 3] {
        self.angular_velocity().components()
    }
}

/// Immutable attitude represented by a quaternion and body angular velocity.
#[cfg(feature = "quaternion-attitude")]
#[derive(Debug, Clone, PartialEq)]
pub struct QuaternionAttitude {
    orientation: Orientation,
    angular_velocity: BodyAngularVelocity,
}

#[cfg(feature = "quaternion-attitude")]
impl QuaternionAttitude {
    /// Constructs an attitude with angular velocity in the orientation's body frame.
    pub fn new(
        orientation: Orientation,
        angular_velocity: BodyAngularVelocity,
    ) -> Result<Self, AttitudeError> {
        if orientation.source_frame() != angular_velocity.body_frame() {
            return Err(AttitudeError::AngularVelocityBodyFrameMismatch);
        }
        if orientation.target_frame() != angular_velocity.relative_to() {
            return Err(AttitudeError::AngularVelocityReferenceFrameMismatch);
        }
        Ok(Self {
            orientation,
            angular_velocity,
        })
    }

    /// Returns the body-to-reference orientation.
    #[must_use]
    pub const fn orientation(&self) -> &Orientation {
        &self.orientation
    }

    /// Returns angular velocity expressed in the body frame.
    #[must_use]
    pub const fn angular_velocity(&self) -> &BodyAngularVelocity {
        &self.angular_velocity
    }

    /// Returns intrinsic roll/pitch/yaw angles about x/y/z.
    #[must_use]
    pub fn angles(&self) -> [Angle; 3] {
        self.orientation.angles()
    }

    /// Returns angular speeds about body x/y/z.
    #[must_use]
    pub const fn angular_speeds(&self) -> [AngularVelocity; 3] {
        self.angular_velocity.components()
    }
}

#[cfg(feature = "quaternion-attitude")]
impl Attitude for QuaternionAttitude {
    fn orientation(&self) -> &Orientation {
        self.orientation()
    }

    fn angular_velocity(&self) -> &BodyAngularVelocity {
        self.angular_velocity()
    }
}

/// Backwards-compatible name for the optional quaternion implementation.
///
/// New code should accept [`Attitude`] as a type parameter instead of relying
/// on one crate-selected representation.
#[cfg(feature = "quaternion-attitude")]
pub type AttitudeState = QuaternionAttitude;

/// Symmetric spacecraft inertia tensor expressed in its attached frame.
#[derive(Debug, Clone, PartialEq)]
pub struct InertiaTensor {
    frame: SpacecraftBodyFrame,
    xx: MomentOfInertia,
    yy: MomentOfInertia,
    zz: MomentOfInertia,
    xy: MomentOfInertia,
    xz: MomentOfInertia,
    yz: MomentOfInertia,
}

impl InertiaTensor {
    /// Constructs and validates a symmetric inertia tensor in `frame`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        frame: SpacecraftBodyFrame,
        xx: MomentOfInertia,
        yy: MomentOfInertia,
        zz: MomentOfInertia,
        xy: MomentOfInertia,
        xz: MomentOfInertia,
        yz: MomentOfInertia,
    ) -> Result<Self, InertiaError> {
        let [xx_si, yy_si, zz_si, xy_si, xz_si, yz_si] =
            [xx, yy, zz, xy, xz, yz].map(|value| value.get::<kilogram_square_meter>());

        if ![xx_si, yy_si, zz_si, xy_si, xz_si, yz_si]
            .into_iter()
            .all(f64::is_finite)
        {
            return Err(InertiaError::NonFinite);
        }

        let leading_minor_2 = xx_si.mul_add(yy_si, -(xy_si * xy_si));
        let determinant = xx_si * (yy_si * zz_si - yz_si * yz_si)
            - xy_si * (xy_si * zz_si - yz_si * xz_si)
            + xz_si * (xy_si * yz_si - yy_si * xz_si);
        if xx_si <= 0.0 || leading_minor_2 <= 0.0 || determinant <= 0.0 {
            return Err(InertiaError::NotPositiveDefinite);
        }

        let matrix = Matrix3::new(
            xx_si, xy_si, xz_si, xy_si, yy_si, yz_si, xz_si, yz_si, zz_si,
        );
        let eigenvalues = matrix.symmetric_eigen().eigenvalues;
        let mut principal_moments = [eigenvalues[0], eigenvalues[1], eigenvalues[2]];
        principal_moments.sort_by(f64::total_cmp);
        let tolerance = principal_moments[2].abs() * 64.0 * f64::EPSILON;
        if principal_moments[2] > principal_moments[0] + principal_moments[1] + tolerance {
            return Err(InertiaError::ViolatesTriangleInequality);
        }

        Ok(Self {
            frame,
            xx,
            yy,
            zz,
            xy,
            xz,
            yz,
        })
    }

    /// Constructs a diagonal inertia tensor in principal axes of `frame`.
    pub fn principal(
        frame: SpacecraftBodyFrame,
        xx: MomentOfInertia,
        yy: MomentOfInertia,
        zz: MomentOfInertia,
    ) -> Result<Self, InertiaError> {
        let zero = MomentOfInertia::new::<kilogram_square_meter>(0.0);
        Self::new(frame, xx, yy, zz, zero, zero, zero)
    }

    /// Returns the frame in which the tensor is expressed.
    #[must_use]
    pub const fn frame(&self) -> ReferenceFrame {
        self.frame.reference_frame()
    }

    /// Returns the spacecraft/body ownership capability.
    #[must_use]
    pub const fn frame_capability(&self) -> &SpacecraftBodyFrame {
        &self.frame
    }

    /// Returns the symmetric tensor as a typed row-major matrix.
    #[must_use]
    pub const fn matrix(&self) -> [[MomentOfInertia; 3]; 3] {
        [
            [self.xx, self.xy, self.xz],
            [self.xy, self.yy, self.yz],
            [self.xz, self.yz, self.zz],
        ]
    }
}

/// Time-independent spacecraft identity and geometry.
///
/// Epoch-dependent quantities such as orbit, mass, inertia, and attitude
/// belong to [`SpacecraftView`], not this object.
#[derive(Debug, Clone, PartialEq)]
pub struct Spacecraft<G: SpacecraftGeometry> {
    body_frame: SpacecraftBodyFrame,
    shape: G,
}

/// Opaque proof that a frame is a particular spacecraft's body-fixed frame.
///
/// Construction requires matching application-defined origin/orientation IDs
/// and affirmatively non-inertial axes. Built-in terrestrial or celestial
/// frames therefore cannot be mislabeled as spacecraft-owned body axes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpacecraftBodyFrame {
    spacecraft_id: Arc<str>,
    reference_frame: ReferenceFrame,
}

impl SpacecraftBodyFrame {
    /// Binds application-defined body axes to one non-empty spacecraft ID.
    pub fn new(
        spacecraft_id: impl Into<String>,
        reference_frame: ReferenceFrame,
    ) -> Result<Self, SpacecraftError> {
        let spacecraft_id = spacecraft_id.into();
        if spacecraft_id.trim().is_empty() {
            return Err(SpacecraftError::EmptyId);
        }
        let owned = matches!(
            (reference_frame.origin(), reference_frame.orientation()),
            (
                FrameOrigin::Custom(origin_id),
                FrameOrientation::Custom {
                    id: orientation_id,
                    motion: FrameMotion::NonInertial,
                }
            ) if origin_id == orientation_id
        );
        if !owned {
            return Err(SpacecraftError::BodyFrameNotSpacecraftOwned);
        }
        Ok(Self {
            spacecraft_id: Arc::from(spacecraft_id),
            reference_frame,
        })
    }

    /// Returns the spacecraft ID bound to these body axes.
    #[must_use]
    pub fn spacecraft_id(&self) -> &str {
        &self.spacecraft_id
    }

    /// Returns the reference-frame identity used by framed physical values.
    #[must_use]
    pub const fn reference_frame(&self) -> ReferenceFrame {
        self.reference_frame
    }
}

impl<G: SpacecraftGeometry> Spacecraft<G> {
    /// Creates a spacecraft from already validated, spacecraft-owned body axes.
    #[must_use]
    pub const fn new(body_frame: SpacecraftBodyFrame, shape: G) -> Self {
        Self { body_frame, shape }
    }

    /// Returns the stable spacecraft identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        self.body_frame.spacecraft_id()
    }

    /// Returns the spacecraft body frame used by geometry, inertia, and attitude.
    #[must_use]
    pub const fn body_frame(&self) -> ReferenceFrame {
        self.body_frame.reference_frame()
    }

    /// Returns the validated spacecraft/body-frame binding.
    #[must_use]
    pub const fn body_frame_capability(&self) -> &SpacecraftBodyFrame {
        &self.body_frame
    }

    /// Returns the time-independent spacecraft geometry.
    #[must_use]
    pub const fn shape(&self) -> &G {
        &self.shape
    }
}

/// Time-independent spacecraft geometry expressed in body axes.
///
/// This is intentionally an open marker contract. Applications can use their
/// own geometric model without waiting for a crate-owned enum variant.
pub trait SpacecraftGeometry: std::fmt::Debug + Send + Sync {}

/// Built-in time-independent spacecraft geometry implementations.
#[cfg(feature = "standard-shapes")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpacecraftShape {
    /// Geometry is intentionally unresolved or irrelevant to the calculation.
    Point,
    /// Sphere with a strictly positive radius.
    Sphere(SphereShape),
    /// Body-axis-aligned cuboid with strictly positive x/y/z dimensions.
    Cuboid(CuboidShape),
}

/// Validated spherical spacecraft geometry.
#[cfg(feature = "standard-shapes")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SphereShape {
    radius: Length,
}

#[cfg(feature = "standard-shapes")]
impl SphereShape {
    /// Constructs a sphere with a finite, strictly positive radius.
    pub fn new(radius: Length) -> Result<Self, ShapeError> {
        validate_dimension(radius)?;
        Ok(Self { radius })
    }

    /// Returns the sphere radius.
    #[must_use]
    pub const fn radius(self) -> Length {
        self.radius
    }
}

/// Validated body-axis-aligned cuboid geometry.
#[cfg(feature = "standard-shapes")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CuboidShape {
    dimensions: [Length; 3],
}

#[cfg(feature = "standard-shapes")]
impl CuboidShape {
    /// Constructs a cuboid with finite, strictly positive x/y/z dimensions.
    pub fn new(dimensions: [Length; 3]) -> Result<Self, ShapeError> {
        for dimension in dimensions {
            validate_dimension(dimension)?;
        }
        Ok(Self { dimensions })
    }

    /// Returns the x/y/z body-axis dimensions.
    #[must_use]
    pub const fn dimensions(self) -> [Length; 3] {
        self.dimensions
    }
}

#[cfg(feature = "standard-shapes")]
impl SpacecraftShape {
    /// Constructs a spherical geometry.
    pub fn sphere(radius: Length) -> Result<Self, ShapeError> {
        SphereShape::new(radius).map(Self::Sphere)
    }

    /// Constructs a body-axis-aligned cuboid geometry.
    pub fn cuboid(dimensions: [Length; 3]) -> Result<Self, ShapeError> {
        CuboidShape::new(dimensions).map(Self::Cuboid)
    }
}

#[cfg(feature = "standard-shapes")]
impl SpacecraftGeometry for SpacecraftShape {}

#[cfg(feature = "standard-shapes")]
fn validate_dimension(dimension: Length) -> Result<(), ShapeError> {
    let metres = dimension.get::<meter>();
    if !metres.is_finite() {
        return Err(ShapeError::NonFiniteDimension);
    }
    if metres <= 0.0 {
        return Err(ShapeError::NotPositiveDimension);
    }
    Ok(())
}

/// Epoch-specific physical view of a time-independent [`Spacecraft`].
///
/// The view borrows the spacecraft definition and owns an epoch-qualified
/// orbit, geometry, and attitude representations selected by the caller.
#[derive(Debug, Clone, PartialEq)]
pub struct SpacecraftView<'a, S: SpacecraftState, G: SpacecraftGeometry, A: Attitude> {
    spacecraft: &'a Spacecraft<G>,
    orbit: Orbit<S>,
    mass: Mass,
    inertia: InertiaTensor,
    attitude: A,
}

impl<'a, S, G, A> SpacecraftView<'a, S, G, A>
where
    S: SpacecraftState,
    G: SpacecraftGeometry,
    A: Attitude,
{
    /// Composes all physical quantities valid at one epoch.
    ///
    /// The caller is responsible for supplying mass, inertia, and attitude
    /// that are valid at the orbit's epoch.
    pub fn new(
        spacecraft: &'a Spacecraft<G>,
        orbit: Orbit<S>,
        mass: Mass,
        inertia: InertiaTensor,
        attitude: A,
    ) -> Result<Self, SpacecraftViewError> {
        let mass_kg = mass.get::<kilogram>();
        if !mass_kg.is_finite() {
            return Err(SpacecraftViewError::NonFiniteMass);
        }
        if mass_kg <= 0.0 {
            return Err(SpacecraftViewError::NotPositiveMass);
        }
        if attitude.angular_velocity().body_frame_capability() != spacecraft.body_frame_capability()
            || attitude.orientation().source_frame() != spacecraft.body_frame()
        {
            return Err(SpacecraftViewError::AttitudeBodyFrameMismatch);
        }
        if inertia.frame_capability() != spacecraft.body_frame_capability() {
            return Err(SpacecraftViewError::InertiaFrameMismatch);
        }
        if attitude.orientation().target_frame() != orbit.as_ref().frame() {
            return Err(SpacecraftViewError::AttitudeReferenceFrameMismatch);
        }
        Ok(Self {
            spacecraft,
            orbit,
            mass,
            inertia,
            attitude,
        })
    }

    /// Returns the time-independent spacecraft definition.
    #[must_use]
    pub const fn spacecraft(&self) -> &'a Spacecraft<G> {
        self.spacecraft
    }

    /// Returns the view epoch.
    #[must_use]
    pub const fn epoch(&self) -> Epoch {
        self.orbit.epoch()
    }

    /// Borrows the epoch-qualified orbit in this complete view.
    #[must_use]
    pub const fn orbit(&self) -> &Orbit<S> {
        &self.orbit
    }

    /// Returns spacecraft mass at the view epoch.
    #[must_use]
    pub const fn mass(&self) -> Mass {
        self.mass
    }

    /// Returns the inertia tensor at the view epoch.
    #[must_use]
    pub const fn inertia(&self) -> &InertiaTensor {
        &self.inertia
    }

    /// Returns the attitude at the view epoch.
    #[must_use]
    pub const fn attitude(&self) -> &A {
        &self.attitude
    }
}

/// Invalid orientation input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum OrientationError {
    /// At least one quaternion component is NaN or infinite.
    #[error("orientation components must be finite")]
    NonFinite,
    /// The quaternion has no defined direction.
    #[error("orientation quaternion must have non-zero norm")]
    ZeroNorm,
}

/// Invalid attitude input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AttitudeError {
    /// At least one angular-velocity component is NaN or infinite.
    #[error("angular-velocity components must be finite")]
    NonFiniteAngularVelocity,
    /// Angular velocity is not expressed in the orientation body frame.
    #[error("angular velocity must be expressed in the attitude body frame")]
    AngularVelocityBodyFrameMismatch,
    /// Angular velocity is not relative to the orientation reference frame.
    #[error("angular velocity reference frame must match the attitude reference frame")]
    AngularVelocityReferenceFrameMismatch,
}

/// Invalid inertia tensor input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InertiaError {
    /// At least one tensor component is NaN or infinite.
    #[error("inertia tensor components must be finite")]
    NonFinite,
    /// The symmetric tensor is not positive definite.
    #[error("inertia tensor must be positive definite")]
    NotPositiveDefinite,
    /// Principal moments cannot arise from a rigid mass distribution.
    #[error("principal moments must satisfy the rigid-body triangle inequality")]
    ViolatesTriangleInequality,
}

/// Invalid time-independent spacecraft input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SpacecraftError {
    /// The spacecraft identifier contains no non-whitespace characters.
    #[error("spacecraft identifier must not be empty")]
    EmptyId,
    /// The frame is not a cohesive application-defined non-inertial body frame.
    #[error("spacecraft body frame must use one matching custom origin/orientation identity and non-inertial axes")]
    BodyFrameNotSpacecraftOwned,
}

/// Invalid time-independent spacecraft geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ShapeError {
    /// A geometry dimension is NaN or infinite.
    #[error("spacecraft shape dimensions must be finite")]
    NonFiniteDimension,
    /// A geometry dimension is zero or negative.
    #[error("spacecraft shape dimensions must be strictly positive")]
    NotPositiveDimension,
}

/// Invalid epoch-specific spacecraft view input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SpacecraftViewError {
    /// Mass is NaN or infinite.
    #[error("mass must be finite")]
    NonFiniteMass,
    /// Mass is zero or negative.
    #[error("mass must be strictly positive")]
    NotPositiveMass,
    /// Inertia is not expressed in the attitude body frame.
    #[error("inertia tensor must be expressed in the attitude body frame")]
    InertiaFrameMismatch,
    /// Attitude moving frame differs from the spacecraft body frame.
    #[error("attitude body frame must match the spacecraft body frame")]
    AttitudeBodyFrameMismatch,
    /// Attitude reference frame differs from the orbit coordinate frame.
    #[error("attitude reference frame must match the orbit frame")]
    AttitudeReferenceFrameMismatch,
}

#[cfg(all(test, feature = "quaternion-attitude", feature = "standard-shapes"))]
mod tests {
    use super::*;
    use frames::{CustomFrameId, FrameMotion, FrameOrientation, FrameOrigin};
    use units::uom::si::angular_velocity::radian_per_second;

    #[derive(Debug, Clone, PartialEq)]
    struct TestState(ReferenceFrame);

    impl SpacecraftState for TestState {
        fn frame(&self) -> ReferenceFrame {
            self.0
        }
    }

    fn body_frame(id: u64) -> ReferenceFrame {
        let id = CustomFrameId::new(id);
        ReferenceFrame::new(
            FrameOrigin::Custom(id),
            FrameOrientation::custom(id, FrameMotion::NonInertial),
        )
    }

    fn owned_body_frame(spacecraft_id: &str, id: u64) -> SpacecraftBodyFrame {
        SpacecraftBodyFrame::new(spacecraft_id, body_frame(id))
            .expect("spacecraft-owned body frame")
    }

    fn attitude(body: &SpacecraftBodyFrame) -> AttitudeState {
        let reference_frame = body.reference_frame();
        QuaternionAttitude::new(
            Orientation::identity(reference_frame, ReferenceFrame::GCRF),
            BodyAngularVelocity::new(
                AngularVelocityVector::from_radians_per_second(0.1, 0.2, 0.3),
                body.clone(),
                ReferenceFrame::GCRF,
            )
            .expect("finite angular velocity"),
        )
        .expect("consistent frames")
    }

    fn inertia(body: &SpacecraftBodyFrame) -> InertiaTensor {
        InertiaTensor::principal(
            body.clone(),
            MomentOfInertia::new::<kilogram_square_meter>(1_000.0),
            MomentOfInertia::new::<kilogram_square_meter>(1_200.0),
            MomentOfInertia::new::<kilogram_square_meter>(800.0),
        )
        .expect("physical inertia")
    }

    fn state() -> TestState {
        TestState(ReferenceFrame::GCRF)
    }

    #[test]
    fn attitude_exposes_angles_and_angular_speeds() {
        let body = owned_body_frame("SC-001", 1);
        let attitude = attitude(&body);

        assert_eq!(attitude.angles(), [Angle::new::<radian>(0.0); 3]);
        assert_eq!(
            attitude.angular_speeds(),
            [
                AngularVelocity::new::<radian_per_second>(0.1),
                AngularVelocity::new::<radian_per_second>(0.2),
                AngularVelocity::new::<radian_per_second>(0.3),
            ]
        );
    }

    #[test]
    fn spacecraft_contains_only_time_independent_identity_and_geometry() {
        let shape = SpacecraftShape::sphere(Length::new::<meter>(1.5)).expect("valid shape");
        let body = body_frame(1);
        let spacecraft = Spacecraft::new(owned_body_frame("SC-001", 1), shape);

        assert_eq!(spacecraft.id(), "SC-001");
        assert_eq!(spacecraft.shape(), &shape);
        let SpacecraftShape::Sphere(sphere) = shape else {
            panic!("sphere constructor must return a sphere")
        };
        assert_eq!(sphere.radius(), Length::new::<meter>(1.5));
        assert_eq!(
            SpacecraftBodyFrame::new("  ", body),
            Err(SpacecraftError::EmptyId)
        );
        assert_eq!(
            SpacecraftShape::sphere(Length::new::<meter>(0.0)),
            Err(ShapeError::NotPositiveDimension)
        );
        assert_eq!(
            CuboidShape::new([
                Length::new::<meter>(1.0),
                Length::new::<meter>(f64::INFINITY),
                Length::new::<meter>(1.0),
            ]),
            Err(ShapeError::NonFiniteDimension)
        );
    }

    #[test]
    fn spacecraft_view_composes_epoch_dependent_physical_data() {
        let body = body_frame(1);
        let owned_body = owned_body_frame("SC-001", 1);
        let spacecraft = Spacecraft::new(owned_body.clone(), SpacecraftShape::Point);
        let attitude = attitude(&owned_body);
        let view = SpacecraftView::new(
            &spacecraft,
            Orbit::new(Epoch::from_tai_seconds(42.0), state()),
            Mass::new::<kilogram>(500.0),
            inertia(&owned_body),
            attitude,
        )
        .expect("consistent view");

        assert_eq!(view.spacecraft(), &spacecraft);
        assert_eq!(view.epoch(), Epoch::from_tai_seconds(42.0));
        assert_eq!(
            view.orbit(),
            &Orbit::new(Epoch::from_tai_seconds(42.0), state())
        );
        assert_eq!(view.mass(), Mass::new::<kilogram>(500.0));
        assert_eq!(view.orbit().as_ref().frame(), ReferenceFrame::GCRF);
        assert_eq!(view.inertia().frame(), body);
        assert_eq!(view.attitude().orientation().source_frame(), body);
    }

    #[test]
    fn spacecraft_view_accepts_application_geometry_and_attitude() {
        #[derive(Debug, Clone, PartialEq)]
        struct PanelMesh;

        impl SpacecraftGeometry for PanelMesh {}

        #[derive(Debug, Clone, PartialEq)]
        struct EstimatedAttitude(QuaternionAttitude);

        impl Attitude for EstimatedAttitude {
            fn orientation(&self) -> &Orientation {
                self.0.orientation()
            }

            fn angular_velocity(&self) -> &BodyAngularVelocity {
                self.0.angular_velocity()
            }
        }

        let owned_body = owned_body_frame("SC-001", 1);
        let spacecraft = Spacecraft::new(owned_body.clone(), PanelMesh);
        let view = SpacecraftView::new(
            &spacecraft,
            Orbit::new(Epoch::from_tai_seconds(42.0), state()),
            Mass::new::<kilogram>(500.0),
            inertia(&owned_body),
            EstimatedAttitude(attitude(&owned_body)),
        )
        .expect("application implementations are accepted");

        assert_eq!(view.spacecraft().shape(), &PanelMesh);
        assert_eq!(
            view.attitude().orientation().source_frame(),
            owned_body.reference_frame()
        );
    }

    #[test]
    fn rigid_body_frames_and_mass_are_validated() {
        let body = body_frame(1);
        let owned_body = owned_body_frame("SC-001", 1);
        let other_owned_body = owned_body_frame("SC-002", 2);
        let spacecraft = Spacecraft::new(owned_body.clone(), SpacecraftShape::Point);
        let valid_attitude = attitude(&owned_body);
        let same_axes_other_owner =
            SpacecraftBodyFrame::new("SC-002", body).expect("same axes, different owner");
        assert_eq!(
            SpacecraftBodyFrame::new("BAD", ReferenceFrame::ITRF2020),
            Err(SpacecraftError::BodyFrameNotSpacecraftOwned)
        );
        assert_eq!(
            SpacecraftBodyFrame::new(
                "BAD",
                ReferenceFrame::new(
                    FrameOrigin::Custom(CustomFrameId::new(1)),
                    FrameOrientation::custom(CustomFrameId::new(2), FrameMotion::NonInertial,),
                ),
            ),
            Err(SpacecraftError::BodyFrameNotSpacecraftOwned)
        );
        assert!(matches!(
            SpacecraftView::new(
                &spacecraft,
                Orbit::new(Epoch::from_tai_seconds(0.0), state()),
                Mass::new::<kilogram>(500.0),
                inertia(&owned_body),
                attitude(&other_owned_body),
            ),
            Err(SpacecraftViewError::AttitudeBodyFrameMismatch)
        ));
        assert!(matches!(
            SpacecraftView::new(
                &spacecraft,
                Orbit::new(Epoch::from_tai_seconds(0.0), state()),
                Mass::new::<kilogram>(500.0),
                inertia(&owned_body),
                attitude(&same_axes_other_owner),
            ),
            Err(SpacecraftViewError::AttitudeBodyFrameMismatch)
        ));
        assert!(matches!(
            SpacecraftView::new(
                &spacecraft,
                Orbit::new(Epoch::from_tai_seconds(0.0), state()),
                Mass::new::<kilogram>(500.0),
                inertia(&same_axes_other_owner),
                valid_attitude.clone(),
            ),
            Err(SpacecraftViewError::InertiaFrameMismatch)
        ));
        let wrong_reference = AttitudeState::new(
            Orientation::identity(body, ReferenceFrame::EME2000),
            BodyAngularVelocity::new(
                AngularVelocityVector::from_radians_per_second(0.0, 0.0, 0.0),
                owned_body.clone(),
                ReferenceFrame::EME2000,
            )
            .expect("finite angular velocity"),
        )
        .expect("internally consistent attitude");
        assert!(matches!(
            SpacecraftView::new(
                &spacecraft,
                Orbit::new(Epoch::from_tai_seconds(0.0), state()),
                Mass::new::<kilogram>(500.0),
                inertia(&owned_body),
                wrong_reference,
            ),
            Err(SpacecraftViewError::AttitudeReferenceFrameMismatch)
        ));
        assert!(matches!(
            SpacecraftView::new(
                &spacecraft,
                Orbit::new(Epoch::from_tai_seconds(0.0), state()),
                Mass::new::<kilogram>(500.0),
                inertia(&other_owned_body),
                valid_attitude.clone(),
            ),
            Err(SpacecraftViewError::InertiaFrameMismatch)
        ));
        assert!(matches!(
            SpacecraftView::new(
                &spacecraft,
                Orbit::new(Epoch::from_tai_seconds(0.0), state()),
                Mass::new::<kilogram>(0.0),
                inertia(&owned_body),
                valid_attitude,
            ),
            Err(SpacecraftViewError::NotPositiveMass)
        ));

        assert_eq!(
            AttitudeState::new(
                Orientation::identity(body, ReferenceFrame::GCRF),
                BodyAngularVelocity::new(
                    AngularVelocityVector::from_radians_per_second(0.0, 0.0, 0.0),
                    other_owned_body.clone(),
                    ReferenceFrame::GCRF,
                )
                .expect("finite angular velocity"),
            ),
            Err(AttitudeError::AngularVelocityBodyFrameMismatch)
        );
        assert_eq!(
            AttitudeState::new(
                Orientation::identity(body, ReferenceFrame::GCRF),
                BodyAngularVelocity::new(
                    AngularVelocityVector::from_radians_per_second(0.0, 0.0, 0.0),
                    owned_body,
                    ReferenceFrame::EME2000,
                )
                .expect("finite angular velocity"),
            ),
            Err(AttitudeError::AngularVelocityReferenceFrameMismatch)
        );
    }

    #[test]
    fn quaternion_and_inertia_validation_remain_explicit() {
        let body = body_frame(1);
        let owned_body = owned_body_frame("SC-001", 1);
        assert_eq!(
            Orientation::try_from((body, ReferenceFrame::GCRF, [Ratio::new::<ratio>(0.0); 4],)),
            Err(OrientationError::ZeroNorm)
        );
        assert_eq!(
            InertiaTensor::principal(
                owned_body,
                MomentOfInertia::new::<kilogram_square_meter>(1.0),
                MomentOfInertia::new::<kilogram_square_meter>(1.0),
                MomentOfInertia::new::<kilogram_square_meter>(3.0),
            ),
            Err(InertiaError::ViolatesTriangleInequality)
        );
    }
}
