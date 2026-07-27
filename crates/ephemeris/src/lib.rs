#![forbid(unsafe_code)]

//! Caller-selected physical ephemeris providers.
//!
//! An [`EphemerisQuery`] names the target, observer, complete reference frame,
//! and epoch. Providers return finite, typed [`EphemerisState`] values and
//! expose the exact scientific-data artifacts on which they depend.
//!
//! [`CubicHermiteEphemeris`] is a small concrete provider for already-decoded
//! position/velocity samples. It authenticates the caller-selected source
//! bytes through [`VerifiedArtifact`], but deliberately does not parse SPK or
//! another operational format.
//!
//! ```
//! use bodies::Body;
//! use ephemeris::{
//!     CubicHermiteEphemeris, EphemerisProvider, EphemerisQuery, EphemerisSample,
//! };
//! use frames::{FrameOrientation, FrameOrigin, ReferenceFrame};
//! use hifitime::{Duration, Epoch};
//! use orskit_data::{
//!     ArtifactCoverage, ArtifactDescriptor, Sha256Digest, TimeCoverage, VerifiedArtifact,
//! };
//! use units::{Position, VelocityVector};
//!
//! let start = Epoch::from_tai_seconds(0.0);
//! let end = start + Duration::from_seconds(10.0);
//! let bytes = b"application-owned ephemeris sample table".to_vec();
//! let descriptor = ArtifactDescriptor::new(
//!     "example authority",
//!     "example states",
//!     "v1",
//!     Sha256Digest::compute(&bytes),
//!     ArtifactCoverage::Interval(TimeCoverage::new(start, end)?),
//! )?;
//! let artifact = VerifiedArtifact::from_bytes(descriptor, bytes)?;
//! let frame = ReferenceFrame::new(
//!     FrameOrigin::Body(Body::EARTH),
//!     FrameOrientation::Icrf,
//! );
//! let samples = vec![
//!     EphemerisSample::new(
//!         start,
//!         Position::from_metres(0.0, 0.0, 0.0),
//!         VelocityVector::from_metres_per_second(1.0, 0.0, 0.0),
//!     )?,
//!     EphemerisSample::new(
//!         end,
//!         Position::from_metres(10.0, 0.0, 0.0),
//!         VelocityVector::from_metres_per_second(1.0, 0.0, 0.0),
//!     )?,
//! ];
//! let provider =
//!     CubicHermiteEphemeris::new(artifact, Body::MOON, Body::EARTH, frame, samples)?;
//! let query = EphemerisQuery::new(
//!     Body::MOON,
//!     Body::EARTH,
//!     frame,
//!     start + Duration::from_seconds(5.0),
//! )?;
//!
//! let state = provider.state(query)?;
//! assert_eq!(state.position(), Position::from_metres(5.0, 0.0, 0.0));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::{error::Error, slice};

use bodies::Body;
use frames::{FrameOrigin, ReferenceFrame};
use hifitime::Epoch;
use orskit_data::CoverageError;
pub use orskit_data::{ArtifactDescriptor, VerifiedArtifact};
use thiserror::Error;
use units::{Position, VelocityVector};

/// Complete request for one geometric Cartesian state.
///
/// `frame` names both the axes and origin. Its origin must be the selected
/// observer body, so the returned position and velocity are unambiguous.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EphemerisQuery {
    target: Body,
    observer: Body,
    frame: ReferenceFrame,
    epoch: Epoch,
}

impl EphemerisQuery {
    /// Constructs an explicit target-relative-to-observer state request.
    pub fn new(
        target: Body,
        observer: Body,
        frame: ReferenceFrame,
        epoch: Epoch,
    ) -> Result<Self, EphemerisQueryError> {
        let actual_origin = frame.origin();
        let expected_origin = FrameOrigin::Body(observer);
        if actual_origin != expected_origin {
            return Err(EphemerisQueryError::FrameOriginMismatch {
                observer,
                expected_origin,
                actual_origin,
            });
        }
        Ok(Self {
            target,
            observer,
            frame,
            epoch,
        })
    }

