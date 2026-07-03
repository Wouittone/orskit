use hifitime::Epoch;
use nalgebra::{Matrix3, Quaternion, UnitQuaternion};
use orskit_frames::ReferenceFrame;
use orskit_units::uom::si::{
    mass::kilogram, moment_of_inertia::kilogram_square_meter, ratio::ratio,
};
use orskit_units::{Mass, MomentOfInertia, Ratio, Velocity};
use thiserror::Error;

use crate::{FramedPosition, FramedVelocity};

/// Complete spacecraft state at one epoch.
///
/// Position and velocity each carry their own reference frame; they are not
/// required to use the same one. Optional orientation and inertia values also
/// carry the frames needed to interpret them.
#[derive(Debug, Clone, PartialEq)]
pub struct SpacecraftState {
    epoch: Epoch,
    position: FramedPosition,
    velocity: FramedVelocity,
    mass: Mass,
    orientation: Option<Orientation>,
    inertia: Option<InertiaTensor>,
}

impl SpacecraftState {
    /// Constructs a translational spacecraft state.
    ///
    /// Orientation and inertia are absent until added with
    /// [`Self::with_orientation`] and [`Self::with_inertia`]. Position and
    /// velocity may be expressed in different frames.
    pub fn new(
        epoch: Epoch,
        position: FramedPosition,
        velocity: FramedVelocity,
        mass: Mass,
    ) -> Result<Self, StateError> {
        let mass_kg = mass.get::<kilogram>();
        if !mass_kg.is_finite() {
            return Err(StateError::NonFiniteMass);
        }
        if mass_kg <= 0.0 {
            return Err(StateError::NotPositiveMass);
        }

        Ok(Self {
            epoch,
            position,
            velocity,
            mass,
            orientation: None,
            inertia: None,
        })
    }

    /// Adds or replaces the orientation.
    #[must_use]
    pub fn with_orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = Some(orientation);
        self
    }

    /// Adds or replaces the framed inertia tensor.
    #[must_use]
    pub fn with_inertia(mut self, inertia: InertiaTensor) -> Self {
        self.inertia = Some(inertia);
        self
    }

    /// Returns the epoch shared by all state values.
    #[must_use]
    pub const fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Returns the framed position.
    #[must_use]
    pub const fn position(&self) -> FramedPosition {
        self.position
    }

    /// Returns the independently framed velocity.
    #[must_use]
    pub const fn velocity(&self) -> FramedVelocity {
        self.velocity
    }

    /// Returns the scalar speed.
    #[must_use]
    pub fn speed(&self) -> Velocity {
        self.velocity.speed()
    }

    /// Returns the spacecraft mass.
    #[must_use]
    pub const fn mass(&self) -> Mass {
        self.mass
    }

    /// Returns the optional framed orientation.
    #[must_use]
    pub const fn orientation(&self) -> Option<&Orientation> {
        self.orientation.as_ref()
    }

    /// Returns the optional framed inertia tensor.
    #[must_use]
    pub const fn inertia(&self) -> Option<&InertiaTensor> {
        self.inertia.as_ref()
    }
}

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
    /// Constructs an orientation from dimensionless quaternion components in
    /// scalar/i/j/k order. The quaternion is normalized after validation.
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

    /// Returns normalized scalar/i/j/k quaternion components as dimensionless
    /// typed ratios.
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
    ///
    /// Arguments are the three diagonal and three unique off-diagonal terms.
    /// Positive definiteness is checked with Sylvester's criterion. The
    /// principal moments must also satisfy the rigid-body triangle inequality.
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

/// Invalid spacecraft state input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum StateError {
    /// Mass is NaN or infinite.
    #[error("mass must be finite")]
    NonFiniteMass,
    /// Mass is zero or negative.
    #[error("mass must be strictly positive")]
    NotPositiveMass,
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

#[cfg(test)]
mod tests {
    use super::*;
    use orskit_frames::{CustomFrameId, FrameOrientation, FrameOrigin};
    use orskit_units::uom::si::{
        length::kilometer, mass::kilogram, velocity::kilometer_per_second,
    };
    use orskit_units::{Length, Position, Velocity, VelocityVector};

    fn body_frame() -> ReferenceFrame {
        let id = CustomFrameId::new(1);
        ReferenceFrame::new(FrameOrigin::Custom(id), FrameOrientation::Custom(id))
    }

    fn position(frame: ReferenceFrame) -> FramedPosition {
        FramedPosition::new(
            Position::new(
                Length::new::<kilometer>(7_000.0),
                Length::new::<kilometer>(0.0),
                Length::new::<kilometer>(0.0),
            ),
            frame,
        )
        .expect("fixture position is finite")
    }

