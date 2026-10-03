#![forbid(unsafe_code)]

//! Caller-selected, fully normalized spherical-harmonic gravity.
//!
//! [`SphericalHarmonicGravityModel`] evaluates the static geopotential through
//! any degree/order truncation supported by a caller-owned
//! [`HarmonicCoefficientProvider`]. Gravity coefficients, their source and
//! revision, normalization, tide system, coefficient frame, and epoch
//! validity are explicit. The caller also selects a sourced
//! [`BodyFixedTransformProvider`]; each evaluation requests the configured
//! inertial-to-body-fixed rotation at its exact Hifitime epoch and time scale,
//! then rotates the SI acceleration back to the input state frame.
//! [`TimeVaryingSphericalHarmonicGravityModel`] composes explicit epoch-dependent
//! normalized coefficient deltas onto an existing static model.
//!
//! Coefficients use the geodetic fully normalized 4π convention with
//! `P̄₀₀ = 1`. The potential is
//!
//! ```text
//! V = μ/r Σₙ (R/r)ⁿ Σₘ P̄ₙₘ(sin φ)
//!       [C̄ₙₘ cos(m λ) + S̄ₙₘ sin(m λ)]
//! ```
//!
//! and acceleration is the Cartesian gradient of `V`. A Cartesian
//! differentiated solid-harmonic recurrence avoids longitude singularities at
//! the poles. The model applies no tide-system conversion or tide correction:
//! the selected coefficient set's tide convention is retained as metadata.
//! Do not combine permanent-tide corrections with coefficients whose selected
//! tide system already includes them.
//!
//! No coefficient data is bundled, fetched, parsed, or selected implicitly.
//! The crate supplies a generic linear secular-rate delta provider, but does
//! not supply a gravity product, tide model, or frame realization; the
//! caller-provided transform must be suitable for the
//! requested epoch, frames, and accuracy. Evaluation assumes the selected
//! finite expansion is valid at the requested position; the crate does not
//! infer a body's surface or enforce an external-field radius.
//!
//! The expansion and normalization follow IERS Conventions (2010), Chapter 6,
//! §6.1. See `.agent/decisions/0047-low-degree-spherical-harmonics-boundary.md`
//! and `docs/guides/spherical-harmonics.md` for the independent validation
//! policy and limitations.
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use dynamics_spherical_harmonics::{
//!     CoefficientEpochSemantics, CoefficientNormalization, HarmonicCoefficient,
//!     HarmonicCoefficientMetadata, HarmonicCoefficientProvider,
//!     SourcedBodyFixedTransformProvider, SphericalHarmonicField,
//!     SphericalHarmonicGravityModel, TideSystem,
//! };
//! use frames::{
//!     BodyFixedTransform, BodyFixedTransformProvider, BodyFixedTransformRequest,
//!     DirectionCosineMatrix, ReferenceDataDescriptor, ReferenceFrame,
//! };
//! use hifitime::{Epoch, TimeScale};
//! use units::uom::si::length::meter;
//! use units::{AngularVelocityVector, GravitationalParameter, Length};
//!
//! #[derive(Debug)]
//! struct Coefficients(HarmonicCoefficientMetadata);
//!
//! impl HarmonicCoefficientProvider for Coefficients {
//!     fn metadata(&self) -> &HarmonicCoefficientMetadata {
//!         &self.0
//!     }
//!
//!     fn coefficient(&self, degree: u32, order: u32) -> Option<HarmonicCoefficient> {
//!         (degree <= 2 && order <= degree).then_some(HarmonicCoefficient {
//!             cosine: if degree == 0 && order == 0 { 1.0 } else { 0.0 },
//!             sine: 0.0,
//!         })
//!     }
//! }
//!
//! #[derive(Debug)]
//! struct CallerTransform;
//!
//! impl BodyFixedTransformProvider for CallerTransform {
//!     fn body_fixed_transform(
//!         &self,
//!         request: BodyFixedTransformRequest,
//!     ) -> Result<BodyFixedTransform, frames::BodyFixedTransformProviderError> {
//!         BodyFixedTransform::new(
//!             request,
//!             DirectionCosineMatrix::identity(),
//!             AngularVelocityVector::from_radians_per_second(0.0, 0.0, 0.0),
//!         )
//!         .map_err(|source| frames::BodyFixedTransformProviderError::InvalidTransform {
//!             source: Box::new(source),
//!         })
//!     }
//! }
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let coefficients = Coefficients(HarmonicCoefficientMetadata {
//!     source: ReferenceDataDescriptor {
//!         authority: "mission data provider".into(),
//!         product: "static gravity coefficients".into(),
//!         revision: "release-1".into(),
//!         checksum: Some("sha256:caller-computed-checksum".into()),
//!     },
//!     normalization: CoefficientNormalization::FullyNormalized4Pi,
//!     tide_system: TideSystem::TideFree,
//!     coefficient_frame: ReferenceFrame::ITRF2020,
//!     maximum_degree: 2,
//!     maximum_order: 2,
//!     epoch_semantics: CoefficientEpochSemantics::TimeInvariant,
//! });
//! let transform = SourcedBodyFixedTransformProvider::new(
//!     Arc::new(CallerTransform),
//!     vec![ReferenceDataDescriptor {
//!         authority: "mission frame provider".into(),
//!         product: "body-fixed rotation".into(),
//!         revision: "frame-release-1".into(),
//!         checksum: None,
//!     }],
//! )?;
//! let model = SphericalHarmonicGravityModel::new(
//!     SphericalHarmonicField {
//!         gravitational_parameter: GravitationalParameter::try_from(3.986_004_418e14)?,
//!         reference_radius: Length::new::<meter>(6_378_137.0),
//!         origin: ReferenceFrame::GCRF.origin(),
//!         inertial_frame: frames::InertialFrame::GCRF,
//!         body_fixed_frame: ReferenceFrame::ITRF2020,
//!         time_scale: TimeScale::TAI,
//!         maximum_degree: 2,
//!         maximum_order: 2,
//!     },
//!     Arc::new(coefficients),
//!     transform,
//! )?;
//! # let _ = (model, Epoch::from_tai_seconds(0.0));
//! # Ok(())
//! # }
//! ```

use std::{
    fmt,
    ops::{Add, Div, Mul, Neg, Sub},
    sync::Arc,
};

use dynamics::{
    CartesianForceModelError, EvaluableCartesianForceModel, Force, ForceModel,
    SpacecraftStateRequirements,
};
use frames::{
    BodyFixedTransformDirection, BodyFixedTransformProvider, BodyFixedTransformRequest,
    FrameOrigin, InertialFrame, ReferenceDataDescriptor, ReferenceFrame,
};
use hifitime::{Epoch, TimeScale};
use orbits::cartesian::{CartesianState, FramedAcceleration};
use thiserror::Error;
use units::uom::si::frequency::hertz;
use units::uom::si::length::meter;
use units::{AccelerationVector, Frequency, GravitationalParameter, Length};

/// Normalization convention of a gravity coefficient set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CoefficientNormalization {
    /// Geodesy fully normalized 4π coefficients, with `P̄₀₀ = 1`.
    FullyNormalized4Pi,
    /// Schmidt semi-normalized associated Legendre functions.
    SchmidtSemiNormalized,
    /// Unnormalized associated Legendre functions.
    Unnormalized,
}

/// Tide system embedded in a coefficient set.
///
/// This is descriptive metadata only. The gravity model does not convert a
/// coefficient set or apply permanent, solid, ocean, or pole tides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TideSystem {
    /// Permanent tide removed from the coefficients.
    TideFree,
    /// Permanent tide retained in the geopotential but removed from the
    /// reference ellipsoid.
    ZeroTide,
    /// Permanent tide retained in both the geopotential and reference
    /// ellipsoid.
    MeanTide,
}

/// Epoch interpretation for an immutable coefficient set.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum CoefficientEpochSemantics {
    /// The selected coefficients are treated as time-invariant.
    TimeInvariant,
    /// The selected coefficients are valid only in the inclusive interval.
    ///
    /// Endpoints are interpreted in `time_scale`; evaluation outside this
    /// interval fails instead of extrapolating.
    ValidBetween {
        /// Inclusive beginning of validity.
        start: Epoch,
        /// Inclusive end of validity.
        end: Epoch,
        /// Time scale in which `start` and `end` are expressed.
        time_scale: TimeScale,
    },
}

/// Provenance and conventions declared by one coefficient provider.
#[derive(Debug, PartialEq)]
pub struct HarmonicCoefficientMetadata {
    /// Immutable source/product/revision identity for the selected data.
    pub source: ReferenceDataDescriptor,
    /// Normalization used by every coefficient from this provider.
    pub normalization: CoefficientNormalization,
    /// Permanent-tide convention embedded in this coefficient set.
    pub tide_system: TideSystem,
    /// Frame whose body-fixed axes define coefficient longitude and latitude.
    pub coefficient_frame: ReferenceFrame,
    /// Highest degree represented by this provider.
    pub maximum_degree: u32,
    /// Highest order represented by this provider.
    pub maximum_order: u32,
    /// Whether these immutable coefficients are epoch independent or bounded.
    pub epoch_semantics: CoefficientEpochSemantics,
}

/// One dimensionless, fully normalized cosine/sine coefficient pair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HarmonicCoefficient {
    /// Cosine coefficient `C̄ₙₘ`.
    pub cosine: f64,
    /// Sine coefficient `S̄ₙₘ`.
    pub sine: f64,
}

/// Caller-owned, immutable coefficient data for a gravity field.
///
/// Every coefficient in the model's selected triangular degree/order region
/// must be present. An explicit zero must be returned for a coefficient that
/// is physically absent; `None` means missing coverage and is a typed error.
pub trait HarmonicCoefficientProvider: fmt::Debug + Send + Sync {
    /// Returns stable provenance, conventions, frame, and coverage metadata.
    fn metadata(&self) -> &HarmonicCoefficientMetadata;

    /// Returns one coefficient, or `None` when the provider lacks coverage.
    fn coefficient(&self, degree: u32, order: u32) -> Option<HarmonicCoefficient>;
}

/// Closed validity interval and time scale for epoch-dependent coefficient
/// deltas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoefficientDeltaCoverage {
    /// Inclusive beginning of the validity interval.
    pub start: Epoch,
    /// Inclusive end of the validity interval.
    pub end: Epoch,
    /// Time scale used to compare evaluation epochs with the interval.
    pub time_scale: TimeScale,
}

/// Provenance and conventions declared by an epoch-dependent coefficient
/// delta provider.
#[derive(Debug, PartialEq)]
pub struct HarmonicCoefficientDeltaMetadata {
    /// Immutable source/product/revision identity for the selected delta data.
    pub source: ReferenceDataDescriptor,
    /// Normalization used by every coefficient delta.
    pub normalization: CoefficientNormalization,
    /// Tide convention to which the deltas apply.
    pub tide_system: TideSystem,
    /// Frame whose body-fixed axes define coefficient longitude and latitude.
    pub coefficient_frame: ReferenceFrame,
    /// Highest degree represented by this provider.
    pub maximum_degree: u32,
    /// Highest order represented by this provider.
    pub maximum_order: u32,
    /// Closed epoch validity interval.
    pub coverage: CoefficientDeltaCoverage,
}

