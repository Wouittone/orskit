#![forbid(unsafe_code)]

//! Caller-owned Earth-orientation data for the [`frames`] transform contract.
//!
//! [`Iau2000EraProvider`] implements only the IAU 2000 Earth Rotation Angle
//! (ERA) using linearly interpolated UT1−UTC samples. Interpolation is carried
//! out on the continuous UT1−TAI offset derived from the sample epochs, so a
//! UTC leap second does not create a spurious angular-rate spike. It is an
//! Earth-spin slice, **not** a complete GCRF-to-ITRF realization: polar motion,
//! precession-nutation/CIP motion, celestial-pole offsets, tides, and
//! terrestrial-realization corrections are omitted. ERA is the CIRS-to-TIRS
//! rotation, so this provider accepts only those intermediate frame identities
//! and does not label its result GCRF-to-ITRF2020.
//!
//! The caller supplies samples and a versioned provenance descriptor. This
//! crate performs no file access, network access, or implicit data selection.
//! Samples must be UTC epochs and finite typed UT1−UTC durations in seconds.
//! Requests may use any Hifitime time scale; their epochs are converted to TAI
//! for coverage and interpolation. ERA is evaluated from elapsed TAI seconds
//! relative to J2000 noon TAI plus the interpolated UT1−TAI offset.
//! Whole-day turns are reduced using the published excess rotation rate in
//! IERS Eq. (5.15), with the sub-day UT1 offset evaluated separately.
//!
//! ```no_run
//! use frames::{ReferenceFrame, ReferenceFrameTransformRequest};
//! use frames_eop::{EarthOrientationSample, Iau2000EraProvider};
//! use hifitime::{Epoch, TimeScale};
//! use units::Time;
//! use units::uom::si::time::second;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let samples = vec![
//!     EarthOrientationSample {
//!         epoch_utc: Epoch::from_gregorian_utc_at_midnight(2025, 1, 1),
//!         ut1_minus_utc: Time::new::<second>(0.1),
//!     },
//!     EarthOrientationSample {
//!         epoch_utc: Epoch::from_gregorian_utc_at_midnight(2025, 1, 2),
//!         ut1_minus_utc: Time::new::<second>(0.1),
//!     },
//! ];
//! let provider = Iau2000EraProvider::new(
//!     samples,
//!     "IERS",
//!     "caller-supplied EOP series",
//!     "release-2025-01",
//!     Some("sha256:replace-with-the-data-checksum"),
//! )?;
//! let request = ReferenceFrameTransformRequest::new(
//!     Epoch::from_gregorian_utc(2025, 1, 1, 12, 0, 0, 0),
//!     TimeScale::UTC,
//!     ReferenceFrame::CIRS,
//!     ReferenceFrame::TIRS,
//! )?;
//! let transform = provider.transform(request)?;
//! assert_eq!(transform.request(), request);
//! assert_eq!(transform.reference_data().len(), 3);
//! # Ok(())
//! # }
//! ```

use std::f64::consts::TAU;

use frames::{
    DirectionCosineMatrix, ProvenancedReferenceFrameTransform, ReferenceDataDescriptor,
    ReferenceFrame, ReferenceFrameTransform, ReferenceFrameTransformError,
    ReferenceFrameTransformProvider, ReferenceFrameTransformProviderError,
    ReferenceFrameTransformRequest,
};
use hifitime::{Epoch, TimeScale};
use thiserror::Error;
use units::uom::si::time::second;
use units::{AngularVelocityVector, Time};

const ERA_REFERENCE_FRACTION: f64 = 0.779_057_273_264_0;
const ERA_EXCESS_TURNS_PER_UT1_DAY: f64 = 0.002_737_811_911_354_48;
const ERA_TURNS_PER_UT1_DAY: f64 = 1.0 + ERA_EXCESS_TURNS_PER_UT1_DAY;
const SECONDS_PER_DAY: f64 = 86_400.0;
const NANOSECONDS_PER_DAY: i128 = 86_400_000_000_000;

fn j2000_noon_tai() -> Epoch {
    Epoch::from_gregorian_tai(2000, 1, 1, 12, 0, 0, 0)
}

/// One caller-supplied Earth-orientation sample.
///
/// `epoch_utc` is the UTC instant of the record; `ut1_minus_utc` is the
/// corresponding UT1−UTC value. The value is retained as a typed time
/// quantity, not as an unqualified scalar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EarthOrientationSample {
    /// UTC epoch at which the EOP value applies.
    pub epoch_utc: Epoch,
    /// UT1−UTC at `epoch_utc`, expressed as a typed SI time quantity.
    pub ut1_minus_utc: Time,
}