    fn velocity(frame: ReferenceFrame) -> FramedVelocity {
        FramedVelocity::new(
            VelocityVector::new(
                Velocity::new::<kilometer_per_second>(0.0),
                Velocity::new::<kilometer_per_second>(7.5),
                Velocity::new::<kilometer_per_second>(0.0),
            ),
            frame,
        )
        .expect("fixture velocity is finite")
    }

    fn translational_state() -> SpacecraftState {
        SpacecraftState::new(
            Epoch::from_tai_seconds(0.0),
            position(ReferenceFrame::GCRF),
            velocity(ReferenceFrame::GCRF),
            Mass::new::<kilogram>(1_000.0),
        )
        .expect("fixture is physically valid")
    }

    #[test]
    fn spacecraft_state_keeps_independent_kinematic_frames() {
        let state = SpacecraftState::new(
            Epoch::from_tai_seconds(0.0),
            position(ReferenceFrame::GCRF),
            velocity(ReferenceFrame::EME2000),
            Mass::new::<kilogram>(1_000.0),
        )
        .expect("different kinematic frames are valid state data");

        assert_eq!(state.position().frame(), ReferenceFrame::GCRF);
        assert_eq!(state.velocity().frame(), ReferenceFrame::EME2000);
        assert_eq!(state.epoch(), Epoch::from_tai_seconds(0.0));
        assert_eq!(state.mass(), Mass::new::<kilogram>(1_000.0));
        assert_eq!(state.speed(), Velocity::new::<kilometer_per_second>(7.5));
    }

    #[test]
    fn optional_rigid_body_state_keeps_its_own_frames() {
        let body = body_frame();
        let inertia = InertiaTensor::principal(
            body,
            MomentOfInertia::new::<kilogram_square_meter>(1_000.0),
            MomentOfInertia::new::<kilogram_square_meter>(1_200.0),
            MomentOfInertia::new::<kilogram_square_meter>(800.0),
        )
        .expect("principal moments are physical");
        let orientation = Orientation::identity(body, ReferenceFrame::ITRF2020);
        let state = translational_state()
            .with_orientation(orientation)
            .with_inertia(inertia);

        assert_eq!(state.orientation().map(Orientation::from_frame), Some(body));
        assert_eq!(
            state.orientation().map(Orientation::to_frame),
            Some(ReferenceFrame::ITRF2020)
        );
        assert_eq!(state.inertia().map(|value| value.frame()), Some(body));
    }

    #[test]
    fn invalid_mass_is_rejected() {
        let error = SpacecraftState::new(
            Epoch::from_tai_seconds(0.0),
            position(ReferenceFrame::GCRF),
            velocity(ReferenceFrame::GCRF),
            Mass::new::<kilogram>(0.0),
        )
        .expect_err("zero mass is invalid");
        assert_eq!(error, StateError::NotPositiveMass);
    }

    #[test]
    fn quaternion_is_normalized_and_zero_is_rejected() {
        let from_frame = body_frame();
        let to_frame = ReferenceFrame::GCRF;
        let orientation = Orientation::from_quaternion(
            from_frame,
            to_frame,
            Ratio::new::<ratio>(2.0),
            Ratio::new::<ratio>(0.0),
            Ratio::new::<ratio>(0.0),
            Ratio::new::<ratio>(0.0),
        )
        .expect("non-zero quaternion is valid");
        assert_eq!(orientation, Orientation::identity(from_frame, to_frame));

        assert_eq!(
            Orientation::from_quaternion(
                from_frame,
                to_frame,
                Ratio::new::<ratio>(0.0),
                Ratio::new::<ratio>(0.0),
                Ratio::new::<ratio>(0.0),
                Ratio::new::<ratio>(0.0),
            ),
            Err(OrientationError::ZeroNorm)
        );
    }

    #[test]
    fn non_positive_definite_inertia_is_rejected() {
        assert_eq!(
            InertiaTensor::principal(
                body_frame(),
                MomentOfInertia::new::<kilogram_square_meter>(1.0),
                MomentOfInertia::new::<kilogram_square_meter>(-1.0),
                MomentOfInertia::new::<kilogram_square_meter>(1.0),
            ),
            Err(InertiaError::NotPositiveDefinite)
        );
    }

    #[test]
    fn non_physical_principal_moments_are_rejected() {
        assert_eq!(
            InertiaTensor::principal(
                body_frame(),
                MomentOfInertia::new::<kilogram_square_meter>(1.0),
                MomentOfInertia::new::<kilogram_square_meter>(1.0),
                MomentOfInertia::new::<kilogram_square_meter>(3.0),
            ),
            Err(InertiaError::ViolatesTriangleInequality)
        );
    }
}