/// Caller-owned, immutable epoch-dependent normalized coefficient deltas.
///
/// Delta values are dimensionless changes to fully normalized coefficients;
/// each request uses an absolute Hifitime epoch. Implementations must return
/// `None` for coefficients not covered by their declared degree/order region.
pub trait HarmonicCoefficientDeltaProvider: fmt::Debug + Send + Sync {
    /// Returns stable provenance, conventions, frame, and closed coverage.
    fn metadata(&self) -> &HarmonicCoefficientDeltaMetadata;

    /// Returns the normalized cosine/sine delta at `epoch`, or `None` when the
    /// coefficient is not covered.
    fn coefficient_delta(
        &self,
        epoch: Epoch,
        degree: u32,
        order: u32,
    ) -> Option<HarmonicCoefficient>;
}

/// Rate of change of one dimensionless normalized harmonic coefficient.
///
/// Frequencies use SI reciprocal seconds, so multiplying by elapsed seconds
/// yields a dimensionless coefficient delta.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HarmonicCoefficientRate {
    /// Cosine-coefficient rate, in s⁻¹.
    pub cosine: Frequency,
    /// Sine-coefficient rate, in s⁻¹.
    pub sine: Frequency,
}

/// Linear secular coefficient-delta provider relative to a reference epoch.
///
/// Each coefficient evolves as `ΔC̄ₙₘ(t) = Ċ̄ₙₘ (t - t₀)` and similarly for
/// `S̄ₙₘ`. Rates are SI reciprocal seconds and epochs are absolute Hifitime
/// instants. Evaluation is valid only within the metadata's inclusive coverage.
pub struct LinearSecularRateProvider {
    metadata: HarmonicCoefficientDeltaMetadata,
    reference_epoch: Epoch,
    rates: Vec<Vec<HarmonicCoefficientRate>>,
}

impl fmt::Debug for LinearSecularRateProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LinearSecularRateProvider")
            .field("metadata", &self.metadata)
            .field("reference_epoch", &self.reference_epoch)
            .finish_non_exhaustive()
    }
}

impl LinearSecularRateProvider {
    /// Validates and snapshots a complete triangular table of per-second
    /// coefficient rates.
    pub fn new(
        metadata: HarmonicCoefficientDeltaMetadata,
        reference_epoch: Epoch,
        rates: Vec<Vec<HarmonicCoefficientRate>>,
    ) -> Result<Self, ConstructionError> {
        validate_delta_metadata(&metadata)?;
        let reference = reference_epoch.to_time_scale(metadata.coverage.time_scale);
        let start = metadata
            .coverage
            .start
            .to_time_scale(metadata.coverage.time_scale);
        let end = metadata
            .coverage
            .end
            .to_time_scale(metadata.coverage.time_scale);
        if reference < start || reference > end {
            return Err(ConstructionError::DeltaReferenceEpochOutOfRange);
        }
        for degree in 0..=metadata.maximum_degree {
            let expected_order = degree.min(metadata.maximum_order);
            for order in 0..=expected_order {
                let rate = rates
                    .get(
                        usize::try_from(degree)
                            .map_err(|_| ConstructionError::CoefficientTableTooLarge)?,
                    )
                    .and_then(|row| usize::try_from(order).ok().and_then(|order| row.get(order)))
                    .ok_or(ConstructionError::MissingCoefficientRate { degree, order })?;
                if !rate.cosine.get::<hertz>().is_finite() || !rate.sine.get::<hertz>().is_finite()
                {
                    return Err(ConstructionError::NonFiniteCoefficientRate { degree, order });
                }
                if degree == 0
                    && order == 0
                    && (rate.cosine.get::<hertz>() != 0.0 || rate.sine.get::<hertz>() != 0.0)
                {
                    return Err(ConstructionError::NonZeroCentralCoefficientRate);
                }
            }
        }
        Ok(Self {
            metadata,
            reference_epoch,
            rates,
        })
    }

    /// Returns the reference epoch from which all secular rates are measured.
    #[must_use]
    pub const fn reference_epoch(&self) -> Epoch {
        self.reference_epoch
    }
}

impl HarmonicCoefficientDeltaProvider for LinearSecularRateProvider {
    fn metadata(&self) -> &HarmonicCoefficientDeltaMetadata {
        &self.metadata
    }

    fn coefficient_delta(
        &self,
        epoch: Epoch,
        degree: u32,
        order: u32,
    ) -> Option<HarmonicCoefficient> {
        let rate = self
            .rates
            .get(usize::try_from(degree).ok()?)?
            .get(usize::try_from(order).ok()?)?;
        let elapsed_seconds = (epoch - self.reference_epoch).to_seconds();
        Some(HarmonicCoefficient {
            cosine: rate.cosine.get::<hertz>().mul_add(elapsed_seconds, 0.0),
            sine: rate.sine.get::<hertz>().mul_add(elapsed_seconds, 0.0),
        })
    }
}

fn validate_delta_metadata(
    metadata: &HarmonicCoefficientDeltaMetadata,
) -> Result<(), ConstructionError> {
    if metadata.maximum_order > metadata.maximum_degree {
        return Err(ConstructionError::InvalidDeltaProviderDegreeOrder);
    }
    if metadata.coverage.end < metadata.coverage.start {
        return Err(ConstructionError::InvalidDeltaCoverage);
    }
    if let Some(field) = invalid_provenance_field(&metadata.source) {
        return Err(ConstructionError::InvalidProvenance {
            provider: ProvenanceProvider::CoefficientDeltas,
            record: 0,
            field,
        });
    }
    Ok(())
}

/// A frame-transform provider together with its caller-declared provenance.
///
/// The underlying frame API intentionally does not prescribe a particular
/// Earth-orientation product. Records must match a data-backed provider's
/// native provenance. Data-free providers may use caller-declared records
/// identifying their transformation convention.
pub struct SourcedBodyFixedTransformProvider {
    provider: Arc<dyn BodyFixedTransformProvider>,
    reference_data: Vec<ReferenceDataDescriptor>,
}

impl fmt::Debug for SourcedBodyFixedTransformProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourcedBodyFixedTransformProvider")
            .field("reference_data", &self.reference_data)
            .finish_non_exhaustive()
    }
}

impl SourcedBodyFixedTransformProvider {
    /// Selects a transform provider and the immutable records identifying its
    /// frame data and conventions.
    ///
    /// If the provider exposes reference data, the supplied records must match
    /// them exactly; caller records cannot override the provider's provenance.
    pub fn new(
        provider: Arc<dyn BodyFixedTransformProvider>,
        reference_data: Vec<ReferenceDataDescriptor>,
    ) -> Result<Self, ConstructionError> {
        if reference_data.is_empty() {
            return Err(ConstructionError::MissingTransformProvenance);
        }
        for (record, descriptor) in reference_data.iter().enumerate() {
            if let Some(field) = invalid_provenance_field(descriptor) {
                return Err(ConstructionError::InvalidProvenance {
                    provider: ProvenanceProvider::FrameTransform,
                    record,
                    field,
                });
            }
        }
        if !provider.reference_data().is_empty()
            && provider.reference_data() != reference_data.as_slice()
        {
            return Err(ConstructionError::TransformProvenanceMismatch);
        }
        Ok(Self {
            provider,
            reference_data,
        })
    }

    /// Returns the selected frame-transform provenance records.
    #[must_use]
    pub fn reference_data(&self) -> &[ReferenceDataDescriptor] {
        &self.reference_data
    }
}

/// Physical constants, frame identities, and selected truncation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SphericalHarmonicField {
    /// Standard gravitational parameter `μ`, in m³/s².
    pub gravitational_parameter: GravitationalParameter,
    /// Harmonic reference radius `R`, in metres.
    pub reference_radius: Length,
    /// Origin shared by the gravity field and both configured frames.
    pub origin: FrameOrigin,
    /// Inertial axes in which the input state and returned acceleration are
    /// expressed.
    pub inertial_frame: InertialFrame,
    /// Body-fixed axes to which the coefficient set is referred.
    pub body_fixed_frame: ReferenceFrame,
    /// Time scale used for every body-fixed transform request.
    pub time_scale: TimeScale,
    /// Maximum degree selected for this evaluation.
    pub maximum_degree: u32,
    /// Maximum order selected for this evaluation.
    pub maximum_order: u32,
}

/// Recoverable coefficient/model configuration failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConstructionError {
    /// The gravitational parameter was not finite and positive.
    #[error("gravitational parameter must be finite and positive")]
    InvalidGravitationalParameter,
    /// The reference radius was not finite and positive.
    #[error("reference radius must be finite and positive")]
    InvalidReferenceRadius,
    /// Inertial, body-fixed, and coefficient frames do not share one origin.
    #[error("gravity field, coefficient, and transform frames must share one origin")]
    FrameOriginMismatch,
    /// Coefficients are expressed in another body-fixed frame.
    #[error("coefficient frame does not match the configured body-fixed frame")]
    CoefficientFrameMismatch {
        /// Frame declared by the coefficient data.
        coefficient_frame: Box<ReferenceFrame>,
        /// Frame selected by the gravity model.
        configured_frame: Box<ReferenceFrame>,
    },
    /// The selected maximum order is greater than the selected degree.
    #[error("selected maximum order must not exceed the selected maximum degree")]
    OrderExceedsDegree,
    /// Coefficient provider declares internally inconsistent dimensions.
    #[error("coefficient provider maximum order exceeds its maximum degree")]
    InvalidProviderDegreeOrder,
    /// Delta provider declares internally inconsistent dimensions.
    #[error("coefficient-delta provider maximum order exceeds its maximum degree")]
    InvalidDeltaProviderDegreeOrder,
    /// Delta validity coverage has reversed endpoints.
    #[error("coefficient-delta validity interval end precedes its start")]
    InvalidDeltaCoverage,
    /// Linear-rate reference epoch is outside declared validity coverage.
    #[error("coefficient-rate reference epoch is outside its declared coverage")]
    DeltaReferenceEpochOutOfRange,
    /// A rate is missing from the selected triangular region.
    #[error("coefficient rate ({degree},{order}) is missing from the selected data")]
    MissingCoefficientRate {
        /// Missing degree.
        degree: u32,
        /// Missing order.
        order: u32,
    },
    /// A rate has a NaN or infinite value.
    #[error("coefficient rate ({degree},{order}) is not finite")]
    NonFiniteCoefficientRate {
        /// Invalid degree.
        degree: u32,
        /// Invalid order.
        order: u32,
    },
    /// A rate would change the unit central coefficient.
    #[error("coefficient rate for C̄₀₀/S̄₀₀ must be zero")]
    NonZeroCentralCoefficientRate,
    /// The delta provider uses a normalization different from the static set.
    #[error("coefficient deltas use a different normalization than the static field")]
    DeltaNormalizationMismatch,
    /// The delta provider declares a different tide convention.
    #[error("coefficient deltas use a different tide system than the static field")]
    DeltaTideSystemMismatch,
    /// The delta provider declares a different coefficient frame.
    #[error("coefficient deltas use a different coefficient frame than the static field")]
    DeltaCoefficientFrameMismatch,
    /// The selected model truncation exceeds delta-provider coverage.
    #[error("coefficient-delta provider does not cover the selected degree/order")]
    InsufficientDeltaCoverage,
    /// The provider uses a convention not implemented by this evaluator.
    #[error("only fully normalized 4π coefficients are supported")]
    UnsupportedNormalization,
    /// The selected model truncation exceeds provider coverage.
    #[error("coefficient provider does not cover the selected degree/order")]
    InsufficientCoverage,
    /// The provider omitted a coefficient in the selected triangular region.
    #[error("coefficient C/S({degree},{order}) is missing from the selected data")]
    MissingCoefficient {
        /// Missing degree.
        degree: u32,
        /// Missing order.
        order: u32,
    },
    /// The supplied coefficient is NaN or infinite.
    #[error("coefficient C/S({degree},{order}) is not finite")]
    NonFiniteCoefficient {
        /// Invalid degree.
        degree: u32,
        /// Invalid order.
        order: u32,
    },
    /// The zeroth-degree coefficient is not the unit central term.
    #[error("C̄₀₀ must equal 1 and S̄₀₀ must equal 0")]
    InvalidCentralCoefficient,
    /// A bounded coefficient validity interval has reversed endpoints.
    #[error("coefficient validity interval end precedes its start")]
    InvalidCoefficientValidityInterval,
    /// Provenance is absent for the selected frame provider.
    #[error("at least one frame-transform provenance record is required")]
    MissingTransformProvenance,
    /// Caller records differ from the selected frame provider's native records.
    #[error("frame-transform provenance does not match the selected provider")]
    TransformProvenanceMismatch,
    /// A source record contains a blank required identity field.
    #[error("{provider:?} provenance record {record} has a blank {field:?} field")]
    InvalidProvenance {
        /// Provider whose record is invalid.
        provider: ProvenanceProvider,
        /// Index of the invalid record.
        record: usize,
        /// Identity field that is blank.
        field: ProvenanceField,
    },
    /// Transform request validation rejected the configured frame pair.
    #[error("body-fixed transform request is invalid: {0}")]
    InvalidTransformRequest(#[from] frames::BodyFixedTransformError),
    /// Degree/order dimensions cannot be represented or allocated.
    #[error("selected coefficient table is too large for this platform")]
    CoefficientTableTooLarge,
}

