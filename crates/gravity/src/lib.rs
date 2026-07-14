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

#[cfg(all(test, feature = "point-mass"))]
mod tests {
    use super::*;
    use frames::Body;

    #[test]
    fn point_mass_retains_the_explicit_selection() {
        let provider = PointMass::new(
            FrameOrigin::Body(Body::EARTH),
            GravitationalParameter::from_cubic_metres_per_second_squared(42.0)
                .expect("positive parameter"),
        );
        assert_eq!(provider.origin(), FrameOrigin::Body(Body::EARTH));
        assert_eq!(
            provider.parameter().as_cubic_metres_per_second_squared(),
            42.0
        );
    }
}
