use std::fmt;
use std::sync::Arc;

use frames::FrameOrigin;
use thiserror::Error;
use units::GravitationalParameter;

/// Structured provenance for a scientific value, model, or scenario.
///
/// Implementations must return non-blank values for every field. The trait is
/// object-safe so applications can attach their own catalog, database, or
/// configuration-backed provenance without converting it into a core-owned
/// representation.
pub trait ScientificSource: fmt::Debug + Send + Sync {
    /// Organization or person responsible for the source.
    fn authority(&self) -> &str;

    /// Named product, publication, dataset, or model.
    fn product(&self) -> &str;

    /// Source version or caller-defined scenario identifier.
    fn version_or_scenario(&self) -> &str;

    /// Stable source locator, such as a DOI, URI, document section, or key.
    fn locator(&self) -> &str;
}

/// Shared, application-extensible scientific provenance.
pub type SharedScientificSource = Arc<dyn ScientificSource>;

/// Central gravity selected for orbital-element interpretation and conversion.
///
/// The object binds an explicit frame origin and typed positive gravitational
/// parameter to caller-defined provenance. Implementations must be immutable
/// with respect to these methods while shared with an orbital state.
pub trait CentralGravity: fmt::Debug + Send + Sync {
    /// Origin about which osculating orbital elements are defined.
    fn origin(&self) -> FrameOrigin;

    /// Positive finite standard gravitational parameter.
    fn gravitational_parameter(&self) -> GravitationalParameter;

    /// Provenance for the selected gravity value or scenario.
    fn source(&self) -> &dyn ScientificSource;
}

/// Shared central-gravity object whose allocation is its unforgeable identity.
///
/// Clone this `Arc` when multiple states or models must refer to the same
/// selection. Independently allocating an equivalent implementation creates a
/// deliberately different identity.
pub type SharedCentralGravity = Arc<dyn CentralGravity>;

/// Small built-in provenance record for callers that do not need a custom type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReferenceSource {
    authority: String,
    product: String,
    version_or_scenario: String,
    locator: String,
}

impl ReferenceSource {
    /// Constructs a reference record with no blank fields.
    pub fn new(
        authority: impl Into<String>,
        product: impl Into<String>,
        version_or_scenario: impl Into<String>,
        locator: impl Into<String>,
    ) -> Result<Self, ScientificSourceError> {
        let authority = normalized_field(authority, ScientificSourceError::BlankAuthority)?;
        let product = normalized_field(product, ScientificSourceError::BlankProduct)?;
        let version_or_scenario = normalized_field(
            version_or_scenario,
            ScientificSourceError::BlankVersionOrScenario,
        )?;
        let locator = normalized_field(locator, ScientificSourceError::BlankLocator)?;
        Ok(Self {
            authority,
            product,
            version_or_scenario,
            locator,
        })
    }
}

impl ScientificSource for ReferenceSource {
    fn authority(&self) -> &str {
        &self.authority
    }

    fn product(&self) -> &str {
        &self.product
    }

    fn version_or_scenario(&self) -> &str {
        &self.version_or_scenario
    }

    fn locator(&self) -> &str {
        &self.locator
    }
}

/// Small built-in immutable central-gravity selection.
///
/// Applications with richer gravity catalogs can implement [`CentralGravity`]
/// directly and use the same conversion APIs.
#[derive(Debug, Clone)]
pub struct PointMassGravity {
    origin: FrameOrigin,
    gravitational_parameter: GravitationalParameter,
    source: SharedScientificSource,
}

impl PointMassGravity {
    /// Binds an explicit origin and typed parameter to complete provenance.
    pub fn new(
        origin: FrameOrigin,
        gravitational_parameter: GravitationalParameter,
        source: SharedScientificSource,
    ) -> Result<Self, ScientificSourceError> {
        validate_source(source.as_ref())?;
        Ok(Self {
            origin,
            gravitational_parameter,
            source,
        })
    }

    /// Returns the shared provenance object without cloning its `Arc`.
    #[must_use]
    pub const fn source_handle(&self) -> &SharedScientificSource {
        &self.source
    }
}

impl CentralGravity for PointMassGravity {
    fn origin(&self) -> FrameOrigin {
        self.origin
    }

    fn gravitational_parameter(&self) -> GravitationalParameter {
        self.gravitational_parameter
    }

    fn source(&self) -> &dyn ScientificSource {
        self.source.as_ref()
    }
}