/// Provider category for a provenance-validation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProvenanceProvider {
    /// Gravity coefficient source.
    GravityCoefficients,
    /// Epoch-dependent coefficient-delta source.
    CoefficientDeltas,
    /// Body-fixed frame transform source.
    FrameTransform,
}

/// Required provenance field that was missing or blank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProvenanceField {
    /// Publishing authority or source.
    Authority,
    /// Product or data family.
    Product,
    /// Immutable release/revision.
    Revision,
    /// Optional but blank checksum.
    Checksum,
}

fn invalid_provenance_field(descriptor: &ReferenceDataDescriptor) -> Option<ProvenanceField> {
    if descriptor.authority.trim().is_empty() {
        Some(ProvenanceField::Authority)
    } else if descriptor.product.trim().is_empty() {
        Some(ProvenanceField::Product)
    } else if descriptor.revision.trim().is_empty() {
        Some(ProvenanceField::Revision)
    } else if descriptor
        .checksum
        .as_deref()
        .is_some_and(|checksum| checksum.trim().is_empty())
    {
        Some(ProvenanceField::Checksum)
    } else {
        None
    }
}

/// Failure while evaluating a configured spherical-harmonic model.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EvaluationError {
    /// Cartesian state does not use the configured inertial frame.
    #[error("Cartesian state frame does not match the configured inertial frame")]
    FrameMismatch,
    /// Cartesian state origin does not match the gravity field origin.
    #[error("Cartesian state origin does not match the gravity field origin")]
    OriginMismatch,
    /// Epoch lies outside an explicitly bounded coefficient validity interval.
    #[error("coefficient set is not valid at requested epoch {requested:?}")]
    CoefficientEpochOutOfRange {
        /// Requested evaluation epoch, expressed in the field time scale.
        requested: Epoch,
        /// Inclusive beginning of coefficient validity.
        valid_from: Epoch,
        /// Inclusive end of coefficient validity.
        valid_to: Epoch,
    },
    /// Epoch lies outside the delta provider's inclusive coverage interval.
    #[error("coefficient deltas are not valid at requested epoch {requested:?}")]
    CoefficientDeltaEpochOutOfRange {
        /// Requested evaluation epoch, expressed in the coverage time scale.
        requested: Epoch,
        /// Inclusive beginning of delta coverage.
        valid_from: Epoch,
        /// Inclusive end of delta coverage.
        valid_to: Epoch,
    },
    /// Delta provider omitted a selected coefficient.
    #[error("coefficient delta C/S({degree},{order}) is missing at the requested epoch")]
    MissingCoefficientDelta {
        /// Missing degree.
        degree: u32,
        /// Missing order.
        order: u32,
    },
    /// A returned coefficient delta is NaN or infinite.
    #[error("coefficient delta C/S({degree},{order}) is not finite")]
    NonFiniteCoefficientDelta {
        /// Invalid degree.
        degree: u32,
        /// Invalid order.
        order: u32,
    },
    /// A provider attempted to change the unit central coefficient.
    #[error("coefficient delta for C̄₀₀/S̄₀₀ must be zero")]
    NonZeroCentralCoefficientDelta,
    /// Adding finite static and time-varying coefficients overflowed.
    #[error("composed coefficient C/S({degree},{order}) is not finite")]
    NonFiniteComposedCoefficient {
        /// Invalid degree.
        degree: u32,
        /// Invalid order.
        order: u32,
    },
    /// Position is non-finite or coincides with the gravity origin.
    #[error("position radius must be finite and non-zero")]
    SingularPosition,
    /// Selected transform provider reported a typed failure.
    #[error("body-fixed transform provider failed: {0}")]
    TransformProvider(#[from] frames::BodyFixedTransformProviderError),
    /// Transform provider returned a result for another frame/epoch/direction.
    #[error("body-fixed transform response did not match the requested transform")]
    TransformResponseMismatch,
    /// Transform result provenance differs from the selected provider records.
    #[error("body-fixed transform response provenance does not match the selected provider")]
    TransformProvenanceMismatch,
    /// The recurrence or returned acceleration is not finite.
    #[error("spherical-harmonic acceleration evaluation is not finite")]
    NonFiniteAcceleration,
}

/// Evaluable static spherical-harmonic gravity force contribution.
pub struct SphericalHarmonicGravityModel {
    field: SphericalHarmonicField,
    metadata: HarmonicCoefficientMetadata,
    coefficients: Vec<Vec<HarmonicCoefficient>>,
    transform_provider: SourcedBodyFixedTransformProvider,
}

/// Spherical-harmonic gravity with epoch-dependent coefficient deltas added
/// to a retained static field.
pub struct TimeVaryingSphericalHarmonicGravityModel {
    static_model: SphericalHarmonicGravityModel,
    delta_provider: Arc<dyn HarmonicCoefficientDeltaProvider>,
}

impl fmt::Debug for SphericalHarmonicGravityModel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SphericalHarmonicGravityModel")
            .field("field", &self.field)
            .field("metadata", &self.metadata)
            .field("transform_provider", &self.transform_provider)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for TimeVaryingSphericalHarmonicGravityModel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TimeVaryingSphericalHarmonicGravityModel")
            .field("static_model", &self.static_model)
            .field("delta_provider", &self.delta_provider)
            .finish()
    }
}

/// Physical identity of the gravity interaction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SphericalHarmonicGravity;

impl Force for SphericalHarmonicGravity {
    fn name(&self) -> &str {
        "gravity"
    }
}

static FORCE: SphericalHarmonicGravity = SphericalHarmonicGravity;

impl SphericalHarmonicGravityModel {
    /// Validates and snapshots one caller-selected coefficient source and
    /// sourced body-fixed transform provider.
    pub fn new(
        field: SphericalHarmonicField,
        coefficients: Arc<dyn HarmonicCoefficientProvider>,
        transform_provider: SourcedBodyFixedTransformProvider,
    ) -> Result<Self, ConstructionError> {
        let mu = field
            .gravitational_parameter
            .as_cubic_metres_per_second_squared();
        let radius = field.reference_radius.get::<meter>();
        if !mu.is_finite() || mu <= 0.0 {
            return Err(ConstructionError::InvalidGravitationalParameter);
        }
        if !radius.is_finite() || radius <= 0.0 {
            return Err(ConstructionError::InvalidReferenceRadius);
        }
        if field.maximum_order > field.maximum_degree {
            return Err(ConstructionError::OrderExceedsDegree);
        }
        if field.inertial_frame.reference_frame().origin() != field.origin
            || field.body_fixed_frame.origin() != field.origin
        {
            return Err(ConstructionError::FrameOriginMismatch);
        }

        let metadata = coefficients.metadata();
        if metadata.maximum_order > metadata.maximum_degree {
            return Err(ConstructionError::InvalidProviderDegreeOrder);
        }
        if metadata.normalization != CoefficientNormalization::FullyNormalized4Pi {
            return Err(ConstructionError::UnsupportedNormalization);
        }
        if metadata.maximum_degree < field.maximum_degree
            || metadata.maximum_order < field.maximum_order
        {
            return Err(ConstructionError::InsufficientCoverage);
        }
        if metadata.coefficient_frame != field.body_fixed_frame {
            return Err(ConstructionError::CoefficientFrameMismatch {
                coefficient_frame: Box::new(metadata.coefficient_frame),
                configured_frame: Box::new(field.body_fixed_frame),
            });
        }
        if let Some(field_name) = invalid_provenance_field(&metadata.source) {
            return Err(ConstructionError::InvalidProvenance {
                provider: ProvenanceProvider::GravityCoefficients,
                record: 0,
                field: field_name,
            });
        }
        if let CoefficientEpochSemantics::ValidBetween { start, end, .. } = metadata.epoch_semantics
        {
            if end < start {
                return Err(ConstructionError::InvalidCoefficientValidityInterval);
            }
        }

        let mut table = Vec::new();
        let row_count = usize::try_from(field.maximum_degree)
            .ok()
            .and_then(|degree| degree.checked_add(1))
            .ok_or(ConstructionError::CoefficientTableTooLarge)?;
        table
            .try_reserve_exact(row_count)
            .map_err(|_| ConstructionError::CoefficientTableTooLarge)?;
        for degree in 0..=field.maximum_degree {
            let row_len = usize::try_from(degree.min(field.maximum_order))
                .ok()
                .and_then(|order| order.checked_add(1))
                .ok_or(ConstructionError::CoefficientTableTooLarge)?;
            let mut row = Vec::new();
            row.try_reserve_exact(row_len)
                .map_err(|_| ConstructionError::CoefficientTableTooLarge)?;
            for order in 0..=degree.min(field.maximum_order) {
                let coefficient = coefficients
                    .coefficient(degree, order)
                    .ok_or(ConstructionError::MissingCoefficient { degree, order })?;
                if !coefficient.cosine.is_finite() || !coefficient.sine.is_finite() {
                    return Err(ConstructionError::NonFiniteCoefficient { degree, order });
                }
                if degree == 0
                    && order == 0
                    && (coefficient.cosine != 1.0 || coefficient.sine != 0.0)
                {
                    return Err(ConstructionError::InvalidCentralCoefficient);
                }
                row.push(coefficient);
            }
            table.push(row);
        }
        BodyFixedTransformRequest::new(
            Epoch::from_tai_seconds(0.0),
            field.time_scale,
            field.inertial_frame,
            field.body_fixed_frame,
            BodyFixedTransformDirection::InertialToBodyFixed,
        )?;

        Ok(Self {
            field,
            metadata: HarmonicCoefficientMetadata {
                source: clone_reference_data(&metadata.source),
                normalization: metadata.normalization,
                tide_system: metadata.tide_system,
                coefficient_frame: metadata.coefficient_frame,
                maximum_degree: metadata.maximum_degree,
                maximum_order: metadata.maximum_order,
                epoch_semantics: metadata.epoch_semantics,
            },
            coefficients: table,
            transform_provider,
        })
    }

