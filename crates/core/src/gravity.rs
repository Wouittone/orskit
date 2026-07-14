use std::{fmt, sync::Arc};

use frames::FrameOrigin;
use units::GravitationalParameter;

/// Structured provenance for a scientific value, model, or scenario.
///
/// The trait is object-safe so applications and implementation crates can
/// attach their own immutable catalog, database, or configuration-backed
/// provenance without a core-owned record shape.
pub trait ScientificSource: fmt::Debug + Send + Sync {
    fn authority(&self) -> &str;
    fn product(&self) -> &str;
    fn version_or_scenario(&self) -> &str;
    fn locator(&self) -> &str;
}

/// Shared, application-extensible scientific provenance.
pub type SharedScientificSource = Arc<dyn ScientificSource>;

/// Central gravity selected for orbital-element interpretation and conversion.
///
/// Implementations must remain immutable while shared with an orbital state.
pub trait CentralGravity: fmt::Debug + Send + Sync {
    fn origin(&self) -> FrameOrigin;
    fn gravitational_parameter(&self) -> GravitationalParameter;
    fn source(&self) -> &dyn ScientificSource;
}

/// Shared central-gravity object whose allocation is its scientific identity.
pub type SharedCentralGravity = Arc<dyn CentralGravity>;

#[cfg(test)]
mod tests {
    use super::*;
    use frames::Body;

    #[derive(Debug)]
    struct ApplicationSource;
    impl ScientificSource for ApplicationSource {
        fn authority(&self) -> &str {
            "mission navigation team"
        }
        fn product(&self) -> &str {
            "frozen gravity selection"
        }
        fn version_or_scenario(&self) -> &str {
            "scenario-42"
        }
        fn locator(&self) -> &str {
            "mission-db://gravity/42"
        }
    }
    #[derive(Debug)]
    struct ApplicationGravity {
        source: ApplicationSource,
    }
    impl CentralGravity for ApplicationGravity {
        fn origin(&self) -> FrameOrigin {
            FrameOrigin::Body(Body::EARTH)
        }
        fn gravitational_parameter(&self) -> GravitationalParameter {
            GravitationalParameter::from_cubic_metres_per_second_squared(42.0).expect("positive")
        }
        fn source(&self) -> &dyn ScientificSource {
            &self.source
        }
    }

    #[test]
    fn application_types_are_accepted_as_object_safe_extensions() {
        let gravity: SharedCentralGravity = Arc::new(ApplicationGravity {
            source: ApplicationSource,
        });
        assert_eq!(gravity.origin(), FrameOrigin::Body(Body::EARTH));
        assert_eq!(gravity.source().authority(), "mission navigation team");
    }
}