/// Invalid structured scientific-source input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ScientificSourceError {
    /// Authority contains no non-whitespace characters.
    #[error("scientific-source authority must not be blank")]
    BlankAuthority,
    /// Product contains no non-whitespace characters.
    #[error("scientific-source product must not be blank")]
    BlankProduct,
    /// Version or scenario contains no non-whitespace characters.
    #[error("scientific-source version or scenario must not be blank")]
    BlankVersionOrScenario,
    /// Locator contains no non-whitespace characters.
    #[error("scientific-source locator must not be blank")]
    BlankLocator,
}

fn validate_source(source: &dyn ScientificSource) -> Result<(), ScientificSourceError> {
    validate_borrowed_field(source.authority(), ScientificSourceError::BlankAuthority)?;
    validate_borrowed_field(source.product(), ScientificSourceError::BlankProduct)?;
    validate_borrowed_field(
        source.version_or_scenario(),
        ScientificSourceError::BlankVersionOrScenario,
    )?;
    validate_borrowed_field(source.locator(), ScientificSourceError::BlankLocator)
}

fn validate_borrowed_field(
    value: &str,
    error: ScientificSourceError,
) -> Result<(), ScientificSourceError> {
    if value.trim().is_empty() {
        Err(error)
    } else {
        Ok(())
    }
}

fn normalized_field(
    value: impl Into<String>,
    error: ScientificSourceError,
) -> Result<String, ScientificSourceError> {
    let value = value.into();
    let value = value.trim();
    if value.is_empty() {
        Err(error)
    } else {
        Ok(value.to_owned())
    }
}

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
            GravitationalParameter::from_cubic_metres_per_second_squared(42.0)
                .expect("positive parameter")
        }

        fn source(&self) -> &dyn ScientificSource {
            &self.source
        }
    }

    fn reference_source() -> ReferenceSource {
        ReferenceSource::new(
            "International Astronomical Union",
            "Test gravity selection",
            "scenario-2026-07",
            "urn:orskit:test:gravity",
        )
        .expect("complete source")
    }

    #[test]
    fn application_types_are_accepted_as_object_safe_extensions() {
        let gravity: SharedCentralGravity = Arc::new(ApplicationGravity {
            source: ApplicationSource,
        });

        assert_eq!(gravity.origin(), FrameOrigin::Body(Body::EARTH));
        assert_eq!(gravity.source().authority(), "mission navigation team");
    }

    #[test]
    fn built_in_point_mass_retains_origin_parameter_and_shared_source() {
        let parameter = GravitationalParameter::from_cubic_metres_per_second_squared(42.0)
            .expect("positive parameter");
        let source: SharedScientificSource = Arc::new(reference_source());
        let gravity =
            PointMassGravity::new(FrameOrigin::Body(Body::EARTH), parameter, source.clone())
                .expect("complete source");

        assert_eq!(gravity.origin(), FrameOrigin::Body(Body::EARTH));
        assert_eq!(gravity.gravitational_parameter(), parameter);
        assert!(Arc::ptr_eq(gravity.source_handle(), &source));
        assert_eq!(gravity.source().version_or_scenario(), "scenario-2026-07");
    }

    #[test]
    fn independently_allocated_gravity_objects_have_distinct_identity() {
        let make = || -> SharedCentralGravity {
            Arc::new(
                PointMassGravity::new(
                    FrameOrigin::Body(Body::EARTH),
                    GravitationalParameter::from_cubic_metres_per_second_squared(42.0)
                        .expect("positive parameter"),
                    Arc::new(reference_source()),
                )
                .expect("complete source"),
            )
        };
        let gravity = make();

        assert!(Arc::ptr_eq(&gravity, &gravity.clone()));
        assert!(!Arc::ptr_eq(&gravity, &make()));
    }

    #[test]
    fn built_in_source_rejects_every_blank_field() {
        assert_eq!(
            ReferenceSource::new(" ", "product", "version", "locator"),
            Err(ScientificSourceError::BlankAuthority)
        );
        assert_eq!(
            ReferenceSource::new("authority", "", "version", "locator"),
            Err(ScientificSourceError::BlankProduct)
        );
        assert_eq!(
            ReferenceSource::new("authority", "product", "\t", "locator"),
            Err(ScientificSourceError::BlankVersionOrScenario)
        );
        assert_eq!(
            ReferenceSource::new("authority", "product", "version", ""),
            Err(ScientificSourceError::BlankLocator)
        );
    }
}