    /// Composes an epoch-dependent coefficient-delta provider onto this
    /// unchanged static field.
    ///
    /// The provider must use the same normalization, tide system, and
    /// coefficient frame as this model, cover its selected degree/order, and
    /// declare non-empty provenance and a closed validity interval.
    pub fn with_coefficient_deltas(
        self,
        delta_provider: Arc<dyn HarmonicCoefficientDeltaProvider>,
    ) -> Result<TimeVaryingSphericalHarmonicGravityModel, ConstructionError> {
        let delta = delta_provider.metadata();
        validate_delta_metadata(delta)?;
        if delta.normalization != self.metadata.normalization {
            return Err(ConstructionError::DeltaNormalizationMismatch);
        }
        if delta.tide_system != self.metadata.tide_system {
            return Err(ConstructionError::DeltaTideSystemMismatch);
        }
        if delta.coefficient_frame != self.metadata.coefficient_frame {
            return Err(ConstructionError::DeltaCoefficientFrameMismatch);
        }
        if delta.maximum_degree < self.field.maximum_degree
            || delta.maximum_order < self.field.maximum_order
        {
            return Err(ConstructionError::InsufficientDeltaCoverage);
        }
        Ok(TimeVaryingSphericalHarmonicGravityModel {
            static_model: self,
            delta_provider,
        })
    }

    /// Returns the physical constants, frame identities, and truncation.
    #[must_use]
    pub const fn field(&self) -> SphericalHarmonicField {
        self.field
    }

    /// Returns the immutable coefficient-set provenance and conventions.
    #[must_use]
    pub const fn metadata(&self) -> &HarmonicCoefficientMetadata {
        &self.metadata
    }

    /// Returns the selected body-fixed transform's source records.
    #[must_use]
    pub fn transform_reference_data(&self) -> &[ReferenceDataDescriptor] {
        self.transform_provider.reference_data()
    }

    /// Returns one snapshotted coefficient in the selected degree/order region.
    #[must_use]
    pub fn coefficient(&self, degree: u32, order: u32) -> Option<HarmonicCoefficient> {
        self.coefficients
            .get(usize::try_from(degree).ok()?)?
            .get(usize::try_from(order).ok()?)
            .copied()
    }

    fn evaluate(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, EvaluationError> {
        self.evaluate_with_coefficients(epoch, state, &self.coefficients)
    }

    fn evaluate_with_coefficients(
        &self,
        epoch: Epoch,
        state: &CartesianState,
        coefficients: &[Vec<HarmonicCoefficient>],
    ) -> Result<FramedAcceleration, EvaluationError> {
        self.evaluate_with_coefficient_accessor(epoch, state, |degree, order| {
            usize::try_from(degree)
                .ok()
                .and_then(|degree| coefficients.get(degree))
                .and_then(|row| usize::try_from(order).ok().and_then(|order| row.get(order)))
                .copied()
                .ok_or(EvaluationError::NonFiniteAcceleration)
        })
    }

    fn evaluate_with_coefficient_accessor<F>(
        &self,
        epoch: Epoch,
        state: &CartesianState,
        mut coefficient: F,
    ) -> Result<FramedAcceleration, EvaluationError>
    where
        F: FnMut(u32, u32) -> Result<HarmonicCoefficient, EvaluationError>,
    {
        if state.frame().origin() != self.field.origin {
            return Err(EvaluationError::OriginMismatch);
        }
        if state.frame() != self.field.inertial_frame.reference_frame() {
            return Err(EvaluationError::FrameMismatch);
        }
        let request = BodyFixedTransformRequest::new(
            epoch,
            self.field.time_scale,
            self.field.inertial_frame,
            self.field.body_fixed_frame,
            BodyFixedTransformDirection::InertialToBodyFixed,
        )
        .map_err(|_| EvaluationError::TransformResponseMismatch)?;
        self.validate_coefficient_epoch(request.epoch())?;
        let resolved = self
            .transform_provider
            .provider
            .as_ref()
            .body_fixed_transform_with_provenance(request)?;
        let reference_data = self.transform_provider.provider.reference_data();
        if resolved.reference_data() != reference_data
            || (!reference_data.is_empty()
                && reference_data != self.transform_provider.reference_data())
        {
            return Err(EvaluationError::TransformProvenanceMismatch);
        }
        let transform = resolved.transform();
        if transform.request() != request {
            return Err(EvaluationError::TransformResponseMismatch);
        }

        let inertial_position = state.position().to_metres();
        let body_position = multiply_matrix_vector(transform.rotation().rows(), inertial_position);
        let radius_squared = body_position[0].mul_add(
            body_position[0],
            body_position[1].mul_add(body_position[1], body_position[2] * body_position[2]),
        );
        if !radius_squared.is_finite() || radius_squared <= 0.0 {
            return Err(EvaluationError::SingularPosition);
        }
        let body_acceleration = evaluate_potential_gradient(
            body_position,
            self.field
                .gravitational_parameter
                .as_cubic_metres_per_second_squared(),
            self.field.reference_radius.get::<meter>(),
            self.field.maximum_degree,
            self.field.maximum_order,
            &mut coefficient,
        )?;
        if body_acceleration.iter().any(|value| !value.is_finite()) {
            return Err(EvaluationError::NonFiniteAcceleration);
        }
        let inertial_acceleration = multiply_matrix_vector(
            transpose_matrix(transform.rotation().rows()),
            body_acceleration,
        );
        FramedAcceleration::new(
            AccelerationVector::from_metres_per_second_squared(
                inertial_acceleration[0],
                inertial_acceleration[1],
                inertial_acceleration[2],
            ),
            state.frame(),
        )
        .map_err(|_| EvaluationError::NonFiniteAcceleration)
    }

    fn validate_coefficient_epoch(&self, requested: Epoch) -> Result<(), EvaluationError> {
        if let CoefficientEpochSemantics::ValidBetween {
            start,
            end,
            time_scale,
        } = self.metadata.epoch_semantics
        {
            let requested = requested.to_time_scale(time_scale);
            let valid_from = start.to_time_scale(time_scale);
            let valid_to = end.to_time_scale(time_scale);
            if requested < valid_from || requested > valid_to {
                return Err(EvaluationError::CoefficientEpochOutOfRange {
                    requested,
                    valid_from,
                    valid_to,
                });
            }
        }
        Ok(())
    }
}

fn multiply_matrix_vector(matrix: [[f64; 3]; 3], vector: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|row| {
        matrix[row][0] * vector[0] + matrix[row][1] * vector[1] + matrix[row][2] * vector[2]
    })
}

fn transpose_matrix(matrix: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    std::array::from_fn(|row| std::array::from_fn(|column| matrix[column][row]))
}

fn clone_reference_data(source: &ReferenceDataDescriptor) -> ReferenceDataDescriptor {
    ReferenceDataDescriptor {
        authority: source.authority.clone(),
        product: source.product.clone(),
        revision: source.revision.clone(),
        checksum: source.checksum.clone(),
    }
}

impl ForceModel for SphericalHarmonicGravityModel {
    fn model_name(&self) -> &str {
        "spherical-harmonic gravity model"
    }

    fn force(&self) -> &dyn Force {
        &FORCE
    }

    fn state_requirements(&self) -> SpacecraftStateRequirements {
        SpacecraftStateRequirements::POSITION
    }
}

impl EvaluableCartesianForceModel for SphericalHarmonicGravityModel {
    fn validate_cartesian(&self, state: &CartesianState) -> Result<(), CartesianForceModelError> {
        let error = if state.frame().origin() != self.field.origin {
            EvaluationError::OriginMismatch
        } else if state.frame() != self.field.inertial_frame.reference_frame() {
            EvaluationError::FrameMismatch
        } else {
            return Ok(());
        };
        Err(CartesianForceModelError::new(self.model_name(), error))
    }

    fn cartesian_acceleration(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, CartesianForceModelError> {
        self.evaluate(epoch, state)
            .map_err(|error| CartesianForceModelError::new(self.model_name(), error))
    }
}

impl TimeVaryingSphericalHarmonicGravityModel {
    /// Returns the static field retained by this composition.
    #[must_use]
    pub const fn static_model(&self) -> &SphericalHarmonicGravityModel {
        &self.static_model
    }

    /// Returns the selected coefficient-delta provider.
    #[must_use]
    pub fn delta_provider(&self) -> &dyn HarmonicCoefficientDeltaProvider {
        self.delta_provider.as_ref()
    }

    fn evaluate(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, EvaluationError> {
        let coverage = self.delta_provider.metadata().coverage;
        let requested = epoch.to_time_scale(coverage.time_scale);
        let valid_from = coverage.start.to_time_scale(coverage.time_scale);
        let valid_to = coverage.end.to_time_scale(coverage.time_scale);
        if requested < valid_from || requested > valid_to {
            return Err(EvaluationError::CoefficientDeltaEpochOutOfRange {
                requested,
                valid_from,
                valid_to,
            });
        }

        self.static_model
            .evaluate_with_coefficient_accessor(epoch, state, |degree, order| {
                let static_coefficient = self
                    .static_model
                    .coefficient(degree, order)
                    .ok_or(EvaluationError::NonFiniteAcceleration)?;
                let delta = self
                    .delta_provider
                    .coefficient_delta(epoch, degree, order)
                    .ok_or(EvaluationError::MissingCoefficientDelta { degree, order })?;
                if !delta.cosine.is_finite() || !delta.sine.is_finite() {
                    return Err(EvaluationError::NonFiniteCoefficientDelta { degree, order });
                }
                if degree == 0 && order == 0 && (delta.cosine != 0.0 || delta.sine != 0.0) {
                    return Err(EvaluationError::NonZeroCentralCoefficientDelta);
                }
                let coefficient = HarmonicCoefficient {
                    cosine: if delta.cosine == 0.0 {
                        static_coefficient.cosine
                    } else {
                        static_coefficient.cosine + delta.cosine
                    },
                    sine: if delta.sine == 0.0 {
                        static_coefficient.sine
                    } else {
                        static_coefficient.sine + delta.sine
                    },
                };
                if !coefficient.cosine.is_finite() || !coefficient.sine.is_finite() {
                    return Err(EvaluationError::NonFiniteComposedCoefficient { degree, order });
                }
                Ok(coefficient)
            })
    }
}

impl ForceModel for TimeVaryingSphericalHarmonicGravityModel {
    fn model_name(&self) -> &str {
        "time-varying spherical-harmonic gravity model"
    }

    fn force(&self) -> &dyn Force {
        &FORCE
    }

    fn state_requirements(&self) -> SpacecraftStateRequirements {
        SpacecraftStateRequirements::POSITION
    }
}

impl EvaluableCartesianForceModel for TimeVaryingSphericalHarmonicGravityModel {
    fn validate_cartesian(&self, state: &CartesianState) -> Result<(), CartesianForceModelError> {
        let error = if state.frame().origin() != self.static_model.field.origin {
            EvaluationError::OriginMismatch
        } else if state.frame() != self.static_model.field.inertial_frame.reference_frame() {
            EvaluationError::FrameMismatch
        } else {
            return Ok(());
        };
        Err(CartesianForceModelError::new(self.model_name(), error))
    }