    /// Body whose state is requested.
    #[must_use]
    pub const fn target(self) -> Body {
        self.target
    }

    /// Body relative to which the state is requested.
    #[must_use]
    pub const fn observer(self) -> Body {
        self.observer
    }

    /// Complete origin-and-axes frame of the requested state.
    #[must_use]
    pub const fn frame(self) -> ReferenceFrame {
        self.frame
    }

    /// Instant at which the state is requested.
    #[must_use]
    pub const fn epoch(self) -> Epoch {
        self.epoch
    }
}

/// Invalid ephemeris query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum EphemerisQueryError {
    /// The frame origin does not represent the selected observer.
    #[error(
        "ephemeris observer {observer} requires frame origin {expected_origin}, got {actual_origin}"
    )]
    FrameOriginMismatch {
        /// Selected observer body.
        observer: Body,
        /// Required frame origin.
        expected_origin: FrameOrigin,
        /// Supplied frame origin.
        actual_origin: FrameOrigin,
    },
}

/// Finite geometric Cartesian position and velocity for one query.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EphemerisState {
    query: EphemerisQuery,
    position: Position,
    velocity: VelocityVector,
}

impl EphemerisState {
    /// Constructs a provider result after checking every physical component.
    pub fn new(
        query: EphemerisQuery,
        position: Position,
        velocity: VelocityVector,
    ) -> Result<Self, EphemerisStateError> {
        validate_state_values(position, velocity)?;
        Ok(Self {
            query,
            position,
            velocity,
        })
    }

    /// Exact query to which this state responds.
    #[must_use]
    pub const fn query(self) -> EphemerisQuery {
        self.query
    }

    /// Target position relative to the observer, in the query frame.
    #[must_use]
    pub const fn position(self) -> Position {
        self.position
    }

    /// Target velocity relative to the observer, in the query frame.
    #[must_use]
    pub const fn velocity(self) -> VelocityVector {
        self.velocity
    }
}

/// Invalid physical state returned or supplied at an ephemeris boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum EphemerisStateError {
    /// At least one position component is NaN or infinite.
    #[error("ephemeris position contains a non-finite component")]
    NonFinitePosition,
    /// At least one velocity component is NaN or infinite.
    #[error("ephemeris velocity contains a non-finite component")]
    NonFiniteVelocity,
}

/// Open contract for caller-selected geometric physical ephemerides.
///
/// Implementations own their interpolation, data decoding, and caching policy.
/// They perform no implicit network access and expose all verified artifacts
/// used for a result. The associated error preserves implementation-specific
/// coverage and interpolation detail.
pub trait EphemerisProvider: Send + Sync {
    /// Provider-specific typed failure.
    type Error: Error + Send + Sync + 'static;

    /// Verified artifacts required by this provider.
    fn reference_data(&self) -> &[ArtifactDescriptor];

    /// Evaluates one complete query.
    fn state(&self, query: EphemerisQuery) -> Result<EphemerisState, Self::Error>;
}

/// One finite position/velocity sample at an absolute instant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EphemerisSample {
    epoch: Epoch,
    position: Position,
    velocity: VelocityVector,
}

impl EphemerisSample {
    /// Constructs a finite sample.
    pub fn new(
        epoch: Epoch,
        position: Position,
        velocity: VelocityVector,
    ) -> Result<Self, EphemerisStateError> {
        validate_state_values(position, velocity)?;
        Ok(Self {
            epoch,
            position,
            velocity,
        })
    }

    /// Sample instant.
    #[must_use]
    pub const fn epoch(self) -> Epoch {
        self.epoch
    }

    /// Sample position in the provider's declared frame.
    #[must_use]
    pub const fn position(self) -> Position {
        self.position
    }

    /// Sample velocity in the provider's declared frame.
    #[must_use]
    pub const fn velocity(self) -> VelocityVector {
        self.velocity
    }
}

