use hifitime::Epoch;
use nalgebra::{Matrix3, Quaternion, UnitQuaternion};
use orskit_frames::ReferenceFrame;
use orskit_units::uom::si::{
    mass::kilogram, moment_of_inertia::kilogram_square_meter, ratio::ratio,
};
use orskit_units::{Mass, MomentOfInertia, Position, Ratio, Velocity, VelocityVector};
use thiserror::Error;

/// Complete spacecraft state at one epoch in one reference frame.
///
/// Position and velocity are expressed in [`Self::frame`]. Orientation, when
/// present, rotates vectors from the spacecraft body frame into that reference
/// frame. The inertia tensor, when present, is expressed in the spacecraft body
/// frame.
#[derive(Debug, Clone, PartialEq)]
pub struct SpacecraftState {
    epoch: Epoch,
    frame: ReferenceFrame,
    position: Position,
    velocity: VelocityVector,
    mass: Mass,
    orientation: Option<Orientation>,
    inertia: Option<InertiaTensor>,
}

impl SpacecraftState {
    /// Constructs a translational spacecraft state.
    ///
    /// Orientation and inertia are absent until added with
    /// [`Self::with_orientation`] and [`Self::with_inertia`].
    pub fn new(
        epoch: Epoch,
        frame: ReferenceFrame,
        position: Position,
        velocity: VelocityVector,
        mass: Mass,
    ) -> Result<Self, StateError> {
        if !position.is_finite() {
            return Err(StateError::NonFinitePosition);
        }
        if !velocity.is_finite() {
            return Err(StateError::NonFiniteVelocity);
        }

        let mass_kg = mass.get::<kilogram>();
        if !mass_kg.is_finite() {
            return Err(StateError::NonFiniteMass);
        }
        if mass_kg <= 0.0 {
            return Err(StateError::NotPositiveMass);
        }

        Ok(Self {
            epoch,
            frame,
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

    /// Adds or replaces the body-frame inertia tensor.
    #[must_use]
    pub fn with_inertia(mut self, inertia: InertiaTensor) -> Self {
        self.inertia = Some(inertia);
        self
    }

    /// Returns the epoch.
    #[must_use]
    pub const fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Returns the reference frame for position, velocity, and orientation.
    #[must_use]
    pub const fn frame(&self) -> ReferenceFrame {
        self.frame
    }

    /// Returns the position.
    #[must_use]
    pub const fn position(&self) -> Position {
        self.position
    }

    /// Returns the velocity vector.
    #[must_use]
    pub const fn velocity(&self) -> VelocityVector {
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

    /// Returns the optional body-to-reference orientation.
    #[must_use]
    pub const fn orientation(&self) -> Option<&Orientation> {
        self.orientation.as_ref()
    }

    /// Returns the optional body-frame inertia tensor.
    #[must_use]
    pub const fn inertia(&self) -> Option<&InertiaTensor> {
        self.inertia.as_ref()
    }
}

/// A normalized rotation from the spacecraft body frame to a reference frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Orientation(UnitQuaternion<f64>);

impl Orientation {
    /// Constructs an orientation from dimensionless quaternion components in
    /// scalar/i/j/k order. The quaternion is normalized after validation.
    pub fn from_quaternion(
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

        Ok(Self(UnitQuaternion::new_normalize(quaternion)))
    }

    /// Identity body-to-reference orientation.
    #[must_use]
    pub fn identity() -> Self {
        Self(UnitQuaternion::identity())
    }

    /// Returns normalized scalar/i/j/k quaternion components as dimensionless
    /// typed ratios.
    #[must_use]
    pub fn quaternion(&self) -> [Ratio; 4] {
        let quaternion = self.0.quaternion();
        [
            Ratio::new::<ratio>(quaternion.w),
            Ratio::new::<ratio>(quaternion.i),
            Ratio::new::<ratio>(quaternion.j),
            Ratio::new::<ratio>(quaternion.k),
        ]
    }
}

/// Symmetric spacecraft inertia tensor expressed in the body frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InertiaTensor {
    xx: MomentOfInertia,
    yy: MomentOfInertia,
    zz: MomentOfInertia,
    xy: MomentOfInertia,
    xz: MomentOfInertia,
    yz: MomentOfInertia,
}

impl InertiaTensor {
    /// Constructs and validates a symmetric inertia tensor.
    ///
    /// Arguments are the three diagonal and three unique off-diagonal terms.
    /// Positive definiteness is checked with Sylvester's criterion. The
    /// principal moments must also satisfy the rigid-body triangle inequality.
    pub fn new(
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
            xx,
            yy,
            zz,
            xy,
            xz,
            yz,
        })
    }

    /// Constructs a diagonal inertia tensor in principal body axes.
    pub fn principal(
        xx: MomentOfInertia,
        yy: MomentOfInertia,
        zz: MomentOfInertia,
    ) -> Result<Self, InertiaError> {
        let zero = MomentOfInertia::new::<kilogram_square_meter>(0.0);
        Self::new(xx, yy, zz, zero, zero, zero)
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
    /// At least one position component is NaN or infinite.
    #[error("position components must be finite")]
    NonFinitePosition,
    /// At least one velocity component is NaN or infinite.
    #[error("velocity components must be finite")]
    NonFiniteVelocity,
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
    use orskit_units::uom::si::{
        length::kilometer, mass::kilogram, velocity::kilometer_per_second,
    };
    use orskit_units::{Length, Velocity};

    fn translational_state() -> SpacecraftState {
        SpacecraftState::new(
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::GCRF,
            Position::new(
                Length::new::<kilometer>(7_000.0),
                Length::new::<kilometer>(0.0),
                Length::new::<kilometer>(0.0),
            ),
            VelocityVector::new(
                Velocity::new::<kilometer_per_second>(0.0),
                Velocity::new::<kilometer_per_second>(7.5),
                Velocity::new::<kilometer_per_second>(0.0),
            ),
            Mass::new::<kilogram>(1_000.0),
        )
        .expect("fixture is physically valid")
    }

    #[test]
    fn spacecraft_state_keeps_physical_context() {
        let state = translational_state();
        assert_eq!(state.frame(), ReferenceFrame::GCRF);
        assert_eq!(state.epoch(), Epoch::from_tai_seconds(0.0));
        assert_eq!(state.mass(), Mass::new::<kilogram>(1_000.0));
        assert_eq!(state.speed(), Velocity::new::<kilometer_per_second>(7.5));
        assert!(state.orientation().is_none());
        assert!(state.inertia().is_none());
    }

    #[test]
    fn optional_rigid_body_state_is_validated() {
        let inertia = InertiaTensor::principal(
            MomentOfInertia::new::<kilogram_square_meter>(1_000.0),
            MomentOfInertia::new::<kilogram_square_meter>(1_200.0),
            MomentOfInertia::new::<kilogram_square_meter>(800.0),
        )
        .expect("principal moments are positive");
        let state = translational_state()
            .with_orientation(Orientation::identity())
            .with_inertia(inertia);

        assert!(state.orientation().is_some());
        assert_eq!(state.inertia(), Some(&inertia));
    }

    #[test]
    fn invalid_mass_is_rejected() {
        let error = SpacecraftState::new(
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::GCRF,
            Position::from_metres(1.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 1.0, 0.0),
            Mass::new::<kilogram>(0.0),
        )
        .expect_err("zero mass is invalid");
        assert_eq!(error, StateError::NotPositiveMass);
    }

    #[test]
    fn quaternion_is_normalized_and_zero_is_rejected() {
        let orientation = Orientation::from_quaternion(
            Ratio::new::<ratio>(2.0),
            Ratio::new::<ratio>(0.0),
            Ratio::new::<ratio>(0.0),
            Ratio::new::<ratio>(0.0),
        )
        .expect("non-zero quaternion is valid");
        assert_eq!(orientation, Orientation::identity());

        assert_eq!(
            Orientation::from_quaternion(
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
                MomentOfInertia::new::<kilogram_square_meter>(1.0),
                MomentOfInertia::new::<kilogram_square_meter>(1.0),
                MomentOfInertia::new::<kilogram_square_meter>(3.0),
            ),
            Err(InertiaError::ViolatesTriangleInequality)
        );
    }
}
