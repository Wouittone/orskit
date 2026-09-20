#![forbid(unsafe_code)]

//! Explicit atmosphere-density provider contracts.
//!
//! This crate owns no atmosphere model or data. A provider receives an epoch,
//! body, position, and expressing frame for every query, and identifies the
//! selected model and data revision. No provider may infer a frame, epoch
//! scale, body, or data set from ambient process state.

use std::sync::Arc;

use frames::{Body, ReferenceFrame};
use hifitime::Epoch;
use thiserror::Error;
use units::uom::si::mass_density::kilogram_per_cubic_meter;
use units::{MassDensity, Position};

/// The context identifying the atmosphere model and data revision in use.
///
/// The provider owns this immutable context. `revision` may identify a
/// versioned data product, configuration, or application-controlled bundle;
/// it must not be left blank when the provider is constructed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtmosphereDataContext {
    model: String,
    revision: String,
}

impl AtmosphereDataContext {
    /// Creates an explicit model and data revision identity.
    pub fn new(
        model: impl Into<String>,
        revision: impl Into<String>,
    ) -> Result<Self, AtmosphereDataContextError> {
        let model = model.into();
        let revision = revision.into();
        if model.trim().is_empty() {
            return Err(AtmosphereDataContextError::BlankModel);
        }
        if revision.trim().is_empty() {
            return Err(AtmosphereDataContextError::BlankRevision);
        }
        Ok(Self { model, revision })
    }

    /// Returns the selected atmosphere model identity.
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Returns the selected model/data revision.
    #[must_use]
    pub fn revision(&self) -> &str {
        &self.revision
    }
}

/// Invalid atmosphere data-context identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum AtmosphereDataContextError {
    /// The model identity was blank.
    #[error("atmosphere model identity must not be blank")]
    BlankModel,
    /// The model/data revision was blank.
    #[error("atmosphere data revision must not be blank")]
    BlankRevision,
}

/// One explicit atmosphere-density query.
///
/// `position` is interpreted in `frame`, at `epoch`, relative to `body`.
/// The epoch is passed through as Hifitime's [`Epoch`], so its time scale
/// remains visible to the caller and provider. A provider must reject frames,
/// bodies, epochs, or coverage it cannot evaluate rather than relabeling the
/// position or selecting an implicit data set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtmosphereDensityInput {
    epoch: Epoch,
    body: Body,
    position: Position,
    frame: ReferenceFrame,
}

impl AtmosphereDensityInput {
    /// Creates a finite, frame-qualified density query.
    pub fn new(
        epoch: Epoch,
        body: Body,
        position: Position,
        frame: ReferenceFrame,
    ) -> Result<Self, AtmosphereDensityInputError> {
        if !position.is_finite() {
            return Err(AtmosphereDensityInputError::NonFinitePosition);
        }
        Ok(Self {
            epoch,
            body,
            position,
            frame,
        })
    }

    /// Returns the requested evaluation epoch.
    #[must_use]
    pub const fn epoch(self) -> Epoch {
        self.epoch
    }

    /// Returns the atmospheric body.
    #[must_use]
    pub const fn body(self) -> Body {
        self.body
    }

    /// Returns the position at which density is requested.
    #[must_use]
    pub const fn position(self) -> Position {
        self.position
    }

    /// Returns the frame expressing [`Self::position`].
    #[must_use]
    pub const fn frame(self) -> ReferenceFrame {
        self.frame
    }
}

/// Invalid atmosphere-density query input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum AtmosphereDensityInputError {
    /// At least one position component was NaN or infinite.
    #[error("atmosphere-density position components must be finite")]
    NonFinitePosition,
}

/// A non-negative, finite atmospheric mass density in SI units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtmosphereDensity(MassDensity);

impl AtmosphereDensity {
    /// Creates a validated density in kilograms per cubic metre.
    pub fn new(value: MassDensity) -> Result<Self, AtmosphereDensityError> {
        let kilograms_per_cubic_meter = value.get::<kilogram_per_cubic_meter>();
        if !kilograms_per_cubic_meter.is_finite() {
            return Err(AtmosphereDensityError::NonFinite);
        }
        if kilograms_per_cubic_meter < 0.0 {
            return Err(AtmosphereDensityError::Negative);
        }
        Ok(Self(value))
    }

    /// Returns the typed mass density.
    #[must_use]
    pub const fn value(self) -> MassDensity {
        self.0
    }
}

/// Invalid atmosphere-density result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum AtmosphereDensityError {
    /// The density was NaN or infinite.
    #[error("atmosphere density must be finite")]
    NonFinite,
    /// Atmospheric mass density cannot be negative.
    #[error("atmosphere density must not be negative")]
    Negative,
}

/// Supplies atmospheric density for explicit epoch/frame/body queries.
///
/// Implementations own their model, data loading, coverage policy, and
/// evaluation errors. The context is exposed so callers can retain the exact
/// model/data selection alongside a propagation scenario.
pub trait AtmosphereDensityProvider: std::fmt::Debug + Send + Sync {
    /// Provider-specific evaluation failure.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Returns the immutable model and data identity used by this provider.
    fn data_context(&self) -> &AtmosphereDataContext;

    /// Evaluates density for one explicit query.
    fn density(&self, input: AtmosphereDensityInput) -> Result<AtmosphereDensity, Self::Error>;
}

/// Shared application-defined atmosphere-density provider.
///
/// The provider's concrete error remains part of the type so object-safe
/// callers can preserve its typed failure contract.
pub type SharedAtmosphereDensityProvider<E> = Arc<dyn AtmosphereDensityProvider<Error = E>>;

#[cfg(test)]
mod tests {
    use super::*;
    use frames::Body;
    use units::uom::si::length::meter;
    use units::uom::si::mass_density::kilogram_per_cubic_meter;

    #[test]
    fn query_retains_epoch_body_position_and_frame() {
        let epoch = Epoch::from_tai_seconds(42.0);
        let position = Position::from_metres(1.0, 2.0, 3.0);
        let input =
            AtmosphereDensityInput::new(epoch, Body::EARTH, position, ReferenceFrame::ITRF2020)
                .expect("finite query");

        assert_eq!(input.epoch(), epoch);
        assert_eq!(input.body(), Body::EARTH);
        assert_eq!(input.position(), position);
        assert_eq!(input.frame(), ReferenceFrame::ITRF2020);
    }

    #[test]
    fn rejects_invalid_context_query_and_density() {
        assert_eq!(
            AtmosphereDataContext::new("", "v1"),
            Err(AtmosphereDataContextError::BlankModel)
        );
        assert_eq!(
            AtmosphereDataContext::new("model", ""),
            Err(AtmosphereDataContextError::BlankRevision)
        );
        assert_eq!(
            AtmosphereDensityInput::new(
                Epoch::from_tai_seconds(0.0),
                Body::EARTH,
                Position::new(
                    units::Length::new::<meter>(f64::NAN),
                    units::Length::new::<meter>(0.0),
                    units::Length::new::<meter>(0.0),
                ),
                ReferenceFrame::ITRF2020,
            ),
            Err(AtmosphereDensityInputError::NonFinitePosition)
        );
        assert_eq!(
            AtmosphereDensity::new(MassDensity::new::<kilogram_per_cubic_meter>(-1.0)),
            Err(AtmosphereDensityError::Negative)
        );
    }

    #[test]
    fn accepts_zero_density() {
        assert_eq!(
            AtmosphereDensity::new(MassDensity::new::<kilogram_per_cubic_meter>(0.0))
                .expect("zero density is physical")
                .value()
                .get::<kilogram_per_cubic_meter>(),
            0.0
        );
    }
}
