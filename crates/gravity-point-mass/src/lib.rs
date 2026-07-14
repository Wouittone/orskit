#![forbid(unsafe_code)]

//! Small immutable point-mass gravity and provenance implementations.
//!
//! `orskit-core` defines the open scientific-source and central-gravity
//! contracts. This crate provides a convenience implementation without
//! requiring applications to adopt it.

use core_crate::{CentralGravity, ScientificSource, SharedScientificSource};
use frames::FrameOrigin;
use thiserror::Error;
use units::GravitationalParameter;

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
        Ok(Self {
            authority: normalized_field(authority, ScientificSourceError::BlankAuthority)?,
            product: normalized_field(product, ScientificSourceError::BlankProduct)?,
            version_or_scenario: normalized_field(
                version_or_scenario,
                ScientificSourceError::BlankVersionOrScenario,
            )?,
            locator: normalized_field(locator, ScientificSourceError::BlankLocator)?,
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

/// Immutable central-gravity selection using a point-mass parameter.
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
    #[error("scientific-source authority must not be blank")]
    BlankAuthority,
    #[error("scientific-source product must not be blank")]
    BlankProduct,
    #[error("scientific-source version or scenario must not be blank")]
    BlankVersionOrScenario,
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
    use std::sync::Arc;

    #[test]
    fn retains_explicit_gravity_selection_and_provenance() {
        let source: SharedScientificSource = Arc::new(
            ReferenceSource::new("IAU", "test", "scenario", "urn:test").expect("complete source"),
        );
        let gravity = PointMassGravity::new(
            FrameOrigin::Body(Body::EARTH),
            GravitationalParameter::from_cubic_metres_per_second_squared(42.0).expect("positive"),
            source.clone(),
        )
        .expect("valid gravity");
        assert_eq!(gravity.origin(), FrameOrigin::Body(Body::EARTH));
        assert!(Arc::ptr_eq(gravity.source_handle(), &source));
    }
}
