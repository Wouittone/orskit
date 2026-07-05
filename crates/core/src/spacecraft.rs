use hifitime::Epoch;
use nalgebra::{Matrix3, Quaternion, UnitQuaternion};
use orskit_frames::ReferenceFrame;
use orskit_units::uom::si::{
    angle::radian, length::meter, mass::kilogram, moment_of_inertia::kilogram_square_meter,
    ratio::ratio,
};
use orskit_units::{
    Angle, AngularVelocity, AngularVelocityVector, Length, Mass, MomentOfInertia, Ratio,
};
use thiserror::Error;

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

impl Orientation {
    /// Constructs an orientation from scalar/i/j/k quaternion components.
    pub fn from_quaternion(
        from_frame: ReferenceFrame,
        to_frame: ReferenceFrame,
        scalar: Ratio,
        i: Ratio,
        j: Ratio,
        k: Ratio,
    ) -> Result<Self, OrientationError> {
        let values = [
            scalar.get::<ratio>(),
            i.get::<ratio>(),
            j.get::<ratio>(),
            k.get::<ratio>(),
        ];
        if !values.into_iter().all(f64::is_finite) {
            return Err(OrientationError::NonFinite);
        }

        let quaternion = Quaternion::new(values[0], values[1], values[2], values[3]);
        if quaternion.norm_squared() <= f64::EPSILON {
            return Err(OrientationError::ZeroNorm);
        }

        Ok(Self {
            rotation: UnitQuaternion::new_normalize(quaternion),
            from_frame,
            to_frame,
        })
    }

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
    pub const fn from_frame(&self) -> ReferenceFrame {
        self.from_frame
    }

    /// Returns the frame whose components this rotation produces.
    #[must_use]
    pub const fn to_frame(&self) -> ReferenceFrame {
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

/// Angular velocity expressed in an explicit frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FramedAngularVelocity {
    value: AngularVelocityVector,
    frame: ReferenceFrame,
}

impl FramedAngularVelocity {
    /// Attaches a frame to a finite angular-velocity vector.
    pub fn new(value: AngularVelocityVector, frame: ReferenceFrame) -> Result<Self, AttitudeError> {
        if !value.is_finite() {
            return Err(AttitudeError::NonFiniteAngularVelocity);
        }
        Ok(Self { value, frame })
    }

    /// Returns the angular-velocity vector.
    #[must_use]
    pub const fn value(self) -> AngularVelocityVector {
        self.value
    }

    /// Returns the expression frame.
    #[must_use]
    pub const fn frame(self) -> ReferenceFrame {
        self.frame
    }

    /// Returns angular speeds about x/y/z.
    #[must_use]
    pub const fn components(self) -> [AngularVelocity; 3] {
        self.value.components()
    }
}

/// Immutable attitude represented by a quaternion and body angular velocity.
#[derive(Debug, Clone, PartialEq)]
pub struct QuaternionAttitude {
    orientation: Orientation,
    angular_velocity: FramedAngularVelocity,
}

impl QuaternionAttitude {
    /// Constructs an attitude with angular velocity in the orientation's body frame.
    pub fn new(
        orientation: Orientation,
        angular_velocity: FramedAngularVelocity,
    ) -> Result<Self, AttitudeError> {
        if orientation.from_frame() != angular_velocity.frame() {
            return Err(AttitudeError::AngularVelocityFrameMismatch);
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
    pub const fn angular_velocity(&self) -> FramedAngularVelocity {
        self.angular_velocity
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

/// Closed set of supported spacecraft attitude representations.
#[derive(Debug, Clone, PartialEq)]
pub enum AttitudeState {
    /// Quaternion orientation and body angular velocity.
    Quaternion(QuaternionAttitude),
}

impl AttitudeState {
    /// Constructs the current quaternion attitude representation.
    pub fn new(
        orientation: Orientation,
        angular_velocity: FramedAngularVelocity,
    ) -> Result<Self, AttitudeError> {
        Ok(Self::Quaternion(QuaternionAttitude::new(
            orientation,
            angular_velocity,
        )?))
    }

    /// Returns the body-to-reference orientation in any representation.
    #[must_use]
    pub const fn orientation(&self) -> &Orientation {
        match self {
            Self::Quaternion(attitude) => attitude.orientation(),
        }
    }

    /// Returns body angular velocity in any representation.
    #[must_use]
    pub const fn angular_velocity(&self) -> FramedAngularVelocity {
        match self {
            Self::Quaternion(attitude) => attitude.angular_velocity(),
        }
    }

    /// Returns intrinsic roll/pitch/yaw angles about x/y/z.
    #[must_use]
    pub fn angles(&self) -> [Angle; 3] {
        match self {
            Self::Quaternion(attitude) => attitude.angles(),
        }
    }

    /// Returns angular speeds about body x/y/z.
    #[must_use]
    pub const fn angular_speeds(&self) -> [AngularVelocity; 3] {
        match self {
            Self::Quaternion(attitude) => attitude.angular_speeds(),
        }
    }
}

impl From<QuaternionAttitude> for AttitudeState {
    fn from(attitude: QuaternionAttitude) -> Self {
        Self::Quaternion(attitude)
    }
}

/// Symmetric spacecraft inertia tensor expressed in its attached frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InertiaTensor {
    frame: ReferenceFrame,
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
        frame: ReferenceFrame,
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
        frame: ReferenceFrame,
        xx: MomentOfInertia,
        yy: MomentOfInertia,
        zz: MomentOfInertia,
    ) -> Result<Self, InertiaError> {
        let zero = MomentOfInertia::new::<kilogram_square_meter>(0.0);
        Self::new(frame, xx, yy, zz, zero, zero, zero)
    }

    /// Returns the frame in which the tensor is expressed.
    #[must_use]
    pub const fn frame(self) -> ReferenceFrame {
        self.frame
    }

    /// Returns the symmetric tensor as a typed row-major matrix.
    #[must_use]
    pub const fn matrix(self) -> [[MomentOfInertia; 3]; 3] {
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
pub struct Spacecraft {
    id: String,
    shape: SpacecraftShape,
}

impl Spacecraft {
    /// Creates a spacecraft with a stable non-empty identifier and geometry.
    pub fn new(id: impl Into<String>, shape: SpacecraftShape) -> Result<Self, SpacecraftError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(SpacecraftError::EmptyId);
        }
        Ok(Self { id, shape })
    }

    /// Returns the stable spacecraft identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the time-independent spacecraft geometry.
    #[must_use]
    pub const fn shape(&self) -> SpacecraftShape {
        self.shape
    }
}

/// Time-independent spacecraft geometry expressed in body axes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpacecraftShape {
    /// Geometry is intentionally unresolved or irrelevant to the calculation.
    Point,
    /// Sphere with a strictly positive radius.
    Sphere { radius: Length },
    /// Body-axis-aligned cuboid with strictly positive x/y/z dimensions.
    Cuboid { dimensions: [Length; 3] },
}

impl SpacecraftShape {
    /// Constructs a spherical geometry.
    pub fn sphere(radius: Length) -> Result<Self, ShapeError> {
        validate_dimension(radius)?;
        Ok(Self::Sphere { radius })
    }

