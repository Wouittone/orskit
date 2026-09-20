#![forbid(unsafe_code)]

//! Gravity-provider contracts and opt-in gravity implementations.
//!
//! The provider is selected explicitly by callers. The built-in point-mass
//! provider is available only with the `point-mass` feature.

use std::{fmt, sync::Arc};

use frames::FrameOrigin;
use units::GravitationalParameter;

/// Supplies the central gravity selection used to interpret an orbit.
///
/// Providers are immutable while shared with a state or dynamics model. A
/// provider identifies both the physical origin and the typed gravitational
/// parameter; callers own catalog, scenario, and data-version management.
pub trait CentralGravityProvider: fmt::Debug + Send + Sync {
    /// Origin about which the orbit is defined.
    fn origin(&self) -> FrameOrigin;

    /// Standard gravitational parameter selected by this provider.
    fn parameter(&self) -> GravitationalParameter;
}

/// Shared, application-defined central-gravity provider.
pub type SharedCentralGravity = Arc<dyn CentralGravityProvider>;

#[cfg(feature = "point-mass")]
mod point_mass {
    use super::*;

    /// Immutable central-gravity provider using one point-mass parameter.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct PointMass {
        origin: FrameOrigin,
        parameter: GravitationalParameter,
    }

    impl PointMass {
        /// Selects a point-mass parameter for one explicit origin.
        #[must_use]
        pub const fn new(origin: FrameOrigin, parameter: GravitationalParameter) -> Self {
            Self { origin, parameter }
        }
    }

    impl CentralGravityProvider for PointMass {
        fn origin(&self) -> FrameOrigin {
            self.origin
        }

        fn parameter(&self) -> GravitationalParameter {
            self.parameter
        }
    }
}

#[cfg(feature = "point-mass")]
pub use point_mass::PointMass;

#[cfg(feature = "zonal")]
mod zonal {
    use super::*;
    use thiserror::Error;
    use units::uom::si::{length::meter, ratio::ratio};
    use units::{Length, Ratio};

    /// Extends a central-gravity selection with axisymmetric (zonal)
    /// oblateness data.
    ///
    /// This first slice supports only the dominant `J2` zonal term; general
    /// degree/order spherical harmonics, tesseral/sectorial terms, and
    /// body-fixed rotation are separate future work. Implementations are
    /// responsible for stating the tide-system convention (tide-free,
    /// zero-tide, or mean-tide) their `j2` value follows, per IERS
    /// Conventions (2010), Chapter 6; this trait does not encode a
    /// convention itself.
    ///
    /// Evaluating this data assumes the consuming frame's declared inertial
    /// axes coincide with the body's oblateness (mean rotation) axis. This is
    /// true for GCRF and Earth's mean rotation axis at the accuracy this term
    /// claims; it is not a general body-fixed-rotation capability.
    pub trait ZonalGravityProvider: CentralGravityProvider {
        /// Equatorial radius used to scale the zonal expansion.
        fn equatorial_radius(&self) -> Length;

        /// Dimensionless unnormalized zonal `J2` coefficient.
        fn j2(&self) -> Ratio;
    }

    /// Immutable point-mass-plus-`J2` zonal gravity data for one body.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct J2GravityField {
        origin: FrameOrigin,
        parameter: GravitationalParameter,
        equatorial_radius: Length,
        j2: Ratio,
    }

    impl J2GravityField {
        /// Selects one body's point-mass parameter, equatorial radius, and
        /// zonal `J2` coefficient.
        pub fn new(
            origin: FrameOrigin,
            parameter: GravitationalParameter,
            equatorial_radius: Length,
            j2: Ratio,
        ) -> Result<Self, J2GravityFieldError> {
            let radius_metres = equatorial_radius.get::<meter>();
            if !radius_metres.is_finite() {
                return Err(J2GravityFieldError::NonFiniteEquatorialRadius);
            }
            if radius_metres <= 0.0 {
                return Err(J2GravityFieldError::NonPositiveEquatorialRadius);
            }
            if !j2.get::<ratio>().is_finite() {
                return Err(J2GravityFieldError::NonFiniteJ2);
            }
            Ok(Self {
                origin,
                parameter,
                equatorial_radius,
                j2,
            })
        }
    }

    impl CentralGravityProvider for J2GravityField {
        fn origin(&self) -> FrameOrigin {
            self.origin
        }

        fn parameter(&self) -> GravitationalParameter {
            self.parameter
        }
    }

    impl ZonalGravityProvider for J2GravityField {
        fn equatorial_radius(&self) -> Length {
            self.equatorial_radius
        }

        fn j2(&self) -> Ratio {
            self.j2
        }
    }

    /// Invalid `J2GravityField` construction input.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
    pub enum J2GravityFieldError {
        /// The supplied equatorial radius was NaN or infinite.
        #[error("equatorial radius must be finite")]
        NonFiniteEquatorialRadius,
        /// The supplied equatorial radius was not strictly positive.
        #[error("equatorial radius must be strictly positive")]
        NonPositiveEquatorialRadius,
        /// The supplied J2 coefficient was NaN or infinite.
        #[error("J2 coefficient must be finite")]
        NonFiniteJ2,
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use frames::Body;
        use units::uom::si::length::kilometer;

        fn earth_parameter() -> GravitationalParameter {
            GravitationalParameter::try_from(3.986_004_418e14).expect("positive parameter")
        }

        #[test]
        fn rejects_non_physical_equatorial_radius_and_j2() {
            assert_eq!(
                J2GravityField::new(
                    FrameOrigin::Body(Body::EARTH),
                    earth_parameter(),
                    Length::new::<kilometer>(0.0),
                    Ratio::new::<ratio>(1.082_63e-3),
                ),
                Err(J2GravityFieldError::NonPositiveEquatorialRadius)
            );
            assert_eq!(
                J2GravityField::new(
                    FrameOrigin::Body(Body::EARTH),
                    earth_parameter(),
                    Length::new::<kilometer>(6378.137),
                    Ratio::new::<ratio>(f64::NAN),
                ),
                Err(J2GravityFieldError::NonFiniteJ2)
            );
        }

        #[test]
        fn retains_the_explicit_selection() {
            let field = J2GravityField::new(
                FrameOrigin::Body(Body::EARTH),
                earth_parameter(),
                Length::new::<kilometer>(6378.137),
                Ratio::new::<ratio>(1.082_63e-3),
            )
            .expect("valid J2 field");
            assert_eq!(field.origin(), FrameOrigin::Body(Body::EARTH));
            assert_eq!(field.equatorial_radius().get::<kilometer>(), 6378.137);
            assert_eq!(field.j2().get::<ratio>(), 1.082_63e-3);
        }
    }
}

#[cfg(feature = "zonal")]
pub use zonal::{J2GravityField, J2GravityFieldError, ZonalGravityProvider};

#[cfg(all(test, feature = "point-mass"))]
mod tests {
    use super::*;
    use frames::Body;

    #[test]
    fn point_mass_retains_the_explicit_selection() {
        let provider = PointMass::new(
            FrameOrigin::Body(Body::EARTH),
            GravitationalParameter::try_from(42.0).expect("positive parameter"),
        );
        assert_eq!(provider.origin(), FrameOrigin::Body(Body::EARTH));
        assert_eq!(
            provider.parameter().as_cubic_metres_per_second_squared(),
            42.0
        );
    }
}
