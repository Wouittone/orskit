use std::hash::{Hash, Hasher};
use std::sync::Arc;

use frames::FrameOrigin;
use units::GravitationalParameter;

use thiserror::Error;

/// Stable structural identity for one sourced gravity context.
///
/// Identifiers are created only by [`GravityContext::new`] from all canonical
/// context fields. Cloning an identifier is cheap and preserves the complete
/// identity without copying provenance strings into every element state.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GravityContextId(Arc<GravityContextIdentity>);

#[derive(Debug, PartialEq)]
struct GravityContextIdentity {
    origin: FrameOrigin,
    gravitational_parameter: GravitationalParameter,
    source: ScientificSource,
}

// `GravitationalParameter` admits only positive finite values, so its equality
// relation is reflexive even though the quantity wrapper cannot derive `Eq`.
impl Eq for GravityContextIdentity {}

impl Hash for GravityContextIdentity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.origin.hash(state);
        self.gravitational_parameter
            .as_cubic_metres_per_second_squared()
            .to_bits()
            .hash(state);
        self.source.hash(state);
    }
}

/// Structured provenance for a scientific value or scenario selection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScientificSource {
    authority: String,
    product: String,
    version_or_scenario: String,
    locator: String,
}

impl ScientificSource {
    /// Constructs a source record with no blank fields.
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

    /// Returns the organization or person responsible for the source.
    #[must_use]
    pub fn authority(&self) -> &str {
        &self.authority
    }

    /// Returns the named product, publication, dataset, or model.
    #[must_use]
    pub fn product(&self) -> &str {
        &self.product
    }

    /// Returns the source version or caller-defined scenario identifier.
    #[must_use]
    pub fn version_or_scenario(&self) -> &str {
        &self.version_or_scenario
    }

    /// Returns the stable source locator supplied by the caller.
    #[must_use]
    pub fn locator(&self) -> &str {
        &self.locator
    }
}

/// Gravity data required to relate Cartesian coordinates and orbital elements.
///
/// A context never infers its origin or parameter from the other. Callers bind
/// the explicitly selected values to a stable identity and provenance record.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GravityContext {
    id: GravityContextId,
}

impl GravityContext {
    /// Binds an origin and positive gravitational parameter to sourced identity.
    #[must_use]
    pub fn new(
        origin: FrameOrigin,
        gravitational_parameter: GravitationalParameter,
        source: ScientificSource,
    ) -> Self {
        Self {
            id: GravityContextId(Arc::new(GravityContextIdentity {
                origin,
                gravitational_parameter,
                source,
            })),
        }
    }

    /// Returns this context's stable identity.
    #[must_use]
    pub const fn id(&self) -> &GravityContextId {
        &self.id
    }

    /// Returns the origin about which the osculating elements are defined.
    #[must_use]
    pub fn origin(&self) -> FrameOrigin {
        self.id.0.origin
    }

    /// Returns the positive gravitational parameter selected by the caller.
    #[must_use]
    pub fn gravitational_parameter(&self) -> GravitationalParameter {
        self.id.0.gravitational_parameter
    }

    /// Returns the provenance of the selected parameter or scenario.
    #[must_use]
    pub fn source(&self) -> &ScientificSource {
        &self.id.0.source
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
    use frames::{Body, FrameOrigin};

    fn source() -> ScientificSource {
        ScientificSource::new(
            "International Astronomical Union",
            "Test gravity selection",
            "scenario-2026-07",
            "urn:orskit:test:gravity",
        )
        .expect("complete source")
    }

    #[test]
    fn gravity_context_retains_stable_identity_origin_parameter_and_source() {
        let parameter = GravitationalParameter::from_cubic_metres_per_second_squared(42.0)
            .expect("positive parameter");
        let context = GravityContext::new(FrameOrigin::Body(Body::EARTH), parameter, source());

        assert_eq!(context.origin(), FrameOrigin::Body(Body::EARTH));
        assert_eq!(context.gravitational_parameter(), parameter);
        assert_eq!(context.source().version_or_scenario(), "scenario-2026-07");
    }

    #[test]
    fn context_identity_is_derived_from_every_canonical_field() {
        let parameter = GravitationalParameter::from_cubic_metres_per_second_squared(42.0)
            .expect("positive parameter");
        let same = || GravityContext::new(FrameOrigin::Body(Body::EARTH), parameter, source());
        let baseline = same();
        assert_eq!(baseline.id(), same().id());

        let changed_parameter = GravityContext::new(
            FrameOrigin::Body(Body::EARTH),
            GravitationalParameter::from_cubic_metres_per_second_squared(43.0)
                .expect("positive parameter"),
            source(),
        );
        assert_ne!(baseline.id(), changed_parameter.id());

        let changed_source = GravityContext::new(
            FrameOrigin::Body(Body::EARTH),
            parameter,
            ScientificSource::new(
                "International Astronomical Union",
                "Test gravity selection",
                "different-scenario",
                "urn:orskit:test:gravity",
            )
            .expect("complete source"),
        );
        assert_ne!(baseline.id(), changed_source.id());

        let changed_origin =
            GravityContext::new(FrameOrigin::Body(Body::MARS), parameter, source());
        assert_ne!(baseline.id(), changed_origin.id());
    }

    #[test]
    fn scientific_source_rejects_every_blank_identity_field() {
        assert_eq!(
            ScientificSource::new(" ", "product", "version", "locator"),
            Err(ScientificSourceError::BlankAuthority)
        );
        assert_eq!(
            ScientificSource::new("authority", "", "version", "locator"),
            Err(ScientificSourceError::BlankProduct)
        );
        assert_eq!(
            ScientificSource::new("authority", "product", "\t", "locator"),
            Err(ScientificSourceError::BlankVersionOrScenario)
        );
        assert_eq!(
            ScientificSource::new("authority", "product", "version", ""),
            Err(ScientificSourceError::BlankLocator)
        );
    }
}