    fn cartesian_acceleration(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, CartesianForceModelError> {
        self.evaluate(epoch, state)
            .map_err(|error| CartesianForceModelError::new(self.model_name(), error))
    }
}

fn evaluate_potential_gradient<F>(
    position: [f64; 3],
    gravitational_parameter: f64,
    reference_radius: f64,
    maximum_degree: u32,
    maximum_order: u32,
    mut coefficient: F,
) -> Result<[f64; 3], EvaluationError>
where
    F: FnMut(u32, u32) -> Result<HarmonicCoefficient, EvaluationError>,
{
    let x = Dual::variable(position[0], 0);
    let y = Dual::variable(position[1], 1);
    let z = Dual::variable(position[2], 2);
    let radius = (x * x + y * y + z * z)
        .sqrt()
        .ok_or(EvaluationError::NonFiniteAcceleration)?;
    let inverse_radius = Dual::constant(1.0) / radius;
    let radial_ratio = Dual::constant(reference_radius) * inverse_radius;
    let unit_x = x * inverse_radius;
    let unit_y = y * inverse_radius;
    let unit_z = z * inverse_radius;
    let longitude_step = ComplexDual {
        real: unit_x,
        imaginary: unit_y,
    };
    let mut harmonic_sum = Dual::constant(0.0);

    for order in 0..=maximum_order {
        let mut diagonal = ComplexDual::ONE;
        for diagonal_degree in 1..=order {
            let k = f64::from(diagonal_degree);
            let normalization = if diagonal_degree == 1 {
                3.0_f64.sqrt()
            } else {
                ((2.0 * k + 1.0) / (2.0 * k)).sqrt()
            };
            diagonal = diagonal.multiply(longitude_step).scale(normalization);
        }

        let mut previous_previous: Option<ComplexDual> = None;
        let mut previous = diagonal;
        let mut radial_power = Dual::constant(1.0);
        for _ in 0..order {
            radial_power = radial_power * radial_ratio;
        }

        for degree in order..=maximum_degree {
            let degree_f = f64::from(degree);
            let order_f = f64::from(order);
            let current = if degree == order {
                diagonal
            } else if degree.checked_sub(order) == Some(1) {
                let next = previous
                    .multiply_real(unit_z)
                    .scale((2.0 * order_f + 3.0).sqrt());
                previous_previous = Some(previous);
                previous = next;
                next
            } else {
                let denominator = degree_f * degree_f - order_f * order_f;
                let first = ((4.0 * degree_f * degree_f - 1.0) / denominator).sqrt();
                let previous_degree = degree_f - 1.0;
                let second = ((2.0 * degree_f + 1.0)
                    * (previous_degree * previous_degree - order_f * order_f)
                    / ((2.0 * degree_f - 3.0) * denominator))
                    .sqrt();
                let next = previous.multiply_real(unit_z).scale(first)
                    - previous_previous
                        .ok_or(EvaluationError::NonFiniteAcceleration)?
                        .scale(second);
                previous_previous = Some(previous);
                previous = next;
                next
            };

            let coefficient = coefficient(degree, order)?;
            harmonic_sum = harmonic_sum
                + (current.real * coefficient.cosine + current.imaginary * coefficient.sine)
                    * radial_power;
            radial_power = radial_power * radial_ratio;
        }
    }

    let potential = harmonic_sum * inverse_radius * gravitational_parameter;
    if potential.derivative_is_finite() {
        Ok(potential.derivative)
    } else {
        Err(EvaluationError::NonFiniteAcceleration)
    }
}

#[derive(Debug, Clone, Copy)]
struct Dual {
    value: f64,
    derivative: [f64; 3],
}

impl Dual {
    const fn constant(value: f64) -> Self {
        Self {
            value,
            derivative: [0.0; 3],
        }
    }

    fn variable(value: f64, index: usize) -> Self {
        let mut derivative = [0.0; 3];
        derivative[index] = 1.0;
        Self { value, derivative }
    }

    fn sqrt(self) -> Option<Self> {
        let value = self.value.sqrt();
        if !value.is_finite() || value == 0.0 {
            return None;
        }
        Some(Self {
            value,
            derivative: self.derivative.map(|derivative| derivative / (2.0 * value)),
        })
    }

    fn derivative_is_finite(self) -> bool {
        self.value.is_finite()
            && self
                .derivative
                .iter()
                .all(|derivative| derivative.is_finite())
    }
}

impl Add for Dual {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            value: self.value + rhs.value,
            derivative: std::array::from_fn(|index| self.derivative[index] + rhs.derivative[index]),
        }
    }
}

impl Sub for Dual {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            value: self.value - rhs.value,
            derivative: std::array::from_fn(|index| self.derivative[index] - rhs.derivative[index]),
        }
    }
}

// The product rule differentiates the scalar multiplication represented here.
#[allow(clippy::suspicious_arithmetic_impl)]
impl Mul for Dual {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            value: self.value * rhs.value,
            derivative: std::array::from_fn(|index| {
                self.derivative[index] * rhs.value + self.value * rhs.derivative[index]
            }),
        }
    }
}

impl Mul<f64> for Dual {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        Self {
            value: self.value * rhs,
            derivative: self.derivative.map(|derivative| derivative * rhs),
        }
    }
}

impl Div for Dual {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        let denominator = rhs.value * rhs.value;
        Self {
            value: self.value / rhs.value,
            derivative: std::array::from_fn(|index| {
                (self.derivative[index] * rhs.value - self.value * rhs.derivative[index])
                    / denominator
            }),
        }
    }
}

impl Neg for Dual {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            value: -self.value,
            derivative: self.derivative.map(|derivative| -derivative),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ComplexDual {
    real: Dual,
    imaginary: Dual,
}

impl ComplexDual {
    const ONE: Self = Self {
        real: Dual::constant(1.0),
        imaginary: Dual::constant(0.0),
    };

    fn multiply(self, rhs: Self) -> Self {
        Self {
            real: self.real * rhs.real - self.imaginary * rhs.imaginary,
            imaginary: self.real * rhs.imaginary + self.imaginary * rhs.real,
        }
    }

    fn multiply_real(self, rhs: Dual) -> Self {
        Self {
            real: self.real * rhs,
            imaginary: self.imaginary * rhs,
        }
    }

    fn scale(self, factor: f64) -> Self {
        Self {
            real: self.real * factor,
            imaginary: self.imaginary * factor,
        }
    }
}

impl Sub for ComplexDual {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            real: self.real - rhs.real,
            imaginary: self.imaginary - rhs.imaginary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frames::{BodyFixedTransform, DirectionCosineMatrix};
    use units::{Position, VelocityVector};

    #[derive(Debug)]
    struct Coefficients {
        metadata: HarmonicCoefficientMetadata,
        values: Vec<Vec<HarmonicCoefficient>>,
    }

    impl HarmonicCoefficientProvider for Coefficients {
        fn metadata(&self) -> &HarmonicCoefficientMetadata {
            &self.metadata
        }

        fn coefficient(&self, degree: u32, order: u32) -> Option<HarmonicCoefficient> {
            self.values
                .get(usize::try_from(degree).ok()?)?
                .get(usize::try_from(order).ok()?)
                .copied()
        }
    }

    #[derive(Debug)]
    struct FixedDeltas {
        metadata: HarmonicCoefficientDeltaMetadata,
        value: Option<HarmonicCoefficient>,
    }

    impl HarmonicCoefficientDeltaProvider for FixedDeltas {
        fn metadata(&self) -> &HarmonicCoefficientDeltaMetadata {
            &self.metadata
        }

        fn coefficient_delta(
            &self,
            _epoch: Epoch,
            _degree: u32,
            _order: u32,
        ) -> Option<HarmonicCoefficient> {
            self.value
        }
    }

    #[derive(Debug)]
    struct RotationProvider {
        angular_rate: f64,
        bad_request: bool,
    }

    impl BodyFixedTransformProvider for RotationProvider {
        fn body_fixed_transform(
            &self,
            request: BodyFixedTransformRequest,
        ) -> Result<BodyFixedTransform, frames::BodyFixedTransformProviderError> {
            let elapsed = request.epoch().to_tai_seconds();
            let angle = self.angular_rate * elapsed;
            let (sin, cos) = angle.sin_cos();
            let rotation =
                DirectionCosineMatrix::new([[cos, sin, 0.0], [-sin, cos, 0.0], [0.0, 0.0, 1.0]])
                    .expect("rotation matrix");
            let answered_request = if self.bad_request {
                BodyFixedTransformRequest::new(
                    request.epoch() + hifitime::Duration::from_seconds(1.0),
                    request.time_scale(),
                    request.inertial_frame(),
                    request.body_fixed_frame(),
                    request.direction(),
                )
                .expect("valid alternate request")
            } else {
                request
            };
            BodyFixedTransform::new(
                answered_request,
                rotation,
                units::AngularVelocityVector::from_radians_per_second(0.0, 0.0, self.angular_rate),
            )
            .map_err(|error| {
                frames::BodyFixedTransformProviderError::InvalidTransform {
                    source: Box::new(error),
                }
            })
        }
    }

    #[derive(Debug)]
    struct ProvenancedRotationProvider {
        reference_data: Vec<ReferenceDataDescriptor>,
        omit_response_provenance: bool,
    }

    impl BodyFixedTransformProvider for ProvenancedRotationProvider {
        fn reference_data(&self) -> &[ReferenceDataDescriptor] {
            &self.reference_data
        }

        fn body_fixed_transform(
            &self,
            request: BodyFixedTransformRequest,
        ) -> Result<BodyFixedTransform, frames::BodyFixedTransformProviderError> {
            RotationProvider {
                angular_rate: 0.0,
                bad_request: false,
            }
            .body_fixed_transform(request)
        }

        fn body_fixed_transform_with_provenance<'a>(
            &'a self,
            request: BodyFixedTransformRequest,
        ) -> Result<
            frames::ProvenancedBodyFixedTransform<'a>,
            frames::BodyFixedTransformProviderError,
        > {
            Ok(frames::ProvenancedBodyFixedTransform::new(
                self.body_fixed_transform(request)?,
                if self.omit_response_provenance {
                    &[]
                } else {
                    self.reference_data()
                },
            ))
        }
    }

    fn metadata(
        degree: u32,
        order: u32,
        frame: ReferenceFrame,
        epoch_semantics: CoefficientEpochSemantics,
    ) -> HarmonicCoefficientMetadata {
        HarmonicCoefficientMetadata {
            source: ReferenceDataDescriptor {
                authority: "synthetic independent test data".into(),
                product: "spherical-harmonic test field".into(),
                revision: "test-1".into(),
                checksum: Some("sha256:test-fixture".into()),
            },
            normalization: CoefficientNormalization::FullyNormalized4Pi,
            tide_system: TideSystem::TideFree,
            coefficient_frame: frame,
            maximum_degree: degree,
            maximum_order: order,
            epoch_semantics,
        }
    }

    fn delta_metadata(
        degree: u32,
        order: u32,
        normalization: CoefficientNormalization,
        tide_system: TideSystem,
        coefficient_frame: ReferenceFrame,
    ) -> HarmonicCoefficientDeltaMetadata {
        HarmonicCoefficientDeltaMetadata {
            source: ReferenceDataDescriptor {
                authority: "synthetic independent test data".into(),
                product: "coefficient-delta test rates".into(),
                revision: "delta-test-1".into(),
                checksum: None,
            },
            normalization,
            tide_system,
            coefficient_frame,
            maximum_degree: degree,
            maximum_order: order,
            coverage: CoefficientDeltaCoverage {
                start: Epoch::from_tai_seconds(0.0),
                end: Epoch::from_tai_seconds(20_000.0),
                time_scale: TimeScale::TAI,
            },
        }
    }