fn validate_state_values(
    position: Position,
    velocity: VelocityVector,
) -> Result<(), EphemerisStateError> {
    if !position.is_finite() {
        return Err(EphemerisStateError::NonFinitePosition);
    }
    if !velocity.is_finite() {
        return Err(EphemerisStateError::NonFiniteVelocity);
    }
    Ok(())
}

/// Piecewise cubic-Hermite interpolation over caller-decoded state samples.
///
/// Each adjacent pair supplies position and its time derivative at both ends.
/// Position uses the cubic Hermite basis and velocity is the derivative of
/// that same polynomial, following the state semantics documented for NAIF
/// SPK type 13. Samples may be unequally spaced.
#[derive(Debug)]
pub struct CubicHermiteEphemeris {
    artifact: VerifiedArtifact,
    target: Body,
    observer: Body,
    frame: ReferenceFrame,
    samples: Box<[EphemerisSample]>,
}

impl CubicHermiteEphemeris {
    /// Builds a provider from an authenticated source artifact and decoded samples.
    ///
    /// The caller is responsible for decoding the verified bytes into `samples`;
    /// this crate intentionally owns no operational file reader. At least two
    /// samples are required, epochs must increase strictly, every sample must
    /// lie within the artifact's declared coverage, and `frame` must be centered
    /// on `observer`.
    pub fn new(
        artifact: VerifiedArtifact,
        target: Body,
        observer: Body,
        frame: ReferenceFrame,
        samples: Vec<EphemerisSample>,
    ) -> Result<Self, CubicHermiteEphemerisError> {
        if frame.origin() != FrameOrigin::Body(observer) {
            return Err(CubicHermiteEphemerisError::FrameOriginMismatch {
                observer,
                actual_origin: frame.origin(),
            });
        }
        if samples.len() < 2 {
            return Err(CubicHermiteEphemerisError::TooFewSamples {
                actual: samples.len(),
            });
        }
        for (index, sample) in samples.iter().enumerate() {
            artifact
                .descriptor()
                .coverage()
                .require(sample.epoch)
                .map_err(
                    |source| CubicHermiteEphemerisError::SampleOutsideArtifactCoverage {
                        index,
                        source,
                    },
                )?;
        }
        for (index, pair) in samples.windows(2).enumerate() {
            if pair[0].epoch >= pair[1].epoch {
                return Err(CubicHermiteEphemerisError::SamplesNotStrictlyIncreasing {
                    earlier_index: index,
                    later_index: index + 1,
                });
            }
        }
        Ok(Self {
            artifact,
            target,
            observer,
            frame,
            samples: samples.into_boxed_slice(),
        })
    }

    /// Authenticated source bytes and their identity, digest, and coverage.
    #[must_use]
    pub const fn artifact(&self) -> &VerifiedArtifact {
        &self.artifact
    }

    /// Target represented by this sample set.
    #[must_use]
    pub const fn target(&self) -> Body {
        self.target
    }

    /// Observer relative to which the samples are expressed.
    #[must_use]
    pub const fn observer(&self) -> Body {
        self.observer
    }

    /// Complete observer-centered frame used by the samples.
    #[must_use]
    pub const fn frame(&self) -> ReferenceFrame {
        self.frame
    }

    /// Samples used by the interpolation provider.
    #[must_use]
    pub const fn samples(&self) -> &[EphemerisSample] {
        &self.samples
    }
}

impl EphemerisProvider for CubicHermiteEphemeris {
    type Error = CubicHermiteEphemerisError;

    fn reference_data(&self) -> &[ArtifactDescriptor] {
        slice::from_ref(self.artifact.descriptor())
    }