/// ERA-only provider backed by an immutable, caller-supplied UT1−UTC series.
///
/// Samples must be strictly chronological, finite, and contain at least two
/// records. The inclusive coverage interval is the first through last sample.
/// Between records, the equivalent continuous UT1−TAI offset is linearly
/// interpolated over elapsed TAI seconds. This is equivalent to interpolating
/// UT1−UTC away from UTC leap seconds and avoids a false rotation-rate spike
/// across a leap. The segment slope is included in the returned angular
/// velocity so that the velocity cross term is consistent with the angle.
///
/// The supported transform is restricted to `CIRS` and `TIRS`, the frames
/// related by ERA. The result is not a GCRF/ITRF2020 transform.
#[derive(Debug)]
pub struct Iau2000EraProvider {
    samples: Vec<EarthOrientationSample>,
    reference_data: [ReferenceDataDescriptor; 3],
}

impl Iau2000EraProvider {
    /// Validates caller-supplied UTC samples and their provenance.
    ///
    /// `authority`, `product`, `revision`, and an optional `checksum` identify
    /// the exact supplied EOP artifact. Fixed provenance records also identify
    /// the IAU 2000 ERA convention and Hifitime UTC leap-second table used.
    pub fn new(
        samples: Vec<EarthOrientationSample>,
        authority: impl Into<String>,
        product: impl Into<String>,
        revision: impl Into<String>,
        checksum: Option<&str>,
    ) -> Result<Self, EarthOrientationError> {
        if samples.len() < 2 {
            return Err(EarthOrientationError::InsufficientSamples {
                actual: samples.len(),
            });
        }

        let samples: Vec<_> = samples
            .into_iter()
            .map(|mut sample| {
                sample.epoch_utc = sample.epoch_utc.to_time_scale(TimeScale::UTC);
                sample
            })
            .collect();
        for (index, sample) in samples.iter().enumerate() {
            if sample.epoch_utc.to_gregorian_utc().0 < 1972 {
                return Err(EarthOrientationError::BeforeLeapSecondEra { sample: index });
            }
            let ut1_minus_utc_seconds = sample.ut1_minus_utc.get::<second>();
            if !ut1_minus_utc_seconds.is_finite() {
                return Err(EarthOrientationError::NonFiniteUt1MinusUtc { sample: index });
            }
            if index > 0 && sample.epoch_utc <= samples[index - 1].epoch_utc {
                return Err(EarthOrientationError::NonChronologicalSamples { sample: index });
            }
        }

        let eop = ReferenceDataDescriptor {
            authority: authority.into(),
            product: product.into(),
            revision: revision.into(),
            checksum: checksum.map(str::to_owned),
        };
        if let Some(field) = invalid_provenance_field(&eop) {
            return Err(EarthOrientationError::InvalidProvenance { field });
        }
        let convention = ReferenceDataDescriptor {
            authority: "International Astronomical Union".to_owned(),
            product: "Earth Rotation Angle".to_owned(),
            revision: "IAU 2000; IERS Conventions 2010 §5.5.3".to_owned(),
            checksum: None,
        };
        let time_scale = ReferenceDataDescriptor {
            authority: "Hifitime".to_owned(),
            product: "UTC-to-TAI leap-second table".to_owned(),
            revision: "4.3.0".to_owned(),
            checksum: None,
        };

        Ok(Self {
            samples,
            reference_data: [eop, convention, time_scale],
        })
    }

    /// Returns the immutable EOP, ERA-convention, and time-scale records.
    #[must_use]
    pub fn reference_data(&self) -> &[ReferenceDataDescriptor] {
        &self.reference_data
    }

    /// Resolves one request into a transform qualified by its provenance.
    ///
    /// The returned value retains the exact epoch, time scale, frame pair,
    /// origin, angular velocity, and borrowed data/convention
    /// descriptors used for this evaluation.
    pub fn transform(
        &self,
        request: ReferenceFrameTransformRequest,
    ) -> Result<ProvenancedEarthRotation<'_>, EarthOrientationError> {
        let inverse = match (request.source_frame(), request.target_frame()) {
            (ReferenceFrame::CIRS, ReferenceFrame::TIRS) => false,
            (ReferenceFrame::TIRS, ReferenceFrame::CIRS) => true,
            _ => {
                return Err(EarthOrientationError::UnsupportedFramePair {
                    request: Box::new(request),
                });
            }
        };

        let epoch_tai = request.epoch().to_time_scale(TimeScale::TAI);
        let (ut1_minus_tai_seconds, ut1_minus_tai_rate) =
            self.interpolate_ut1_minus_tai(epoch_tai, request)?;
        // Split before conversion to f64 so distant epochs retain sub-day
        // precision; the integral one-turn-per-day term vanishes modulo one.
        let tai_nanoseconds = (epoch_tai - j2000_noon_tai()).total_nanoseconds();
        let whole_days = tai_nanoseconds.div_euclid(NANOSECONDS_PER_DAY) as f64;
        let subday_ut1_seconds =
            tai_nanoseconds.rem_euclid(NANOSECONDS_PER_DAY) as f64 / 1.0e9 + ut1_minus_tai_seconds;
        let turns = ERA_REFERENCE_FRACTION
            + (ERA_EXCESS_TURNS_PER_UT1_DAY * whole_days).rem_euclid(1.0)
            + ERA_TURNS_PER_UT1_DAY * (subday_ut1_seconds / SECONDS_PER_DAY);
        if !subday_ut1_seconds.is_finite() || !turns.is_finite() {
            return Err(EarthOrientationError::NonFiniteEvaluation {
                request: Box::new(request),
            });
        }