    fn linear_delta_provider(
        rates: Vec<Vec<HarmonicCoefficientRate>>,
    ) -> LinearSecularRateProvider {
        LinearSecularRateProvider::new(
            delta_metadata(
                u32::try_from(rates.len() - 1).expect("degree"),
                u32::try_from(rates.last().expect("rate rows").len() - 1).expect("order"),
                CoefficientNormalization::FullyNormalized4Pi,
                TideSystem::TideFree,
                ReferenceFrame::ITRF2020,
            ),
            Epoch::from_tai_seconds(0.0),
            rates,
        )
        .expect("linear delta provider")
    }

    fn coefficient_rate(cosine: f64, sine: f64) -> HarmonicCoefficientRate {
        HarmonicCoefficientRate {
            cosine: Frequency::new::<hertz>(cosine),
            sine: Frequency::new::<hertz>(sine),
        }
    }

    fn sourced_transform(angular_rate: f64) -> SourcedBodyFixedTransformProvider {
        SourcedBodyFixedTransformProvider::new(
            Arc::new(RotationProvider {
                angular_rate,
                bad_request: false,
            }),
            vec![ReferenceDataDescriptor {
                authority: "synthetic test transform".into(),
                product: "body-fixed rotation".into(),
                revision: "rotation-1".into(),
                checksum: None,
            }],
        )
        .expect("valid transform provenance")
    }

    fn model(
        degree: u32,
        order: u32,
        values: Vec<Vec<HarmonicCoefficient>>,
        epoch_semantics: CoefficientEpochSemantics,
        angular_rate: f64,
    ) -> SphericalHarmonicGravityModel {
        let coefficient_provider = Coefficients {
            metadata: metadata(degree, order, ReferenceFrame::ITRF2020, epoch_semantics),
            values,
        };
        SphericalHarmonicGravityModel::new(
            SphericalHarmonicField {
                gravitational_parameter: GravitationalParameter::try_from(3.986_004_418e14)
                    .expect("positive μ"),
                reference_radius: Length::new::<meter>(6_378_137.0),
                origin: ReferenceFrame::GCRF.origin(),
                inertial_frame: InertialFrame::GCRF,
                body_fixed_frame: ReferenceFrame::ITRF2020,
                time_scale: TimeScale::TAI,
                maximum_degree: degree,
                maximum_order: order,
            },
            Arc::new(coefficient_provider),
            sourced_transform(angular_rate),
        )
        .expect("valid harmonic model")
    }

    fn synthetic_coefficients(degree: u32, order: u32) -> Vec<Vec<HarmonicCoefficient>> {
        (0..=degree)
            .map(|n| {
                (0..=n.min(order))
                    .map(|m| {
                        if n == 0 && m == 0 {
                            HarmonicCoefficient {
                                cosine: 1.0,
                                sine: 0.0,
                            }
                        } else {
                            let scale = 1.0e-6 / f64::from((n + 1) * (m + 1));
                            HarmonicCoefficient {
                                cosine: scale * f64::from((n + m) % 3 + 1),
                                sine: -scale * f64::from((2 * n + m) % 4 + 1),
                            }
                        }
                    })
                    .collect()
            })
            .collect()
    }

    fn state(position: [f64; 3]) -> CartesianState {
        CartesianState::new(
            ReferenceFrame::GCRF,
            Position::from_metres(position[0], position[1], position[2]),
            VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
        )
        .expect("finite inertial state")
    }

    fn independent_potential(
        position: [f64; 3],
        mu: f64,
        reference_radius: f64,
        maximum_degree: u32,
        maximum_order: u32,
        coefficients: &[Vec<HarmonicCoefficient>],
    ) -> f64 {
        let radius = position
            .iter()
            .map(|value| value * value)
            .sum::<f64>()
            .sqrt();
        let sine_latitude = position[2] / radius;
        let longitude = position[1].atan2(position[0]);
        let mut potential_sum = 0.0;
        for degree in 0..=maximum_degree {
            let mut legendre = vec![0.0; degree as usize + 1];
            legendre[0] = if degree == 0 { 1.0 } else { 0.0 };
            if degree > 0 {
                let mut previous = vec![0.0; degree as usize + 1];
                previous[0] = 1.0;
                let mut previous_previous = vec![0.0; degree as usize + 1];
                for n in 1..=degree {
                    let mut current = vec![0.0; degree as usize + 1];
                    for order in 0..=n {
                        current[order as usize] = if order == n {
                            f64::from(2 * n - 1)
                                * (1.0 - sine_latitude * sine_latitude).sqrt()
                                * previous[(order - 1) as usize]
                        } else if order == n - 1 {
                            f64::from(2 * n - 1) * sine_latitude * previous[order as usize]
                        } else {
                            (f64::from(2 * n - 1) * sine_latitude * previous[order as usize]
                                - f64::from(n + order - 1) * previous_previous[order as usize])
                                / f64::from(n - order)
                        };
                    }
                    previous_previous = previous;
                    previous = current;
                }
                legendre = previous;
            }
            let radial = (reference_radius / radius).powi(degree as i32);
            for order in 0..=degree.min(maximum_order) {
                let mut factorial_ratio = 1.0;
                for k in (degree - order + 1)..=(degree + order) {
                    factorial_ratio /= f64::from(k);
                }
                let order_scale = if order == 0 { 1.0 } else { 2.0 };
                let normalization =
                    (order_scale * f64::from(2 * degree + 1) * factorial_ratio).sqrt();
                let angle = f64::from(order) * longitude;
                let coefficient = coefficients[degree as usize][order as usize];
                potential_sum += radial
                    * normalization
                    * legendre[order as usize]
                    * (coefficient.cosine * angle.cos() + coefficient.sine * angle.sin());
            }
        }
        mu / radius * potential_sum
    }

    fn independent_gradient(
        position: [f64; 3],
        degree: u32,
        order: u32,
        coefficients: &[Vec<HarmonicCoefficient>],
    ) -> [f64; 3] {
        let h = 100.0;
        std::array::from_fn(|axis| {
            let mut plus_one = position;
            let mut plus_two = position;
            let mut minus_one = position;
            let mut minus_two = position;
            plus_one[axis] += h;
            plus_two[axis] += 2.0 * h;
            minus_one[axis] -= h;
            minus_two[axis] -= 2.0 * h;
            let potential = |point| {
                independent_potential(
                    point,
                    3.986_004_418e14,
                    6_378_137.0,
                    degree,
                    order,
                    coefficients,
                )
            };
            (-potential(plus_two) + 8.0 * potential(plus_one) - 8.0 * potential(minus_one)
                + potential(minus_two))
                / (12.0 * h)
        })
    }

    fn rotate(rotation: DirectionCosineMatrix, vector: [f64; 3]) -> [f64; 3] {
        let rows = rotation.rows();
        std::array::from_fn(|row| {
            rows[row][0] * vector[0] + rows[row][1] * vector[1] + rows[row][2] * vector[2]
        })
    }