    fn state(&self, query: EphemerisQuery) -> Result<EphemerisState, Self::Error> {
        if query.target != self.target || query.observer != self.observer {
            return Err(CubicHermiteEphemerisError::UnsupportedPath {
                requested_target: query.target,
                requested_observer: query.observer,
                available_target: self.target,
                available_observer: self.observer,
            });
        }
        if query.frame != self.frame {
            return Err(CubicHermiteEphemerisError::UnsupportedFrame {
                requested: query.frame,
            });
        }
        self.artifact
            .descriptor()
            .coverage()
            .require(query.epoch)
            .map_err(|source| CubicHermiteEphemerisError::ArtifactCoverage { source })?;

        let start = self.samples[0].epoch;
        let end = self.samples[self.samples.len() - 1].epoch;
        if query.epoch < start || query.epoch > end {
            return Err(CubicHermiteEphemerisError::OutsideInterpolationCoverage {
                epoch: query.epoch,
                start,
                end,
            });
        }

        let right = self
            .samples
            .partition_point(|sample| sample.epoch <= query.epoch);
        if right > 0 && self.samples[right - 1].epoch == query.epoch {
            let sample = self.samples[right - 1];
            return EphemerisState::new(query, sample.position, sample.velocity)
                .map_err(|source| CubicHermiteEphemerisError::NonFiniteResult { source });
        }
        let left_sample = self.samples[right - 1];
        let right_sample = self.samples[right];
        interpolate(query, left_sample, right_sample)
    }
}

fn interpolate(
    query: EphemerisQuery,
    left: EphemerisSample,
    right: EphemerisSample,
) -> Result<EphemerisState, CubicHermiteEphemerisError> {
    let interval_seconds = (right.epoch - left.epoch).to_seconds();
    let tau = (query.epoch - left.epoch).to_seconds() / interval_seconds;
    let tau_squared = tau * tau;
    let tau_cubed = tau_squared * tau;
    let h00 = 2.0 * tau_cubed - 3.0 * tau_squared + 1.0;
    let h10 = tau_cubed - 2.0 * tau_squared + tau;
    let h01 = -2.0 * tau_cubed + 3.0 * tau_squared;
    let h11 = tau_cubed - tau_squared;
    let dh00_dt = (6.0 * tau_squared - 6.0 * tau) / interval_seconds;
    let dh10_dt = 3.0 * tau_squared - 4.0 * tau + 1.0;
    let dh01_dt = (-6.0 * tau_squared + 6.0 * tau) / interval_seconds;
    let dh11_dt = 3.0 * tau_squared - 2.0 * tau;

    let left_position = left.position.to_metres();
    let right_position = right.position.to_metres();
    let left_velocity = left.velocity.to_metres_per_second();
    let right_velocity = right.velocity.to_metres_per_second();
    let mut position = [0.0; 3];
    let mut velocity = [0.0; 3];
    for axis in 0..3 {
        position[axis] = h00 * left_position[axis]
            + h10 * interval_seconds * left_velocity[axis]
            + h01 * right_position[axis]
            + h11 * interval_seconds * right_velocity[axis];
        velocity[axis] = dh00_dt * left_position[axis]
            + dh10_dt * left_velocity[axis]
            + dh01_dt * right_position[axis]
            + dh11_dt * right_velocity[axis];
    }

    EphemerisState::new(
        query,
        Position::from_metres(position[0], position[1], position[2]),
        VelocityVector::from_metres_per_second(velocity[0], velocity[1], velocity[2]),
    )
    .map_err(|source| CubicHermiteEphemerisError::NonFiniteResult { source })
}