        let era = TAU * turns.rem_euclid(1.0);
        let (sin_era, cos_era) = era.sin_cos();
        let cirs_to_tirs = DirectionCosineMatrix::new([
            [cos_era, sin_era, 0.0],
            [-sin_era, cos_era, 0.0],
            [0.0, 0.0, 1.0],
        ])
        .map_err(|error| {
            EarthOrientationError::InvalidTransform(
                ReferenceFrameTransformError::InvalidRotationMatrix(Box::new(error)),
            )
        })?;
        let rotation = if inverse {
            DirectionCosineMatrix::new([
                [cos_era, -sin_era, 0.0],
                [sin_era, cos_era, 0.0],
                [0.0, 0.0, 1.0],
            ])
            .map_err(|error| {
                EarthOrientationError::InvalidTransform(
                    ReferenceFrameTransformError::InvalidRotationMatrix(Box::new(error)),
                )
            })?
        } else {
            cirs_to_tirs
        };
        let angular_rate =
            TAU * ERA_TURNS_PER_UT1_DAY / SECONDS_PER_DAY * (1.0 + ut1_minus_tai_rate);
        let angular_velocity = AngularVelocityVector::from_radians_per_second(
            0.0,
            0.0,
            if inverse { -angular_rate } else { angular_rate },
        );
        let transform = ReferenceFrameTransform::new(request, rotation, angular_velocity)
            .map_err(EarthOrientationError::InvalidTransform)?;
        Ok(ProvenancedEarthRotation::new(
            transform,
            &self.reference_data,
        ))
    }

    fn interpolate_ut1_minus_tai(
        &self,
        epoch_tai: Epoch,
        request: ReferenceFrameTransformRequest,
    ) -> Result<(f64, f64), EarthOrientationError> {
        let first = self.samples[0];
        let last = self.samples[self.samples.len() - 1];
        let first_tai = first.epoch_utc.to_time_scale(TimeScale::TAI);
        let last_tai = last.epoch_utc.to_time_scale(TimeScale::TAI);
        if epoch_tai < first_tai || epoch_tai > last_tai {
            return Err(EarthOrientationError::EpochOutOfRange {
                request: Box::new(request),
                start: first_tai,
                end: last_tai,
            });
        }

        let upper = self
            .samples
            .partition_point(|sample| sample.epoch_utc.to_time_scale(TimeScale::TAI) <= epoch_tai);
        let left_index = if upper == self.samples.len() {
            upper - 2
        } else {
            upper.saturating_sub(1)
        };
        let left = self.samples[left_index];
        let right = self.samples[left_index + 1];
        let interval_seconds = (right.epoch_utc.to_time_scale(TimeScale::TAI)
            - left.epoch_utc.to_time_scale(TimeScale::TAI))
        .to_seconds();
        let elapsed_seconds =
            (epoch_tai - left.epoch_utc.to_time_scale(TimeScale::TAI)).to_seconds();
        let left_ut1_tai = ut1_minus_tai_seconds(left);
        let right_ut1_tai = ut1_minus_tai_seconds(right);
        let ut1_tai_difference = right_ut1_tai - left_ut1_tai;
        let fraction = elapsed_seconds / interval_seconds;
        let ut1_minus_tai_seconds = ut1_tai_difference.mul_add(fraction, left_ut1_tai);
        let ut1_minus_tai_rate = ut1_tai_difference / interval_seconds;
        if !interval_seconds.is_finite()
            || interval_seconds <= 0.0
            || !fraction.is_finite()
            || !ut1_minus_tai_seconds.is_finite()
            || !ut1_minus_tai_rate.is_finite()
        {
            return Err(EarthOrientationError::NonFiniteEvaluation {
                request: Box::new(request),
            });
        }
        Ok((ut1_minus_tai_seconds, ut1_minus_tai_rate))
    }
}

fn ut1_minus_tai_seconds(sample: EarthOrientationSample) -> f64 {
    sample.ut1_minus_utc.get::<second>() - tai_minus_utc_seconds(sample.epoch_utc)
}

fn tai_minus_utc_seconds(epoch_utc: Epoch) -> f64 {
    f64::from(epoch_utc.leap_seconds_iers())
}

/// One evaluated ERA transform with its immutable selected provenance.
///
/// This is the frames-layer transform/provenance pair returned by an
/// [`Iau2000EraProvider`] and remains usable through an object-safe provider.
pub type ProvenancedEarthRotation<'a> = ProvenancedReferenceFrameTransform<'a>;

impl ReferenceFrameTransformProvider for Iau2000EraProvider {
    fn reference_data(&self) -> &[ReferenceDataDescriptor] {
        &self.reference_data
    }