    /// Constructs a body-axis-aligned cuboid geometry.
    pub fn cuboid(dimensions: [Length; 3]) -> Result<Self, ShapeError> {
        for dimension in dimensions {
            validate_dimension(dimension)?;
        }
        Ok(Self::Cuboid { dimensions })
    }
}

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
/// orbit plus a closed attitude state. It is not generic over any physical
/// representation.
#[derive(Debug, Clone, PartialEq)]
pub struct SpacecraftView<'a> {
    spacecraft: &'a Spacecraft,
    orbit: Orbit,
    mass: Mass,
    inertia: InertiaTensor,
    attitude: AttitudeState,
}

impl<'a> SpacecraftView<'a> {
    /// Composes all physical quantities valid at one epoch.
    ///
    /// The caller is responsible for supplying mass, inertia, and attitude
    /// that are valid at the orbit's epoch.
    pub fn new(
        spacecraft: &'a Spacecraft,
        orbit: Orbit,
        mass: Mass,
        inertia: InertiaTensor,
        attitude: AttitudeState,
    ) -> Result<Self, SpacecraftViewError> {
        let mass_kg = mass.get::<kilogram>();
        if !mass_kg.is_finite() {
            return Err(SpacecraftViewError::NonFiniteMass);
        }
        if mass_kg <= 0.0 {
            return Err(SpacecraftViewError::NotPositiveMass);
        }
        if inertia.frame() != attitude.orientation().from_frame() {
            return Err(SpacecraftViewError::InertiaFrameMismatch);
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
    pub const fn spacecraft(&self) -> &'a Spacecraft {
        self.spacecraft
    }

    /// Returns the view epoch.
    #[must_use]
    pub const fn epoch(&self) -> Epoch {
        self.orbit.epoch()
    }

    /// Returns the epoch-qualified orbit in this complete view.
    #[must_use]
    pub const fn orbit(&self) -> Orbit {
        self.orbit
    }

    /// Returns spacecraft mass at the view epoch.
    #[must_use]
    pub const fn mass(&self) -> Mass {
        self.mass
    }

    /// Returns the orbital state at the view epoch.
    #[must_use]
    pub const fn state(&self) -> SpacecraftState {
        self.orbit.state()
    }

    /// Returns the inertia tensor at the view epoch.
    #[must_use]
    pub const fn inertia(&self) -> InertiaTensor {
        self.inertia
    }

    /// Returns the attitude at the view epoch.
    #[must_use]
    pub const fn attitude(&self) -> &AttitudeState {
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
    AngularVelocityFrameMismatch,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use orskit_frames::{CustomFrameId, FrameOrientation, FrameOrigin};
    use orskit_units::uom::si::{angular_velocity::radian_per_second, velocity::meter_per_second};
    use orskit_units::{Position, VelocityVector};

    fn body_frame(id: u64) -> ReferenceFrame {
        let id = CustomFrameId::new(id);
        ReferenceFrame::new(FrameOrigin::Custom(id), FrameOrientation::Custom(id))
    }

    fn attitude(body: ReferenceFrame) -> AttitudeState {
        QuaternionAttitude::new(
            Orientation::identity(body, ReferenceFrame::GCRF),
            FramedAngularVelocity::new(
                AngularVelocityVector::from_radians_per_second(0.1, 0.2, 0.3),
                body,
            )
            .expect("finite angular velocity"),
        )
        .expect("consistent frames")
        .into()
    }

    fn inertia(body: ReferenceFrame) -> InertiaTensor {
        InertiaTensor::principal(
            body,
            MomentOfInertia::new::<kilogram_square_meter>(1_000.0),
            MomentOfInertia::new::<kilogram_square_meter>(1_200.0),
            MomentOfInertia::new::<kilogram_square_meter>(800.0),
        )
        .expect("physical inertia")
    }

    fn state() -> SpacecraftState {
        crate::CartesianState::new(
            ReferenceFrame::GCRF,
            Position::from_metres(7_000_000.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 7_500.0, 0.0),
        )
        .expect("finite state")
        .into()
    }

    #[test]
    fn attitude_exposes_angles_and_angular_speeds() {
        let body = body_frame(1);
        let attitude = attitude(body);

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
        let spacecraft = Spacecraft::new("SC-001", shape).expect("non-empty id");

        assert_eq!(spacecraft.id(), "SC-001");
        assert_eq!(spacecraft.shape(), shape);
        assert_eq!(
            Spacecraft::new("  ", SpacecraftShape::Point),
            Err(SpacecraftError::EmptyId)
        );
        assert_eq!(
            SpacecraftShape::sphere(Length::new::<meter>(0.0)),
            Err(ShapeError::NotPositiveDimension)
        );
    }

    #[test]
    fn spacecraft_view_composes_epoch_dependent_physical_data() {
        let body = body_frame(1);
        let spacecraft = Spacecraft::new("SC-001", SpacecraftShape::Point).expect("valid craft");
        let attitude = attitude(body);
        let view = SpacecraftView::new(
            &spacecraft,
            Orbit::new(Epoch::from_tai_seconds(42.0), state()),
            Mass::new::<kilogram>(500.0),
            inertia(body),
            attitude,
        )
        .expect("consistent view");

        assert_eq!(view.spacecraft(), &spacecraft);
        assert_eq!(view.epoch(), Epoch::from_tai_seconds(42.0));
        assert_eq!(
            view.orbit(),
            Orbit::new(Epoch::from_tai_seconds(42.0), state())
        );
        assert_eq!(view.mass(), Mass::new::<kilogram>(500.0));
        assert_eq!(
            match view.state() {
                SpacecraftState::Cartesian(state) => state.speed().get::<meter_per_second>(),
                _ => unreachable!("fixture is Cartesian"),
            },
            7_500.0
        );
        assert_eq!(view.inertia().frame(), body);
        assert_eq!(view.attitude().orientation().from_frame(), body);
    }

    #[test]
    fn rigid_body_frames_and_mass_are_validated() {
        let body = body_frame(1);
        let other = body_frame(2);
        let spacecraft = Spacecraft::new("SC-001", SpacecraftShape::Point).expect("valid craft");
        let valid_attitude = attitude(body);
        assert!(matches!(
            SpacecraftView::new(
                &spacecraft,
                Orbit::new(Epoch::from_tai_seconds(0.0), state()),
                Mass::new::<kilogram>(500.0),
                inertia(other),
                valid_attitude.clone(),
            ),
            Err(SpacecraftViewError::InertiaFrameMismatch)
        ));
        assert!(matches!(
            SpacecraftView::new(
                &spacecraft,
                Orbit::new(Epoch::from_tai_seconds(0.0), state()),
                Mass::new::<kilogram>(0.0),
                inertia(body),
                valid_attitude,
            ),
            Err(SpacecraftViewError::NotPositiveMass)
        ));

        assert_eq!(
            AttitudeState::new(
                Orientation::identity(body, ReferenceFrame::GCRF),
                FramedAngularVelocity::new(
                    AngularVelocityVector::from_radians_per_second(0.0, 0.0, 0.0),
                    other,
                )
                .expect("finite angular velocity"),
            ),
            Err(AttitudeError::AngularVelocityFrameMismatch)
        );
    }

    #[test]
    fn quaternion_and_inertia_validation_remain_explicit() {
        let body = body_frame(1);
        assert_eq!(
            Orientation::from_quaternion(
                body,
                ReferenceFrame::GCRF,
                Ratio::new::<ratio>(0.0),
                Ratio::new::<ratio>(0.0),
                Ratio::new::<ratio>(0.0),
                Ratio::new::<ratio>(0.0),
            ),
            Err(OrientationError::ZeroNorm)
        );
        assert_eq!(
            InertiaTensor::principal(
                body,
                MomentOfInertia::new::<kilogram_square_meter>(1.0),
                MomentOfInertia::new::<kilogram_square_meter>(1.0),
                MomentOfInertia::new::<kilogram_square_meter>(3.0),
            ),
            Err(InertiaError::ViolatesTriangleInequality)
        );
    }
}