    #[test]
    fn matches_independent_potential_gradient_across_truncations_axes_radii_and_epochs() {
        let epochs = [
            Epoch::from_tai_seconds(0.0),
            Epoch::from_tai_seconds(5_000.0),
            Epoch::from_tai_seconds(20_000.0),
        ];
        let positions = [
            [7_000_000.0, -1_200_000.0, 2_000_000.0],
            [8_000_000.0, 0.0, 0.0],
            [0.0, -9_000_000.0, 0.0],
            [0.0, 0.0, 7_500_000.0],
            [0.0, 0.0, -12_000_000.0],
        ];
        for (degree, order) in [(0, 0), (2, 0), (3, 1), (5, 3), (8, 8), (20, 13)] {
            let coefficients = synthetic_coefficients(degree, order);
            let model = model(
                degree,
                order,
                coefficients.clone(),
                CoefficientEpochSemantics::TimeInvariant,
                7.0e-5,
            );
            for epoch in epochs {
                let angle = 7.0e-5 * epoch.to_tai_seconds();
                let (sin, cos) = angle.sin_cos();
                let rotation = DirectionCosineMatrix::new([
                    [cos, sin, 0.0],
                    [-sin, cos, 0.0],
                    [0.0, 0.0, 1.0],
                ])
                .expect("rotation");
                for inertial in positions {
                    let body = rotate(rotation, inertial);
                    let expected_body = independent_gradient(body, degree, order, &coefficients);
                    let expected_inertial = rotate(
                        DirectionCosineMatrix::new([
                            [cos, -sin, 0.0],
                            [sin, cos, 0.0],
                            [0.0, 0.0, 1.0],
                        ])
                        .expect("inverse rotation"),
                        expected_body,
                    );
                    let actual = model
                        .cartesian_acceleration(epoch, &state(inertial))
                        .expect("evaluated acceleration")
                        .value()
                        .to_metres_per_second_squared();
                    for axis in 0..3 {
                        let absolute_tolerance: f64 = 2.0e-9;
                        let relative_tolerance = 2.0e-9 * expected_inertial[axis].abs();
                        assert!(
                            (actual[axis] - expected_inertial[axis]).abs()
                                < absolute_tolerance.max(relative_tolerance),
                            "degree/order=({degree},{order}), epoch={epoch:?}, \
                             position={inertial:?}, axis={axis}, actual={}, expected={}",
                            actual[axis],
                            expected_inertial[axis]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn central_and_zonal_truncations_match_closed_form_point_mass_and_j2() {
        let values = vec![
            vec![HarmonicCoefficient {
                cosine: 1.0,
                sine: 0.0,
            }],
            vec![HarmonicCoefficient {
                cosine: 0.0,
                sine: 0.0,
            }],
            vec![HarmonicCoefficient {
                cosine: -1.082_626_68e-3 / 5.0_f64.sqrt(),
                sine: 0.0,
            }],
        ];
        let position = [7_000_000.0, -1_200_000.0, 2_000_000.0];
        let acceleration = model(2, 0, values, CoefficientEpochSemantics::TimeInvariant, 0.0)
            .cartesian_acceleration(Epoch::from_tai_seconds(0.0), &state(position))
            .expect("acceleration")
            .value()
            .to_metres_per_second_squared();
        let r2 = position.iter().map(|value| value * value).sum::<f64>();
        let r = r2.sqrt();
        let z2_r2 = position[2] * position[2] / r2;
        let mu = 3.986_004_418e14;
        let j2 = 1.082_626_68e-3;
        let base = std::array::from_fn::<_, 3, _>(|axis| -mu * position[axis] / r.powi(3));
        let factor = 1.5 * mu * j2 * 6_378_137.0_f64.powi(2) / r.powi(5);
        let expected = [
            base[0] + factor * position[0] * (5.0 * z2_r2 - 1.0),
            base[1] + factor * position[1] * (5.0 * z2_r2 - 1.0),
            base[2] + factor * position[2] * (5.0 * z2_r2 - 3.0),
        ];
        for axis in 0..3 {
            assert!(
                (acceleration[axis] - expected[axis]).abs() < 2.0e-12,
                "axis={axis}, actual={}, expected={}",
                acceleration[axis],
                expected[axis]
            );
        }
    }

    #[test]
    fn zero_coefficient_deltas_are_bit_identical_to_the_static_model() {
        let coefficients = synthetic_coefficients(2, 1);
        let static_model = model(
            2,
            1,
            coefficients.clone(),
            CoefficientEpochSemantics::TimeInvariant,
            3.0e-5,
        );
        let rates = vec![
            vec![coefficient_rate(0.0, 0.0)],
            vec![coefficient_rate(0.0, 0.0), coefficient_rate(0.0, 0.0)],
            vec![coefficient_rate(0.0, 0.0), coefficient_rate(0.0, 0.0)],
        ];
        let time_varying = model(
            2,
            1,
            coefficients,
            CoefficientEpochSemantics::TimeInvariant,
            3.0e-5,
        )
        .with_coefficient_deltas(Arc::new(linear_delta_provider(rates)))
        .expect("matching zero-rate provider");

        for epoch in [
            Epoch::from_tai_seconds(0.0),
            Epoch::from_tai_seconds(5_000.0),
            Epoch::from_tai_seconds(20_000.0),
        ] {
            let position = [7_000_000.0, -1_200_000.0, 2_000_000.0];
            let static_acceleration = static_model
                .cartesian_acceleration(epoch, &state(position))
                .expect("static acceleration")
                .value()
                .to_metres_per_second_squared();
            let composed_acceleration = time_varying
                .cartesian_acceleration(epoch, &state(position))
                .expect("zero-delta acceleration")
                .value()
                .to_metres_per_second_squared();
            assert_eq!(
                static_acceleration.map(f64::to_bits),
                composed_acceleration.map(f64::to_bits)
            );
        }
    }

    #[test]
    fn linear_secular_deltas_match_a_static_sum_and_independent_gradient() {
        let degree = 2;
        let order = 1;
        let static_coefficients = synthetic_coefficients(degree, order);
        let static_model = model(
            degree,
            order,
            static_coefficients.clone(),
            CoefficientEpochSemantics::TimeInvariant,
            0.0,
        );
        let rates = vec![
            vec![coefficient_rate(0.0, 0.0)],
            vec![coefficient_rate(0.0, 0.0), coefficient_rate(0.0, 0.0)],
            vec![
                coefficient_rate(2.0e-12, 0.0),
                coefficient_rate(3.0e-12, -4.0e-12),
            ],
        ];
        let provider = linear_delta_provider(rates);
        let epoch = Epoch::from_tai_seconds(10_000.0);
        let expected_coefficients = static_coefficients
            .iter()
            .enumerate()
            .map(|(n, row)| {
                row.iter()
                    .enumerate()
                    .map(|(m, coefficient)| {
                        let delta = provider
                            .coefficient_delta(epoch, n as u32, m as u32)
                            .expect("rate coverage");
                        HarmonicCoefficient {
                            cosine: coefficient.cosine + delta.cosine,
                            sine: coefficient.sine + delta.sine,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let time_varying = static_model
            .with_coefficient_deltas(Arc::new(provider))
            .expect("matching rate provider");
        let summed_static = model(
            degree,
            order,
            expected_coefficients.clone(),
            CoefficientEpochSemantics::TimeInvariant,
            0.0,
        );
        let position = [7_000_000.0, -1_200_000.0, 2_000_000.0];
        let varying_acceleration = time_varying
            .cartesian_acceleration(epoch, &state(position))
            .expect("time-varying acceleration")
            .value()
            .to_metres_per_second_squared();
        let summed_acceleration = summed_static
            .cartesian_acceleration(epoch, &state(position))
            .expect("summed static acceleration")
            .value()
            .to_metres_per_second_squared();
        assert_eq!(
            varying_acceleration.map(f64::to_bits),
            summed_acceleration.map(f64::to_bits)
        );

        let expected = independent_gradient(position, degree, order, &expected_coefficients);
        for axis in 0..3 {
            let tolerance = 2.0e-9_f64.max(expected[axis].abs() * 2.0e-9);
            assert!(
                (varying_acceleration[axis] - expected[axis]).abs() <= tolerance,
                "axis={axis}, actual={}, expected={}",
                varying_acceleration[axis],
                expected[axis]
            );
        }
    }

    #[test]
    fn delta_metadata_mismatches_and_coverage_are_typed() {
        let build = |normalization, tide_system, frame, degree, order| {
            let metadata = delta_metadata(degree, order, normalization, tide_system, frame);
            model(
                2,
                1,
                synthetic_coefficients(2, 1),
                CoefficientEpochSemantics::TimeInvariant,
                0.0,
            )
            .with_coefficient_deltas(Arc::new(FixedDeltas {
                metadata,
                value: Some(HarmonicCoefficient {
                    cosine: 0.0,
                    sine: 0.0,
                }),
            }))
        };

        assert!(matches!(
            build(
                CoefficientNormalization::Unnormalized,
                TideSystem::TideFree,
                ReferenceFrame::ITRF2020,
                2,
                1
            ),
            Err(ConstructionError::DeltaNormalizationMismatch)
        ));
        assert!(matches!(
            build(
                CoefficientNormalization::FullyNormalized4Pi,
                TideSystem::MeanTide,
                ReferenceFrame::ITRF2020,
                2,
                1
            ),
            Err(ConstructionError::DeltaTideSystemMismatch)
        ));
        assert!(matches!(
            build(
                CoefficientNormalization::FullyNormalized4Pi,
                TideSystem::TideFree,
                ReferenceFrame::GCRF,
                2,
                1
            ),
            Err(ConstructionError::DeltaCoefficientFrameMismatch)
        ));
        assert!(matches!(
            build(
                CoefficientNormalization::FullyNormalized4Pi,
                TideSystem::TideFree,
                ReferenceFrame::ITRF2020,
                1,
                0
            ),
            Err(ConstructionError::InsufficientDeltaCoverage)
        ));

        let mut invalid_coverage = delta_metadata(
            2,
            1,
            CoefficientNormalization::FullyNormalized4Pi,
            TideSystem::TideFree,
            ReferenceFrame::ITRF2020,
        );
        invalid_coverage.coverage.start = Epoch::from_tai_seconds(10.0);
        invalid_coverage.coverage.end = Epoch::from_tai_seconds(5.0);
        assert!(matches!(
            model(
                2,
                1,
                synthetic_coefficients(2, 1),
                CoefficientEpochSemantics::TimeInvariant,
                0.0,
            )
            .with_coefficient_deltas(Arc::new(FixedDeltas {
                metadata: invalid_coverage,
                value: None,
            })),
            Err(ConstructionError::InvalidDeltaCoverage)
        ));
    }

    #[test]
    fn linear_rate_provider_rejects_incomplete_invalid_and_out_of_coverage_rates() {
        let metadata = delta_metadata(
            0,
            0,
            CoefficientNormalization::FullyNormalized4Pi,
            TideSystem::TideFree,
            ReferenceFrame::ITRF2020,
        );
        assert!(matches!(
            LinearSecularRateProvider::new(metadata, Epoch::from_tai_seconds(0.0), Vec::new()),
            Err(ConstructionError::MissingCoefficientRate {
                degree: 0,
                order: 0
            })
        ));

        let metadata = delta_metadata(
            0,
            0,
            CoefficientNormalization::FullyNormalized4Pi,
            TideSystem::TideFree,
            ReferenceFrame::ITRF2020,
        );
        assert!(matches!(
            LinearSecularRateProvider::new(
                metadata,
                Epoch::from_tai_seconds(0.0),
                vec![vec![coefficient_rate(1.0e-12, 0.0)]]
            ),
            Err(ConstructionError::NonZeroCentralCoefficientRate)
        ));

        let metadata = delta_metadata(
            0,
            0,
            CoefficientNormalization::FullyNormalized4Pi,
            TideSystem::TideFree,
            ReferenceFrame::ITRF2020,
        );
        assert!(matches!(
            LinearSecularRateProvider::new(
                metadata,
                Epoch::from_tai_seconds(0.0),
                vec![vec![coefficient_rate(f64::INFINITY, 0.0)]]
            ),
            Err(ConstructionError::NonFiniteCoefficientRate {
                degree: 0,
                order: 0
            })
        ));

        let mut metadata = delta_metadata(
            0,
            0,
            CoefficientNormalization::FullyNormalized4Pi,
            TideSystem::TideFree,
            ReferenceFrame::ITRF2020,
        );
        metadata.coverage.end = Epoch::from_tai_seconds(10.0);
        assert!(matches!(
            LinearSecularRateProvider::new(
                metadata,
                Epoch::from_tai_seconds(10.1),
                vec![vec![coefficient_rate(0.0, 0.0)]]
            ),
            Err(ConstructionError::DeltaReferenceEpochOutOfRange)
        ));

        let mut metadata = delta_metadata(
            0,
            0,
            CoefficientNormalization::FullyNormalized4Pi,
            TideSystem::TideFree,
            ReferenceFrame::ITRF2020,
        );
        metadata.source.authority.clear();
        assert!(matches!(
            LinearSecularRateProvider::new(
                metadata,
                Epoch::from_tai_seconds(0.0),
                vec![vec![coefficient_rate(0.0, 0.0)]]
            ),
            Err(ConstructionError::InvalidProvenance {
                provider: ProvenanceProvider::CoefficientDeltas,
                field: ProvenanceField::Authority,
                ..
            })
        ));
    }

    #[test]
    fn delta_evaluation_rejects_coverage_missing_nonfinite_and_central_changes() {
        let make_model = |metadata, value| {
            model(
                0,
                0,
                synthetic_coefficients(0, 0),
                CoefficientEpochSemantics::TimeInvariant,
                0.0,
            )
            .with_coefficient_deltas(Arc::new(FixedDeltas { metadata, value }))
            .expect("valid delta model")
        };
        let mut metadata = delta_metadata(
            0,
            0,
            CoefficientNormalization::FullyNormalized4Pi,
            TideSystem::TideFree,
            ReferenceFrame::ITRF2020,
        );
        metadata.coverage.start = Epoch::from_tai_seconds(2.0);
        metadata.coverage.end = Epoch::from_tai_seconds(10.0);
        let position = state([7.0e6, 0.0, 0.0]);

        let outside = make_model(metadata, None)
            .cartesian_acceleration(Epoch::from_tai_seconds(1.0), &position)
            .expect_err("outside delta coverage");
        assert!(std::error::Error::source(&outside)
            .and_then(|source| source.downcast_ref::<EvaluationError>())
            .is_some_and(|source| matches!(
                source,
                EvaluationError::CoefficientDeltaEpochOutOfRange { .. }
            )));

        let mut metadata = delta_metadata(
            0,
            0,
            CoefficientNormalization::FullyNormalized4Pi,
            TideSystem::TideFree,
            ReferenceFrame::ITRF2020,
        );
        metadata.coverage.end = Epoch::from_tai_seconds(10.0);
        let missing = make_model(metadata, None)
            .cartesian_acceleration(Epoch::from_tai_seconds(1.0), &position)
            .expect_err("missing delta");
        assert!(std::error::Error::source(&missing)
            .and_then(|source| source.downcast_ref::<EvaluationError>())
            .is_some_and(|source| matches!(
                source,
                EvaluationError::MissingCoefficientDelta {
                    degree: 0,
                    order: 0
                }
            )));

        for (coefficient, expected) in [
            (
                HarmonicCoefficient {
                    cosine: f64::NAN,
                    sine: 0.0,
                },
                "non-finite",
            ),
            (
                HarmonicCoefficient {
                    cosine: 1.0e-6,
                    sine: 0.0,
                },
                "central",
            ),
        ] {
            let metadata = delta_metadata(
                0,
                0,
                CoefficientNormalization::FullyNormalized4Pi,
                TideSystem::TideFree,
                ReferenceFrame::ITRF2020,
            );
            let error = make_model(metadata, Some(coefficient))
                .cartesian_acceleration(Epoch::from_tai_seconds(1.0), &position)
                .expect_err("invalid coefficient delta");
            let cause = std::error::Error::source(&error)
                .and_then(|source| source.downcast_ref::<EvaluationError>())
                .expect("typed evaluation error");
            match expected {
                "non-finite" => {
                    assert!(matches!(
                        cause,
                        EvaluationError::NonFiniteCoefficientDelta { .. }
                    ))
                }
                "central" => assert!(matches!(
                    cause,
                    EvaluationError::NonZeroCentralCoefficientDelta
                )),
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn rejects_caller_records_that_override_native_transform_provenance() {
        let provider = Arc::new(ProvenancedRotationProvider {
            reference_data: sourced_transform(0.0).reference_data,
            omit_response_provenance: false,
        });
        let mut records: Vec<_> = provider
            .reference_data()
            .iter()
            .map(clone_reference_data)
            .collect();
        records[0].revision = "different-data-release".into();
        assert!(matches!(
            SourcedBodyFixedTransformProvider::new(provider, records),
            Err(ConstructionError::TransformProvenanceMismatch)
        ));
    }

    #[test]
    fn evaluation_retains_native_provenance_and_rejects_missing_response_records() {
        for omit_response_provenance in [false, true] {
            let provider = Arc::new(ProvenancedRotationProvider {
                reference_data: sourced_transform(0.0).reference_data,
                omit_response_provenance,
            });
            let records = provider
                .reference_data()
                .iter()
                .map(clone_reference_data)
                .collect();
            let mut model = model(
                0,
                0,
                synthetic_coefficients(0, 0),
                CoefficientEpochSemantics::TimeInvariant,
                0.0,
            );
            model.transform_provider =
                SourcedBodyFixedTransformProvider::new(provider.clone(), records)
                    .expect("matching native provenance");
            assert_eq!(model.transform_reference_data(), provider.reference_data());
            let result = model
                .cartesian_acceleration(Epoch::from_tai_seconds(0.0), &state([7.0e6, 0.0, 0.0]));
            if omit_response_provenance {
                let error = result.expect_err("provider omitted its native records");
                assert!(std::error::Error::source(&error)
                    .and_then(|source| source.downcast_ref::<EvaluationError>())
                    .is_some_and(|source| matches!(
                        source,
                        EvaluationError::TransformProvenanceMismatch
                    )));
            } else {
                assert_eq!(
                    result.expect("consistent transform provenance").frame(),
                    ReferenceFrame::GCRF
                );
            }
        }
    }

    #[test]
    fn reports_selected_source_normalization_tide_and_transform_records() {
        let model = model(
            0,
            0,
            synthetic_coefficients(0, 0),
            CoefficientEpochSemantics::TimeInvariant,
            0.0,
        );
        assert_eq!(
            model.metadata().normalization,
            CoefficientNormalization::FullyNormalized4Pi
        );
        assert_eq!(model.metadata().tide_system, TideSystem::TideFree);
        assert_eq!(model.metadata().source.revision, "test-1");
        assert_eq!(model.transform_reference_data()[0].revision, "rotation-1");
    }

    #[test]
    fn bounded_coefficients_accept_both_endpoints_and_reject_extrapolation() {
        let start = Epoch::from_gregorian_utc_at_midnight(2025, 1, 1);
        let end = Epoch::from_gregorian_utc_at_midnight(2025, 1, 2);
        let model = model(
            0,
            0,
            synthetic_coefficients(0, 0),
            CoefficientEpochSemantics::ValidBetween {
                start,
                end,
                time_scale: TimeScale::UTC,
            },
            0.0,
        );
        for epoch in [start, end] {
            model
                .cartesian_acceleration(epoch, &state([7.0e6, 0.0, 0.0]))
                .expect("inclusive validity endpoint");
        }
        let error = model
            .cartesian_acceleration(
                end + hifitime::Duration::from_seconds(0.1),
                &state([7.0e6, 0.0, 0.0]),
            )
            .expect_err("outside bounded coefficient validity");
        assert!(std::error::Error::source(&error)
            .and_then(|source| source.downcast_ref::<EvaluationError>())
            .is_some_and(|source| matches!(
                source,
                EvaluationError::CoefficientEpochOutOfRange { .. }
            )));
    }

    #[test]
    fn rejects_wrong_transform_request_with_typed_failure() {
        let coefficient_provider = Coefficients {
            metadata: metadata(
                0,
                0,
                ReferenceFrame::ITRF2020,
                CoefficientEpochSemantics::TimeInvariant,
            ),
            values: synthetic_coefficients(0, 0),
        };
        let wrong_transform = SourcedBodyFixedTransformProvider::new(
            Arc::new(RotationProvider {
                angular_rate: 0.0,
                bad_request: true,
            }),
            vec![ReferenceDataDescriptor {
                authority: "test".into(),
                product: "bad transform".into(),
                revision: "1".into(),
                checksum: None,
            }],
        )
        .expect("provenance");
        let model = SphericalHarmonicGravityModel::new(
            SphericalHarmonicField {
                gravitational_parameter: GravitationalParameter::try_from(1.0).expect("μ"),
                reference_radius: Length::new::<meter>(1.0),
                origin: ReferenceFrame::GCRF.origin(),
                inertial_frame: InertialFrame::GCRF,
                body_fixed_frame: ReferenceFrame::ITRF2020,
                time_scale: TimeScale::TAI,
                maximum_degree: 0,
                maximum_order: 0,
            },
            Arc::new(coefficient_provider),
            wrong_transform,
        )
        .expect("model");
        let error = model
            .cartesian_acceleration(Epoch::from_tai_seconds(0.0), &state([2.0, 0.0, 0.0]))
            .expect_err("mismatched response");
        assert!(std::error::Error::source(&error)
            .and_then(|source| source.downcast_ref::<EvaluationError>())
            .is_some_and(|source| matches!(source, EvaluationError::TransformResponseMismatch)));
    }

    #[test]
    fn construction_rejects_metadata_frame_coverage_and_missing_coefficients() {
        let field = |degree, order| SphericalHarmonicField {
            gravitational_parameter: GravitationalParameter::try_from(1.0).expect("μ"),
            reference_radius: Length::new::<meter>(1.0),
            origin: ReferenceFrame::GCRF.origin(),
            inertial_frame: InertialFrame::GCRF,
            body_fixed_frame: ReferenceFrame::ITRF2020,
            time_scale: TimeScale::TAI,
            maximum_degree: degree,
            maximum_order: order,
        };
        let transform = || sourced_transform(0.0);
        let provider = |degree, order, frame, values| Coefficients {
            metadata: metadata(
                degree,
                order,
                frame,
                CoefficientEpochSemantics::TimeInvariant,
            ),
            values,
        };

        let wrong_frame = provider(0, 0, ReferenceFrame::GCRF, synthetic_coefficients(0, 0));
        assert!(matches!(
            SphericalHarmonicGravityModel::new(field(0, 0), Arc::new(wrong_frame), transform()),
            Err(ConstructionError::CoefficientFrameMismatch { .. })
        ));

        let insufficient = provider(1, 0, ReferenceFrame::ITRF2020, synthetic_coefficients(1, 0));
        assert!(matches!(
            SphericalHarmonicGravityModel::new(field(2, 0), Arc::new(insufficient), transform()),
            Err(ConstructionError::InsufficientCoverage)
        ));

        let mut missing_values = synthetic_coefficients(2, 1);
        missing_values[2].pop();
        let missing = provider(2, 1, ReferenceFrame::ITRF2020, missing_values);
        assert!(matches!(
            SphericalHarmonicGravityModel::new(field(2, 1), Arc::new(missing), transform()),
            Err(ConstructionError::MissingCoefficient {
                degree: 2,
                order: 1
            })
        ));
    }

    #[test]
    fn reports_typed_input_frame_origin_and_singular_position_failures() {
        let model = model(
            0,
            0,
            synthetic_coefficients(0, 0),
            CoefficientEpochSemantics::TimeInvariant,
            0.0,
        );
        let at = Epoch::from_tai_seconds(0.0);
        let mismatched_frame = CartesianState::new(
            ReferenceFrame::ITRF2020,
            Position::from_metres(7.0e6, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
        )
        .expect("finite state");
        let wrong_origin = CartesianState::new(
            ReferenceFrame::ICRF,
            Position::from_metres(7.0e6, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
        )
        .expect("finite state");
        let singular = state([0.0, 0.0, 0.0]);

        for (candidate, expected) in [
            (&mismatched_frame, "frame"),
            (&wrong_origin, "origin"),
            (&singular, "position"),
        ] {
            let error = model
                .cartesian_acceleration(at, candidate)
                .expect_err("invalid state");
            let cause = std::error::Error::source(&error)
                .and_then(|source| source.downcast_ref::<EvaluationError>())
                .expect("typed evaluator source");
            match expected {
                "frame" => assert!(matches!(cause, EvaluationError::FrameMismatch)),
                "origin" => assert!(matches!(cause, EvaluationError::OriginMismatch)),
                "position" => assert!(matches!(cause, EvaluationError::SingularPosition)),
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn rejects_missing_or_blank_provenance_and_unsupported_normalization() {
        assert!(matches!(
            SourcedBodyFixedTransformProvider::new(
                Arc::new(RotationProvider {
                    angular_rate: 0.0,
                    bad_request: false
                }),
                Vec::new()
            ),
            Err(ConstructionError::MissingTransformProvenance)
        ));

        let mut invalid = metadata(
            0,
            0,
            ReferenceFrame::ITRF2020,
            CoefficientEpochSemantics::TimeInvariant,
        );
        invalid.source.revision.clear();
        let provider = Coefficients {
            metadata: invalid,
            values: synthetic_coefficients(0, 0),
        };
        assert!(matches!(
            SphericalHarmonicGravityModel::new(
                SphericalHarmonicField {
                    gravitational_parameter: GravitationalParameter::try_from(1.0).expect("μ"),
                    reference_radius: Length::new::<meter>(1.0),
                    origin: ReferenceFrame::GCRF.origin(),
                    inertial_frame: InertialFrame::GCRF,
                    body_fixed_frame: ReferenceFrame::ITRF2020,
                    time_scale: TimeScale::TAI,
                    maximum_degree: 0,
                    maximum_order: 0,
                },
                Arc::new(provider),
                sourced_transform(0.0)
            ),
            Err(ConstructionError::InvalidProvenance {
                provider: ProvenanceProvider::GravityCoefficients,
                field: ProvenanceField::Revision,
                ..
            })
        ));

        let mut unsupported = metadata(
            0,
            0,
            ReferenceFrame::ITRF2020,
            CoefficientEpochSemantics::TimeInvariant,
        );
        unsupported.normalization = CoefficientNormalization::Unnormalized;
        let provider = Coefficients {
            metadata: unsupported,
            values: synthetic_coefficients(0, 0),
        };
        assert!(matches!(
            SphericalHarmonicGravityModel::new(
                SphericalHarmonicField {
                    gravitational_parameter: GravitationalParameter::try_from(1.0).expect("μ"),
                    reference_radius: Length::new::<meter>(1.0),
                    origin: ReferenceFrame::GCRF.origin(),
                    inertial_frame: InertialFrame::GCRF,
                    body_fixed_frame: ReferenceFrame::ITRF2020,
                    time_scale: TimeScale::TAI,
                    maximum_degree: 0,
                    maximum_order: 0,
                },
                Arc::new(provider),
                sourced_transform(0.0)
            ),
            Err(ConstructionError::UnsupportedNormalization)
        ));
    }
}