    fn reference_frame_transform(
        &self,
        request: ReferenceFrameTransformRequest,
    ) -> Result<ReferenceFrameTransform, ReferenceFrameTransformProviderError> {
        self.transform(request)
            .map(ProvenancedEarthRotation::transform)
            .map_err(|error| match error {
                EarthOrientationError::UnsupportedFramePair { .. } => {
                    ReferenceFrameTransformProviderError::UnsupportedRequest {
                        request: Box::new(request),
                    }
                }
                EarthOrientationError::EpochOutOfRange { .. } => {
                    ReferenceFrameTransformProviderError::EpochOutOfRange {
                        epoch: request.epoch(),
                        time_scale: request.time_scale(),
                    }
                }
                EarthOrientationError::InvalidTransform(source) => {
                    ReferenceFrameTransformProviderError::InvalidTransform {
                        source: Box::new(source),
                    }
                }
                source => ReferenceFrameTransformProviderError::Source {
                    source: Box::new(source),
                },
            })
    }
}

fn invalid_provenance_field(provenance: &ReferenceDataDescriptor) -> Option<ProvenanceField> {
    if provenance.authority.trim().is_empty() {
        Some(ProvenanceField::Authority)
    } else if provenance.product.trim().is_empty() {
        Some(ProvenanceField::Product)
    } else if provenance.revision.trim().is_empty() {
        Some(ProvenanceField::Revision)
    } else if provenance
        .checksum
        .as_deref()
        .is_some_and(|checksum| checksum.trim().is_empty())
    {
        Some(ProvenanceField::Checksum)
    } else {
        None
    }
}

/// A required identity field missing from the supplied EOP provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvenanceField {
    /// Data authority or publisher.
    Authority,
    /// EOP product or data family.
    Product,
    /// Immutable issue or revision.
    Revision,
    /// Optional checksum supplied as an empty string.
    Checksum,
}

impl std::fmt::Display for ProvenanceField {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Authority => "authority",
            Self::Product => "product",
            Self::Revision => "revision",
            Self::Checksum => "checksum",
        })
    }
}