/// Construction or evaluation failure for [`CubicHermiteEphemeris`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CubicHermiteEphemerisError {
    /// The provider frame is not centered on its declared observer.
    #[error("ephemeris frame origin {actual_origin} does not represent observer {observer}")]
    FrameOriginMismatch {
        /// Declared observer.
        observer: Body,
        /// Supplied frame origin.
        actual_origin: FrameOrigin,
    },
    /// Cubic Hermite interpolation requires two endpoints.
    #[error("cubic Hermite ephemeris requires at least two samples, got {actual}")]
    TooFewSamples {
        /// Supplied sample count.
        actual: usize,
    },
    /// A decoded sample lies outside the source artifact's declared coverage.
    #[error("ephemeris sample {index} lies outside artifact coverage")]
    SampleOutsideArtifactCoverage {
        /// Zero-based sample index.
        index: usize,
        /// Artifact coverage detail.
        #[source]
        source: CoverageError,
    },
    /// Sample epochs contain a duplicate or reversal.
    #[error(
        "ephemeris sample epochs are not strictly increasing between indices {earlier_index} and {later_index}"
    )]
    SamplesNotStrictlyIncreasing {
        /// Earlier sample index.
        earlier_index: usize,
        /// Later sample index.
        later_index: usize,
    },
    /// The provider does not contain the requested target/observer pair.
    #[error(
        "requested ephemeris path {requested_target} relative to {requested_observer}, but provider contains {available_target} relative to {available_observer}"
    )]
    UnsupportedPath {
        /// Requested target.
        requested_target: Body,
        /// Requested observer.
        requested_observer: Body,
        /// Available target.
        available_target: Body,
        /// Available observer.
        available_observer: Body,
    },
    /// The provider's samples use a different frame.
    #[error("requested ephemeris frame {requested} is not available from this provider")]
    UnsupportedFrame {
        /// Requested frame.
        requested: ReferenceFrame,
    },
    /// The source artifact does not declare coverage of the request.
    #[error("ephemeris artifact does not cover the requested epoch")]
    ArtifactCoverage {
        /// Artifact coverage detail.
        #[source]
        source: CoverageError,
    },
    /// The request is not bracketed by samples even though artifact coverage permits it.
    #[error("epoch {epoch} is outside interpolation coverage [{start}, {end}]")]
    OutsideInterpolationCoverage {
        /// Requested instant.
        epoch: Epoch,
        /// First sample instant.
        start: Epoch,
        /// Last sample instant.
        end: Epoch,
    },
    /// Floating-point interpolation produced a non-finite physical result.
    #[error("ephemeris interpolation produced a non-finite state")]
    NonFiniteResult {
        /// Invalid state detail.
        #[source]
        source: EphemerisStateError,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use hifitime::{Duration, TimeScale};
    use orskit_data::{ArtifactCoverage, Sha256Digest, TimeCoverage};
    use std::str::FromStr;

    const HORIZONS_BYTES: &[u8] =
        include_bytes!("../testdata/jpl_horizons_moon_earth_20260101.csv");
    const HORIZONS_DIGEST: &str =
        "746f0193039f2f5affebd80f4718f8e5897a2cc8738513dd0c5fea9b0d5aca8e";

    fn earth_icrf() -> ReferenceFrame {
        ReferenceFrame::new(
            FrameOrigin::Body(Body::EARTH),
            frames::FrameOrientation::Icrf,
        )
    }

    fn horizons_epochs() -> [Epoch; 3] {
        [
            Epoch::from_gregorian_hms(2026, 1, 1, 0, 0, 0, TimeScale::TDB),
            Epoch::from_gregorian_hms(2026, 1, 1, 0, 1, 0, TimeScale::TDB),
            Epoch::from_gregorian_hms(2026, 1, 1, 0, 2, 0, TimeScale::TDB),
        ]
    }

    fn horizons_states() -> [(Position, VelocityVector); 3] {
        [
            (
                Position::from_metres(
                    1.443_257_274_919_273e8,
                    2.895_841_575_449_329e8,
                    1.601_589_230_016_842e8,
                ),
                VelocityVector::from_metres_per_second(
                    -1.004_314_137_672_221e3,
                    3.839_146_101_176_44e2,
                    1.725_348_969_484_014e2,
                ),
            ),
            (
                Position::from_metres(
                    1.442_654_663_936_8e8,
                    2.896_071_879_905_91e8,
                    1.601_692_726_376_739e8,
                ),
                VelocityVector::from_metres_per_second(
                    -1.004_389_132_162_786e3,
                    3.837_669_096_619_949e2,
                    1.724_529_683_808_703e2,
                ),
            ),
            (
                Position::from_metres(
                    1.442_052_007_966_769e8,
                    2.896_302_095_738_334e8,
                    1.601_796_173_577_709e8,
                ),
                VelocityVector::from_metres_per_second(
                    -1.004_464_096_197_29e3,
                    3.836_191_962_574_164e2,
                    1.723_710_338_601_744e2,
                ),
            ),
        ]
    }

    fn artifact(coverage: ArtifactCoverage) -> VerifiedArtifact {
        let digest = Sha256Digest::from_str(HORIZONS_DIGEST).expect("recorded digest");
        assert_eq!(Sha256Digest::compute(HORIZONS_BYTES), digest);
        let descriptor = ArtifactDescriptor::new(
            "NASA/JPL Solar System Dynamics Group",
            "Horizons DE441 Moon (301) relative Earth (399), geometric ICRF vectors",
            "API 1.2 response retrieved 2026-07-24",
            digest,
            coverage,
        )
        .expect("complete fixture identity");
        VerifiedArtifact::from_bytes(descriptor, HORIZONS_BYTES.to_vec()).expect("fixture checksum")
    }

    fn provider(coverage: ArtifactCoverage) -> CubicHermiteEphemeris {
        let epochs = horizons_epochs();
        let states = horizons_states();
        CubicHermiteEphemeris::new(
            artifact(coverage),
            Body::MOON,
            Body::EARTH,
            earth_icrf(),
            vec![
                EphemerisSample::new(epochs[0], states[0].0, states[0].1).expect("finite endpoint"),
                EphemerisSample::new(epochs[2], states[2].0, states[2].1).expect("finite endpoint"),
            ],
        )
        .expect("valid provider")
    }

    #[test]
    fn independently_validated_horizons_midpoint() {
        let epochs = horizons_epochs();
        let expected = horizons_states()[1];
        let coverage =
            ArtifactCoverage::Interval(TimeCoverage::new(epochs[0], epochs[2]).expect("coverage"));
        let provider = provider(coverage);
        let query =
            EphemerisQuery::new(Body::MOON, Body::EARTH, earth_icrf(), epochs[1]).expect("query");

        let actual = provider.state(query).expect("interpolated state");
        assert_eq!(actual.query(), query);
        for (actual, expected) in actual
            .position()
            .to_metres()
            .into_iter()
            .zip(expected.0.to_metres())
        {
            assert!((actual - expected).abs() < 1.0e-3);
        }
        for (actual, expected) in actual
            .velocity()
            .to_metres_per_second()
            .into_iter()
            .zip(expected.1.to_metres_per_second())
        {
            assert!((actual - expected).abs() < 1.0e-6);
        }
        assert_eq!(provider.reference_data().len(), 1);
        assert_eq!(
            provider.reference_data()[0].authority(),
            "NASA/JPL Solar System Dynamics Group"
        );
    }

    #[test]
    fn exact_endpoints_are_preserved() {
        let epochs = horizons_epochs();
        let states = horizons_states();
        let provider = provider(ArtifactCoverage::AllTime);
        for (epoch, expected) in [(epochs[0], states[0]), (epochs[2], states[2])] {
            let query =
                EphemerisQuery::new(Body::MOON, Body::EARTH, earth_icrf(), epoch).expect("query");
            let actual = provider.state(query).expect("endpoint");
            assert_eq!(actual.position(), expected.0);
            assert_eq!(actual.velocity(), expected.1);
        }
    }

    #[test]
    fn query_requires_observer_centered_frame() {
        let result = EphemerisQuery::new(
            Body::MOON,
            Body::EARTH,
            ReferenceFrame::ICRF,
            horizons_epochs()[0],
        );
        assert!(matches!(
            result,
            Err(EphemerisQueryError::FrameOriginMismatch {
                observer: Body::EARTH,
                actual_origin: FrameOrigin::Barycenter(_),
                ..
            })
        ));
    }

    #[test]
    fn non_finite_samples_are_rejected() {
        assert_eq!(
            EphemerisSample::new(
                horizons_epochs()[0],
                Position::from_metres(f64::NAN, 0.0, 0.0),
                VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
            ),
            Err(EphemerisStateError::NonFinitePosition)
        );
    }

    #[test]
    fn construction_rejects_too_few_and_unordered_samples() {
        let epoch = horizons_epochs()[0];
        let state = horizons_states()[0];
        let sample = EphemerisSample::new(epoch, state.0, state.1).expect("sample");
        let empty = CubicHermiteEphemeris::new(
            artifact(ArtifactCoverage::AllTime),
            Body::MOON,
            Body::EARTH,
            earth_icrf(),
            vec![sample],
        );
        assert!(matches!(
            empty,
            Err(CubicHermiteEphemerisError::TooFewSamples { actual: 1 })
        ));

        let unordered = CubicHermiteEphemeris::new(
            artifact(ArtifactCoverage::AllTime),
            Body::MOON,
            Body::EARTH,
            earth_icrf(),
            vec![sample, sample],
        );
        assert!(matches!(
            unordered,
            Err(CubicHermiteEphemerisError::SamplesNotStrictlyIncreasing {
                earlier_index: 0,
                later_index: 1
            })
        ));
    }

    #[test]
    fn construction_rejects_wrong_origin_and_sample_outside_artifact_coverage() {
        let epochs = horizons_epochs();
        let states = horizons_states();
        let samples = vec![
            EphemerisSample::new(epochs[0], states[0].0, states[0].1).expect("sample"),
            EphemerisSample::new(epochs[2], states[2].0, states[2].1).expect("sample"),
        ];
        let wrong_origin = CubicHermiteEphemeris::new(
            artifact(ArtifactCoverage::AllTime),
            Body::MOON,
            Body::EARTH,
            ReferenceFrame::ICRF,
            samples.clone(),
        );
        assert!(matches!(
            wrong_origin,
            Err(CubicHermiteEphemerisError::FrameOriginMismatch { .. })
        ));

        let narrow =
            ArtifactCoverage::Interval(TimeCoverage::new(epochs[0], epochs[1]).expect("coverage"));
        let outside_coverage = CubicHermiteEphemeris::new(
            artifact(narrow),
            Body::MOON,
            Body::EARTH,
            earth_icrf(),
            samples,
        );
        assert!(matches!(
            outside_coverage,
            Err(CubicHermiteEphemerisError::SampleOutsideArtifactCoverage { index: 1, .. })
        ));
    }

    #[test]
    fn artifact_and_interpolation_coverage_failures_are_distinct() {
        let epochs = horizons_epochs();
        let before = epochs[0] - Duration::from_seconds(1.0);
        let after = epochs[2] + Duration::from_seconds(1.0);
        let declared =
            ArtifactCoverage::Interval(TimeCoverage::new(epochs[0], epochs[2]).expect("coverage"));
        let bounded_provider = provider(declared);
        let outside_artifact =
            EphemerisQuery::new(Body::MOON, Body::EARTH, earth_icrf(), before).expect("query");
        assert!(matches!(
            bounded_provider.state(outside_artifact),
            Err(CubicHermiteEphemerisError::ArtifactCoverage { .. })
        ));

        let provider = provider(ArtifactCoverage::AllTime);
        let outside_samples =
            EphemerisQuery::new(Body::MOON, Body::EARTH, earth_icrf(), after).expect("query");
        assert!(matches!(
            provider.state(outside_samples),
            Err(CubicHermiteEphemerisError::OutsideInterpolationCoverage { .. })
        ));
    }

    #[test]
    fn path_and_frame_mismatches_are_typed() {
        let provider = provider(ArtifactCoverage::AllTime);
        let epoch = horizons_epochs()[1];
        let wrong_path =
            EphemerisQuery::new(Body::SUN, Body::EARTH, earth_icrf(), epoch).expect("query");
        assert!(matches!(
            provider.state(wrong_path),
            Err(CubicHermiteEphemerisError::UnsupportedPath { .. })
        ));

        let eme2000 = ReferenceFrame::new(
            FrameOrigin::Body(Body::EARTH),
            frames::FrameOrientation::Eme2000,
        );
        let wrong_frame =
            EphemerisQuery::new(Body::MOON, Body::EARTH, eme2000, epoch).expect("query");
        assert!(matches!(
            provider.state(wrong_frame),
            Err(CubicHermiteEphemerisError::UnsupportedFrame { .. })
        ));
    }
}
