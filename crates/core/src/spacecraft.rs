use nalgebra::{Matrix3, Quaternion, UnitQuaternion};
use orskit_frames::ReferenceFrame;
use orskit_units::uom::si::{
    mass::kilogram, moment_of_inertia::kilogram_square_meter, ratio::ratio,
};
use orskit_units::{Mass, MomentOfInertia, Ratio};
use thiserror::Error;

use crate::StateError;

/// Representation-independent physical properties of a spacecraft.
///
/// These values are required by every complete [`crate::State`]. Their frame
/// identities remain explicit and are not inferred from the orbit
/// representation.
#[derive(Debug, Clone, PartialEq)]
pub struct SpacecraftProperties {
    mass: Mass,
    orientation: Orientation,
    inertia: InertiaTensor,
}

impl SpacecraftProperties {
    /// Constructs the physical properties shared by every state representation.
    pub fn new(
        mass: Mass,
        orientation: Orientation,
        inertia: InertiaTensor,
    ) -> Result<Self, StateError> {
        let mass_kg = mass.get::<kilogram>();
        if !mass_kg.is_finite() {
            return Err(StateError::NonFiniteMass);
        }
        if mass_kg <= 0.0 {
            return Err(StateError::NotPositiveMass);
        }

        Ok(Self {
            mass,
            orientation,
            inertia,
        })
    }

    /// Returns the spacecraft mass.
    #[must_use]
    pub const fn mass(&self) -> Mass {
        self.mass
    }

    /// Returns the explicit spacecraft orientation.
    #[must_use]
    pub const fn orientation(&self) -> &Orientation {
        &self.orientation
    }

    /// Returns the explicit framed inertia tensor.
    #[must_use]
    pub const fn inertia(&self) -> InertiaTensor {
        self.inertia
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
    use orskit_units::uom::si::mass::kilogram;

    fn body_frame() -> ReferenceFrame {
        let id = CustomFrameId::new(1);
        ReferenceFrame::new(FrameOrigin::Custom(id), FrameOrientation::Custom(id))
    }

    fn physical_parts() -> (Orientation, InertiaTensor) {
        let body = body_frame();
        let inertia = InertiaTensor::principal(
            body,
            MomentOfInertia::new::<kilogram_square_meter>(1_000.0),
            MomentOfInertia::new::<kilogram_square_meter>(1_200.0),
            MomentOfInertia::new::<kilogram_square_meter>(800.0),
        )
        .expect("principal moments are physical");
        let orientation = Orientation::identity(body, ReferenceFrame::ITRF2020);
        (orientation, inertia)
    }

    #[test]
    fn spacecraft_properties_require_positive_mass_and_explicit_rigid_body_data() {
        let (orientation, inertia) = physical_parts();
        let properties =
            SpacecraftProperties::new(Mass::new::<kilogram>(1_000.0), orientation.clone(), inertia)
                .expect("fixture properties are physical");
        assert_eq!(properties.mass(), Mass::new::<kilogram>(1_000.0));
        assert_eq!(properties.orientation(), &orientation);
        assert_eq!(properties.inertia(), inertia);

        let error = SpacecraftProperties::new(Mass::new::<kilogram>(0.0), orientation, inertia)
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