/// Invalid caller-supplied Earth-orientation data or transform request.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EarthOrientationError {
    /// Fewer than two records cannot define interpolation or coverage.
    #[error("Earth-orientation data require at least two samples, got {actual}")]
    InsufficientSamples {
        /// Number of samples supplied.
        actual: usize,
    },
    /// UTC pre-dates the first integral TAI−UTC offset used by this slice.
    #[error("Earth-orientation sample {sample} predates 1972-01-01 UTC")]
    BeforeLeapSecondEra {
        /// Zero-based sample index.
        sample: usize,
    },
    /// A sample contains a non-finite UT1−UTC value.
    #[error("Earth-orientation sample {sample} has a non-finite UT1−UTC value")]
    NonFiniteUt1MinusUtc {
        /// Zero-based sample index.
        sample: usize,
    },
    /// Sample epochs are duplicated or not strictly increasing.
    #[error("Earth-orientation sample {sample} is not strictly later than its predecessor")]
    NonChronologicalSamples {
        /// Zero-based invalid sample index.
        sample: usize,
    },
    /// A required provenance field is empty.
    #[error("Earth-orientation provenance has an empty {field}")]
    InvalidProvenance {
        /// Empty field.
        field: ProvenanceField,
    },
    /// The requested frames do not match this provider's declared axes.
    #[error("ERA provider does not support requested frame pair in {request:?}")]
    UnsupportedFramePair {
        /// Rejected request containing the frame pair and full request context.
        request: Box<ReferenceFrameTransformRequest>,
    },
    /// The requested epoch is outside the closed sample coverage interval.
    #[error("request {request:?} is outside Earth-orientation coverage [{start:?}, {end:?}]")]
    EpochOutOfRange {
        /// Rejected request, retaining the caller's epoch, scale, and frames.
        request: Box<ReferenceFrameTransformRequest>,
        /// First sample epoch expressed in TAI.
        start: Epoch,
        /// Last sample epoch expressed in TAI.
        end: Epoch,
    },
    /// An intermediate floating-point result was not finite.
    #[error("Earth-orientation evaluation for {request:?} produced a non-finite value")]
    NonFiniteEvaluation {
        /// Request associated with the failed numerical evaluation.
        request: Box<ReferenceFrameTransformRequest>,
    },
    /// The validated frames-layer transform could not be constructed.
    #[error("ERA provider produced an invalid frames-layer transform")]
    InvalidTransform(#[source] ReferenceFrameTransformError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use frames::{BodyFixedTransformDirection, FrameKinematics, ReferenceFrameTransformRequest};
    use hifitime::Duration;
    use units::Position;

    const ERA_AT_J2000_RADIANS: f64 = 4.894_961_212_823_058;
    const RATE_TOLERANCE_RAD_PER_SEC: f64 = 1.0e-15;
    const ANGLE_TOLERANCE_RAD: f64 = 2.0e-12;

    fn epoch_utc(day: u8, hour: u8) -> Epoch {
        Epoch::from_gregorian_utc(2000, 1, day, hour, 0, 0, 0)
    }

    fn provider(dut1_start: f64, dut1_end: f64) -> Iau2000EraProvider {
        Iau2000EraProvider::new(
            vec![
                EarthOrientationSample {
                    epoch_utc: epoch_utc(1, 0),
                    ut1_minus_utc: Time::new::<second>(dut1_start),
                },
                EarthOrientationSample {
                    epoch_utc: epoch_utc(2, 0),
                    ut1_minus_utc: Time::new::<second>(dut1_end),
                },
            ],
            "IERS",
            "EOP test series",
            "test-revision",
            Some("sha256:test"),
        )
        .expect("valid EOP series")
    }

    fn request(
        epoch: Epoch,
        direction: BodyFixedTransformDirection,
    ) -> ReferenceFrameTransformRequest {
        let (source, target) = match direction {
            BodyFixedTransformDirection::InertialToBodyFixed => {
                (ReferenceFrame::CIRS, ReferenceFrame::TIRS)
            }
            BodyFixedTransformDirection::BodyFixedToInertial => {
                (ReferenceFrame::TIRS, ReferenceFrame::CIRS)
            }
        };
        ReferenceFrameTransformRequest::new(epoch, TimeScale::UTC, source, target)
            .expect("valid CIRS/TIRS request")
    }

    #[test]
    fn independent_iers_era_vector_at_j2000_is_reproduced() {
        // IERS Conventions (2010), §5.5.3, Eq. (5.15): at JD(UT1)=2451545.0,
        // ERA = 2π × 0.7790572732640 = 280.46061837504 degrees.
        let j2000_utc = Epoch::from_gregorian_utc(2000, 1, 1, 12, 0, 0, 0);
        assert_eq!(tai_minus_utc_seconds(j2000_utc), 32.0);
        assert_eq!(
            ut1_minus_tai_seconds(EarthOrientationSample {
                epoch_utc: j2000_utc,
                ut1_minus_utc: Time::new::<second>(0.0),
            }),
            -32.0
        );
        let provider = provider(0.0, 0.0);
        let transform = provider
            .transform(request(
                epoch_utc(1, 12),
                BodyFixedTransformDirection::InertialToBodyFixed,
            ))
            .expect("J2000 UTC epoch lies inside coverage");
        let rows = transform.rotation().rows();
        assert!((rows[0][0] - ERA_AT_J2000_RADIANS.cos()).abs() < ANGLE_TOLERANCE_RAD);
        assert!((rows[0][1] - ERA_AT_J2000_RADIANS.sin()).abs() < ANGLE_TOLERANCE_RAD);
        assert!((rows[1][0] + ERA_AT_J2000_RADIANS.sin()).abs() < ANGLE_TOLERANCE_RAD);
        assert!((rows[1][1] - ERA_AT_J2000_RADIANS.cos()).abs() < ANGLE_TOLERANCE_RAD);

        let cirs = FrameKinematics::new(
            Position::from_metres(1.0, 0.0, 0.0),
            units::VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
            ReferenceFrame::CIRS,
        )
        .expect("finite CIRS test state");
        let tirs = transform
            .transform_kinematics(cirs)
            .expect("valid direct transform");
        let expected_rate = TAU * ERA_TURNS_PER_UT1_DAY / SECONDS_PER_DAY;
        let [x, y, _] = tirs.position().to_metres();
        let [vx, vy, _] = tirs.velocity().to_metres_per_second();
        assert!((x - ERA_AT_J2000_RADIANS.cos()).abs() < ANGLE_TOLERANCE_RAD);
        assert!((y + ERA_AT_J2000_RADIANS.sin()).abs() < ANGLE_TOLERANCE_RAD);
        assert!((vx + expected_rate * ERA_AT_J2000_RADIANS.sin()).abs() < 1.0e-12);
        assert!((vy + expected_rate * ERA_AT_J2000_RADIANS.cos()).abs() < 1.0e-12);
    }

    #[test]
    fn independent_iers_era_vectors_preserve_precision_away_from_j2000() {
        // Exact rational evaluation of IERS Eq. (5.15), using the published
        // decimals 0.7790572732640 and 1.00273781191135448, integer nanoseconds
        // since J2000 UT1 noon, and modulo reduction before conversion to f64.
        let vectors = [
            (
                Epoch::from_gregorian_utc(2025, 1, 1, 12, 34, 56, 987_654_321),
                0.1,
                5.058_554_460_940_293,
            ),
            (
                Epoch::from_gregorian_utc_at_midnight(1972, 1, 2),
                0.0,
                1.764_467_734_032_388,
            ),
            (
                Epoch::from_gregorian_utc(1999, 12, 31, 23, 59, 59, 123_456_789),
                -0.3,
                1.744_681_674_560_645,
            ),
            (
                Epoch::from_gregorian_utc_at_midnight(2025, 1, 1),
                0.1,
                1.755_445_963_197_561_6,
            ),
            (
                Epoch::from_gregorian_utc(2100, 1, 1, 0, 0, 0, 1),
                -0.2,
                1.735_831_153_034_682_4,
            ),
            (
                Epoch::from_gregorian_utc(2200, 1, 1, 18, 0, 0, 123_456_789),
                0.25,
                0.151_854_366_293_834,
            ),
        ];
        for (epoch, dut1, expected_era) in vectors {
            let provider = Iau2000EraProvider::new(
                vec![
                    EarthOrientationSample {
                        epoch_utc: epoch - Duration::from_seconds(43_200.0),
                        ut1_minus_utc: Time::new::<second>(dut1),
                    },
                    EarthOrientationSample {
                        epoch_utc: epoch + Duration::from_seconds(43_200.0),
                        ut1_minus_utc: Time::new::<second>(dut1),
                    },
                ],
                "IERS",
                "exact-rational ERA test vectors",
                "IERS Conventions 2010 Eq. (5.15)",
                None,
            )
            .expect("valid constant UT1 offset");
            for direction in [
                BodyFixedTransformDirection::InertialToBodyFixed,
                BodyFixedTransformDirection::BodyFixedToInertial,
            ] {
                let transform = provider
                    .transform(request(epoch, direction))
                    .expect("covered reference epoch");
                let rows = transform.rotation().rows();
                let sign = match direction {
                    BodyFixedTransformDirection::InertialToBodyFixed => 1.0,
                    BodyFixedTransformDirection::BodyFixedToInertial => -1.0,
                };
                let actual_era = (sign * rows[0][1]).atan2(rows[0][0]);
                let error = (actual_era - expected_era)
                    .sin()
                    .atan2((actual_era - expected_era).cos());
                assert!(
                    error.abs() < ANGLE_TOLERANCE_RAD,
                    "{epoch:?}, {direction:?}: ERA error {error:e} rad"
                );
            }
        }
    }

    #[test]
    fn direct_and_inverse_position_velocity_transforms_recover_the_state() {
        let provider = provider(0.2, 0.2);
        let epoch = epoch_utc(1, 12);
        let cirs = FrameKinematics::new(
            Position::from_metres(7_000_000.0, -1_200_000.0, 800_000.0),
            units::VelocityVector::from_metres_per_second(1_000.0, 7_400.0, -120.0),
            ReferenceFrame::CIRS,
        )
        .expect("finite CIRS state");
        let tirs = provider
            .transform(request(
                epoch,
                BodyFixedTransformDirection::InertialToBodyFixed,
            ))
            .expect("covered forward transform")
            .transform_kinematics(cirs)
            .expect("forward state transform");
        assert_eq!(tirs.frame(), ReferenceFrame::TIRS);
        let recovered = provider
            .transform(request(
                epoch,
                BodyFixedTransformDirection::BodyFixedToInertial,
            ))
            .expect("covered inverse transform")
            .transform_kinematics(tirs)
            .expect("inverse state transform");
        assert_eq!(recovered.frame(), ReferenceFrame::CIRS);

        for (actual, expected) in recovered
            .position()
            .to_metres()
            .into_iter()
            .zip(cirs.position().to_metres())
        {
            assert!((actual - expected).abs() < 2.0e-8);
        }
        for (actual, expected) in recovered
            .velocity()
            .to_metres_per_second()
            .into_iter()
            .zip(cirs.velocity().to_metres_per_second())
        {
            assert!((actual - expected).abs() < 2.0e-12);
        }
    }

    #[test]
    fn angular_rate_includes_interpolated_ut1_utc_slope() {
        let start = -0.2;
        let end = 0.2;
        let provider = provider(start, end);
        let transform = provider
            .transform(request(
                epoch_utc(1, 12),
                BodyFixedTransformDirection::InertialToBodyFixed,
            ))
            .expect("covered transform");
        let interval_seconds = (epoch_utc(2, 0) - epoch_utc(1, 0)).to_seconds();
        let expected = TAU * ERA_TURNS_PER_UT1_DAY / SECONDS_PER_DAY
            * (1.0 + (end - start) / interval_seconds);
        let actual = transform.angular_velocity().to_radians_per_second()[2];
        assert!((actual - expected).abs() < RATE_TOLERANCE_RAD_PER_SEC);
    }

    #[test]
    fn coverage_is_closed_and_outside_epochs_are_typed_errors() {
        let provider = provider(0.0, 0.0);
        for epoch in [epoch_utc(1, 0), epoch_utc(2, 0)] {
            provider
                .transform(request(
                    epoch,
                    BodyFixedTransformDirection::InertialToBodyFixed,
                ))
                .expect("both coverage endpoints are included");
        }
        let outside_request = ReferenceFrameTransformRequest::new(
            epoch_utc(2, 0) + Duration::from_seconds(0.001),
            TimeScale::TAI,
            ReferenceFrame::CIRS,
            ReferenceFrame::TIRS,
        )
        .expect("valid out-of-coverage request");
        let error = provider
            .transform(outside_request)
            .expect_err("post-coverage request fails");
        match error {
            EarthOrientationError::EpochOutOfRange { request, .. } => {
                assert_eq!(*request, outside_request);
                assert_eq!(request.time_scale(), TimeScale::TAI);
            }
            other => panic!("expected a coverage error, got {other:?}"),
        }
    }

    #[test]
    fn non_utc_requests_are_converted_using_their_declared_time_scale() {
        let epoch_tai = epoch_utc(1, 12).to_time_scale(TimeScale::TAI);
        let tai_request = ReferenceFrameTransformRequest::new(
            epoch_tai,
            TimeScale::TAI,
            ReferenceFrame::CIRS,
            ReferenceFrame::TIRS,
        )
        .expect("valid request");
        let zero_offset_provider = provider(0.0, 0.0);
        let transform = zero_offset_provider
            .transform(tai_request)
            .expect("same physical epoch is covered");
        let utc_provider = provider(0.0, 0.0);
        let expected = utc_provider
            .transform(request(
                epoch_utc(1, 12),
                BodyFixedTransformDirection::InertialToBodyFixed,
            ))
            .expect("UTC request is covered");
        assert_eq!(transform.rotation(), expected.rotation());
        assert_eq!(transform.time_scale(), TimeScale::TAI);
    }

    #[test]
    fn unsupported_frames_and_invalid_provenance_are_rejected() {
        let provider = provider(0.0, 0.0);
        let unsupported = ReferenceFrameTransformRequest::new(
            epoch_utc(1, 12),
            TimeScale::UTC,
            ReferenceFrame::EME2000,
            ReferenceFrame::ITRF2020,
        )
        .expect("valid same-origin frame pair");
        assert!(matches!(
            provider.transform(unsupported),
            Err(EarthOrientationError::UnsupportedFramePair { .. })
        ));
        let gcrf_to_itrf = ReferenceFrameTransformRequest::new(
            epoch_utc(1, 12),
            TimeScale::UTC,
            ReferenceFrame::GCRF,
            ReferenceFrame::ITRF2020,
        )
        .expect("valid same-origin frame pair");
        assert!(matches!(
            provider.transform(gcrf_to_itrf),
            Err(EarthOrientationError::UnsupportedFramePair { .. })
        ));

        let invalid = Iau2000EraProvider::new(
            vec![
                EarthOrientationSample {
                    epoch_utc: epoch_utc(1, 0),
                    ut1_minus_utc: Time::new::<second>(0.0),
                },
                EarthOrientationSample {
                    epoch_utc: epoch_utc(2, 0),
                    ut1_minus_utc: Time::new::<second>(0.0),
                },
            ],
            "IERS",
            "EOP",
            " ",
            None,
        );
        assert!(matches!(
            invalid,
            Err(EarthOrientationError::InvalidProvenance {
                field: ProvenanceField::Revision
            })
        ));
    }

    #[test]
    fn invalid_sample_values_and_order_are_rejected() {
        let later = EarthOrientationSample {
            epoch_utc: epoch_utc(2, 0),
            ut1_minus_utc: Time::new::<second>(0.0),
        };
        let earlier = EarthOrientationSample {
            epoch_utc: epoch_utc(1, 0),
            ut1_minus_utc: Time::new::<second>(0.0),
        };
        let invalid_value = Iau2000EraProvider::new(
            vec![
                EarthOrientationSample {
                    epoch_utc: epoch_utc(1, 0),
                    ut1_minus_utc: Time::new::<second>(f64::NAN),
                },
                later,
            ],
            "IERS",
            "EOP test series",
            "invalid-value",
            None,
        );
        assert!(matches!(
            invalid_value,
            Err(EarthOrientationError::NonFiniteUt1MinusUtc { sample: 0 })
        ));

        let unordered = Iau2000EraProvider::new(
            vec![later, earlier],
            "IERS",
            "EOP test series",
            "unordered",
            None,
        );
        assert!(matches!(
            unordered,
            Err(EarthOrientationError::NonChronologicalSamples { sample: 1 })
        ));
    }

    #[test]
    fn provenance_exposes_the_exact_input_and_convention_revision() {
        let provider = provider(0.0, 0.0);
        let artifacts = provider.reference_data();
        assert_eq!(artifacts.len(), 3);
        assert_eq!(artifacts[0].revision, "test-revision");
        assert_eq!(artifacts[0].checksum.as_deref(), Some("sha256:test"));
        assert_eq!(artifacts[1].product, "Earth Rotation Angle");
        assert!(artifacts[1].revision.starts_with("IAU 2000;"));
        assert_eq!(artifacts[2].product, "UTC-to-TAI leap-second table");
        assert_eq!(artifacts[2].revision, "4.3.0");

        let request = request(
            epoch_utc(1, 12),
            BodyFixedTransformDirection::InertialToBodyFixed,
        );
        let resolved = provider.transform(request).expect("covered request");
        assert_eq!(resolved.request(), request);
        assert_eq!(resolved.reference_data(), artifacts);
    }

    #[test]
    fn leap_second_boundary_preserves_ut1_rate_and_coverage() {
        let provider = Iau2000EraProvider::new(
            vec![
                EarthOrientationSample {
                    epoch_utc: Epoch::from_gregorian_utc_at_midnight(2016, 12, 31),
                    ut1_minus_utc: Time::new::<second>(-0.4),
                },
                EarthOrientationSample {
                    epoch_utc: Epoch::from_gregorian_utc_at_midnight(2017, 1, 1),
                    ut1_minus_utc: Time::new::<second>(0.6),
                },
            ],
            "IERS",
            "EOP test series",
            "leap-second-test",
            None,
        )
        .expect("valid leap-second-spanning series");
        let request_at = |epoch| {
            ReferenceFrameTransformRequest::new(
                epoch,
                TimeScale::TAI,
                ReferenceFrame::CIRS,
                ReferenceFrame::TIRS,
            )
            .expect("valid request")
        };
        let before = Epoch::from_gregorian_tai(2017, 1, 1, 0, 0, 35, 0);
        let leap = Epoch::from_gregorian_tai(2017, 1, 1, 0, 0, 36, 0);
        let after = Epoch::from_gregorian_tai(2017, 1, 1, 0, 0, 37, 0);
        let eop_before = Epoch::from_gregorian_utc_at_midnight(2016, 12, 31);
        let eop_after = Epoch::from_gregorian_utc_at_midnight(2017, 1, 1);
        assert_eq!(tai_minus_utc_seconds(eop_before), 36.0);
        assert_eq!(tai_minus_utc_seconds(eop_after), 37.0);
        assert_eq!(
            ut1_minus_tai_seconds(EarthOrientationSample {
                epoch_utc: eop_before,
                ut1_minus_utc: Time::new::<second>(-0.4),
            }),
            -36.4
        );
        assert_eq!(
            ut1_minus_tai_seconds(EarthOrientationSample {
                epoch_utc: eop_after,
                ut1_minus_utc: Time::new::<second>(0.6),
            }),
            -36.4
        );
        let before_transform = provider
            .transform(request_at(before))
            .expect("covered pre-leap epoch");
        let leap_transform = provider
            .transform(request_at(leap))
            .expect("covered leap-second epoch");
        let after_transform = provider
            .transform(request_at(after))
            .expect("covered post-leap epoch");

        let rate = before_transform.angular_velocity().to_radians_per_second()[2];
        for transform in [leap_transform, after_transform] {
            assert!(
                (transform.angular_velocity().to_radians_per_second()[2] - rate).abs() < 1.0e-15
            );
        }
        let angle = |transform: ProvenancedEarthRotation<'_>| {
            let rows = transform.rotation().rows();
            rows[0][1].atan2(rows[0][0])
        };
        let before_to_leap = (angle(leap_transform) - angle(before_transform)).rem_euclid(TAU);
        let leap_to_after = (angle(after_transform) - angle(leap_transform)).rem_euclid(TAU);
        assert!((before_to_leap - rate * (leap - before).to_seconds()).abs() < 1.0e-8);
        assert!((leap_to_after - rate * (after - leap).to_seconds()).abs() < 1.0e-8);
    }

    #[test]
    fn utc_samples_before_the_integral_leap_table_era_are_rejected() {
        let error = Iau2000EraProvider::new(
            vec![
                EarthOrientationSample {
                    epoch_utc: Epoch::from_gregorian_utc_at_midnight(1971, 12, 31),
                    ut1_minus_utc: Time::new::<second>(0.0),
                },
                EarthOrientationSample {
                    epoch_utc: Epoch::from_gregorian_utc_at_midnight(1972, 1, 1),
                    ut1_minus_utc: Time::new::<second>(0.0),
                },
            ],
            "IERS",
            "EOP test series",
            "pre-leap-table-test",
            None,
        )
        .expect_err("pre-1972 fractional UTC offsets are outside this model");
        assert!(matches!(
            error,
            EarthOrientationError::BeforeLeapSecondEra { sample: 0 }
        ));
    }

    #[test]
    fn provider_trait_preserves_typed_coverage_and_frame_failures() {
        let eop_provider = provider(0.0, 0.0);
        let object: &dyn ReferenceFrameTransformProvider = &eop_provider;
        assert_eq!(object.reference_data(), eop_provider.reference_data());
        let covered_request = request(
            epoch_utc(1, 12),
            BodyFixedTransformDirection::InertialToBodyFixed,
        );
        let resolved = object
            .reference_frame_transform_with_provenance(covered_request)
            .expect("trait-object resolution retains provenance");
        assert_eq!(resolved.request(), covered_request);
        assert_eq!(resolved.reference_data(), eop_provider.reference_data());
        let boxed: Box<dyn ReferenceFrameTransformProvider> = Box::new(provider(0.0, 0.0));
        assert_eq!(boxed.reference_data().len(), 3);
        let shared: std::sync::Arc<dyn ReferenceFrameTransformProvider> =
            std::sync::Arc::new(provider(0.0, 0.0));
        assert_eq!(shared.reference_data().len(), 3);
        let coverage = ReferenceFrameTransformProvider::reference_frame_transform(
            &eop_provider,
            request(
                epoch_utc(2, 0) + Duration::from_seconds(1.0),
                BodyFixedTransformDirection::InertialToBodyFixed,
            ),
        )
        .expect_err("coverage is closed");
        assert!(matches!(
            coverage,
            ReferenceFrameTransformProviderError::EpochOutOfRange { .. }
        ));
    }
}
