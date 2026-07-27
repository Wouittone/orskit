//! IERS-convention terrestrial/celestial transforms.
//!
//! The celestial model is evaluated by the unmodified `sofars` 0.6.1
//! dependency, a pure-Rust implementation derived from the IAU SOFA
//! collection. This crate is not SOFA software and is not endorsed by the IAU
//! SOFA Board. See the [SOFA terms](https://www.iausofa.org/terms-and-conditions)
//! and [IERS Conventions (2010), Chapter 5](https://iers-conventions.obspm.fr/content/chapter5/icc5.pdf).

use hifitime::{Duration, Epoch, Unit};
use orskit_data::{ArtifactCoverage, ArtifactDescriptor, CoverageError, VerifiedArtifact};
use sofars::pnp::c2t06a;
use thiserror::Error;
use units::uom::si::{angle::radian, time::second};
use units::{Angle, Time};

use crate::{
    FrameKinematics, FrameKinematicsError, FrameReferenceDataSupplier, Position, ReferenceFrame,
    VelocityVector,
};

const JULIAN_DATE_MJD_ORIGIN: f64 = 2_400_000.5;
const SECONDS_PER_DAY: f64 = 86_400.0;
const DERIVATIVE_STEP_SECONDS: f64 = 0.5;

/// Celestial and Earth-rotation conventions used by a terrestrial transform.
///
/// The current model is the CIO-based IERS Conventions (2010) procedure with
/// IAU 2006 precession and IAU 2000A nutation. It uses the IAU Earth Rotation
/// Angle, the IERS TIO locator, and caller-supplied UT1 and polar motion.
/// Celestial-pole offsets (`dX`, `dY`) are not applied in this first provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EarthOrientationConvention {
    /// IERS Conventions (2010), IAU 2006 precession and IAU 2000A nutation.
    Iers2010Iau2006_2000A,
}

/// One caller-decoded Earth-orientation sample.
///
/// `epoch` is an absolute Hifitime instant. `ut1_minus_tai` is used rather
/// than UT1-UTC so interpolation remains continuous across UTC leap seconds.
/// Polar motion coordinates follow the IERS/SOFA sign convention: `x` is
/// measured along the Greenwich meridian and `y` along 90 degrees west.
/// The provider does not synthesize subdaily tidal or libration corrections;
/// callers must include any corrections required by their selected product in
/// the supplied series.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EarthOrientationSample {
    epoch: Epoch,
    ut1_minus_tai: Time,
    x_pole: Angle,
    y_pole: Angle,
}

impl EarthOrientationSample {
    /// Constructs a finite Earth-orientation sample.
    pub fn new(
        epoch: Epoch,
        ut1_minus_tai: Time,
        x_pole: Angle,
        y_pole: Angle,
    ) -> Result<Self, EarthOrientationError> {
        if !ut1_minus_tai.get::<second>().is_finite() {
            return Err(EarthOrientationError::NonFiniteSample {
                field: EarthOrientationField::Ut1MinusTai,
            });
        }
        if !x_pole.get::<radian>().is_finite() {
            return Err(EarthOrientationError::NonFiniteSample {
                field: EarthOrientationField::XPole,
            });
        }
        if !y_pole.get::<radian>().is_finite() {
            return Err(EarthOrientationError::NonFiniteSample {
                field: EarthOrientationField::YPole,
            });
        }
        Ok(Self {
            epoch,
            ut1_minus_tai,
            x_pole,
            y_pole,
        })
    }

    /// Sample epoch.
    #[must_use]
    pub const fn epoch(self) -> Epoch {
        self.epoch
    }

    /// UT1 minus TAI at the sample epoch.
    #[must_use]
    pub const fn ut1_minus_tai(self) -> Time {
        self.ut1_minus_tai
    }

    /// IERS polar-motion x coordinate.
    #[must_use]
    pub const fn x_pole(self) -> Angle {
        self.x_pole
    }

    /// IERS polar-motion y coordinate.
    #[must_use]
    pub const fn y_pole(self) -> Angle {
        self.y_pole
    }
}

/// IERS 2010 GCRF/ITRF2020 transform backed by one verified EOP artifact.
///
/// Construction consumes the exact [`VerifiedArtifact`] selected by the
/// caller and typed samples decoded from those bytes. Samples are linearly
/// interpolated in uniform TAI time, including the continuous UT1-TAI
/// difference. The descriptor's interval must exactly equal the first and last
/// sample epochs, and interpolation across a wider interval than
/// `maximum_interpolation_span` is rejected.
/// No subdaily EOP correction model is applied after interpolation.
///
/// Position uses the IERS GCRF-to-ITRS rotation. Velocity includes the time
/// derivative of the complete rotation, evaluated with a 0.5-second
/// second-order finite-difference stencil. Its truncation contribution scales
/// as the third time derivative of the rotated position times the square of
/// the step; the validation case bounds this below 2 micrometres per second at
/// an Earth-surface radius. No matrix is exposed publicly.
#[derive(Debug)]
pub struct Iers2010EarthOrientation {
    artifact: VerifiedArtifact,
    samples: Box<[EarthOrientationSample]>,
    maximum_interpolation_span: Duration,
}

impl Iers2010EarthOrientation {
    /// Builds a provider from caller-verified bytes and samples decoded from
    /// that same artifact.
    ///
    /// At least two strictly increasing samples are required. The descriptor
    /// must declare an interval exactly matching their endpoints. The maximum
    /// interpolation span must be positive.
    pub fn new(
        artifact: VerifiedArtifact,
        samples: Vec<EarthOrientationSample>,
        maximum_interpolation_span: Duration,
    ) -> Result<Self, EarthOrientationError> {
        if maximum_interpolation_span <= Duration::ZERO {
            return Err(EarthOrientationError::NonPositiveMaximumInterpolationSpan);
        }
        if samples.len() < 2 {
            return Err(EarthOrientationError::InsufficientSamples {
                actual: samples.len(),
            });
        }
        for (index, pair) in samples.windows(2).enumerate() {
            if pair[0].epoch >= pair[1].epoch {
                return Err(EarthOrientationError::NonIncreasingEpoch { index: index + 1 });
            }
        }

        let start = samples[0].epoch;
        let end = samples[samples.len() - 1].epoch;
        match artifact.descriptor().coverage() {
            ArtifactCoverage::Interval(coverage)
                if coverage.start() == start && coverage.end() == end => {}
            declared => {
                return Err(EarthOrientationError::CoverageMismatch {
                    declared,
                    sample_start: start,
                    sample_end: end,
                });
            }
        }

        Ok(Self {
            artifact,
            samples: samples.into_boxed_slice(),
            maximum_interpolation_span,
        })
    }

    /// Convention set used by this provider.
    #[must_use]
    pub const fn convention(&self) -> EarthOrientationConvention {
        EarthOrientationConvention::Iers2010Iau2006_2000A
    }

    /// Largest sample interval across which linear interpolation is permitted.
    #[must_use]
    pub const fn maximum_interpolation_span(&self) -> Duration {
        self.maximum_interpolation_span
    }

    fn orientation_at(
        &self,
        epoch: Epoch,
    ) -> Result<InterpolatedOrientation, EarthOrientationError> {
        self.artifact.descriptor().coverage().require(epoch)?;

        match self
            .samples
            .binary_search_by(|sample| sample.epoch.cmp(&epoch))
        {
            Ok(index) => Ok(self.samples[index].into()),
            Err(0) | Err(_) if epoch < self.samples[0].epoch => unreachable!(
                "descriptor and sample coverage were validated to have identical endpoints"
            ),
            Err(index) if index == self.samples.len() => unreachable!(
                "descriptor and sample coverage were validated to have identical endpoints"
            ),
            Err(index) => {
                let before = self.samples[index - 1];
                let after = self.samples[index];
                let span = after.epoch - before.epoch;
                if span > self.maximum_interpolation_span {
                    return Err(EarthOrientationError::InterpolationGap {
                        before: before.epoch,
                        after: after.epoch,
                        maximum: self.maximum_interpolation_span,
                    });
                }
                let fraction = (epoch - before.epoch).to_seconds() / span.to_seconds();
                Ok(InterpolatedOrientation {
                    ut1_minus_tai_seconds: interpolate(
                        before.ut1_minus_tai.get::<second>(),
                        after.ut1_minus_tai.get::<second>(),
                        fraction,
                    ),
                    x_pole_radians: interpolate(
                        before.x_pole.get::<radian>(),
                        after.x_pole.get::<radian>(),
                        fraction,
                    ),
                    y_pole_radians: interpolate(
                        before.y_pole.get::<radian>(),
                        after.y_pole.get::<radian>(),
                        fraction,
                    ),
                })
            }
        }
    }

    fn celestial_to_terrestrial(&self, epoch: Epoch) -> Result<Matrix3, EarthOrientationError> {
        let orientation = self.orientation_at(epoch)?;
        let (tt_day, tt_fraction) = split_julian_date(epoch.to_mjd_tt_duration(), 0.0);
        let (ut1_day, ut1_fraction) = split_julian_date(
            epoch.to_tai_duration() + Unit::Day * hifitime::MJD_J1900,
            orientation.ut1_minus_tai_seconds,
        );
        Ok(c2t06a(
            tt_day,
            tt_fraction,
            ut1_day,
            ut1_fraction,
            orientation.x_pole_radians,
            orientation.y_pole_radians,
        ))
    }

    fn rotation_and_derivative(
        &self,
        epoch: Epoch,
    ) -> Result<(Matrix3, Matrix3), EarthOrientationError> {
        let step = Duration::from_seconds(DERIVATIVE_STEP_SECONDS);
        let coverage = match self.artifact.descriptor().coverage() {
            ArtifactCoverage::Interval(coverage) => coverage,
            ArtifactCoverage::AllTime => unreachable!("construction rejects all-time EOP coverage"),
        };
        let current = self.celestial_to_terrestrial(epoch)?;

        let derivative = if epoch - step >= coverage.start() && epoch + step <= coverage.end() {
            scale_matrix(
                subtract_matrix(
                    self.celestial_to_terrestrial(epoch + step)?,
                    self.celestial_to_terrestrial(epoch - step)?,
                ),
                1.0 / (2.0 * DERIVATIVE_STEP_SECONDS),
            )
        } else if epoch + step * 2 <= coverage.end() {
            let one = self.celestial_to_terrestrial(epoch + step)?;
            let two = self.celestial_to_terrestrial(epoch + step * 2)?;
            scale_matrix(
                linear_combination([(-3.0, current), (4.0, one), (-1.0, two)]),
                1.0 / (2.0 * DERIVATIVE_STEP_SECONDS),
            )
        } else if epoch - step * 2 >= coverage.start() {
            let one = self.celestial_to_terrestrial(epoch - step)?;
            let two = self.celestial_to_terrestrial(epoch - step * 2)?;
            scale_matrix(
                linear_combination([(3.0, current), (-4.0, one), (1.0, two)]),
                1.0 / (2.0 * DERIVATIVE_STEP_SECONDS),
            )
        } else {
            return Err(EarthOrientationError::DerivativeStencilOutsideCoverage {
                epoch,
                start: coverage.start(),
                end: coverage.end(),
            });
        };

        Ok((current, derivative))
    }
}

impl FrameReferenceDataSupplier for Iers2010EarthOrientation {
    type Error = EarthOrientationError;

    fn reference_data(&self) -> &[ArtifactDescriptor] {
        std::slice::from_ref(self.artifact.descriptor())
    }

    fn transform_kinematics(
        &self,
        epoch: Epoch,
        kinematics: FrameKinematics,
        target: ReferenceFrame,
    ) -> Result<FrameKinematics, Self::Error> {
        let source = kinematics.frame();
        if source == target {
            return Ok(kinematics);
        }
        let gcrf_to_itrf = if source == ReferenceFrame::GCRF && target == ReferenceFrame::ITRF2020 {
            true
        } else if source == ReferenceFrame::ITRF2020 && target == ReferenceFrame::GCRF {
            false
        } else {
            return Err(EarthOrientationError::UnsupportedTransform {
                frames: Box::new((source, target)),
            });
        };
        let (rotation, derivative) = self.rotation_and_derivative(epoch)?;
        let position = kinematics.position().to_metres();
        let velocity = kinematics.velocity().to_metres_per_second();

        let (target_position, target_velocity) = if gcrf_to_itrf {
            let target_position = multiply(rotation, position);
            (
                target_position,
                add(multiply(rotation, velocity), multiply(derivative, position)),
            )
        } else {
            let transpose = transpose(rotation);
            let target_position = multiply(transpose, position);
            (
                target_position,
                multiply(
                    transpose,
                    subtract(velocity, multiply(derivative, target_position)),
                ),
            )
        };

        Ok(FrameKinematics::new(
            Position::from_metres(target_position[0], target_position[1], target_position[2]),
            VelocityVector::from_metres_per_second(
                target_velocity[0],
                target_velocity[1],
                target_velocity[2],
            ),
            target,
        )?)
    }
}

#[derive(Debug, Clone, Copy)]
struct InterpolatedOrientation {
    ut1_minus_tai_seconds: f64,
    x_pole_radians: f64,
    y_pole_radians: f64,
}

impl From<EarthOrientationSample> for InterpolatedOrientation {
    fn from(sample: EarthOrientationSample) -> Self {
        Self {
            ut1_minus_tai_seconds: sample.ut1_minus_tai.get::<second>(),
            x_pole_radians: sample.x_pole.get::<radian>(),
            y_pole_radians: sample.y_pole.get::<radian>(),
        }
    }
}

/// A field in one Earth-orientation sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EarthOrientationField {
    /// UT1 minus TAI.
    Ut1MinusTai,
    /// Polar-motion x coordinate.
    XPole,
    /// Polar-motion y coordinate.
    YPole,
}

/// Failure while constructing or evaluating an Earth-orientation transform.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EarthOrientationError {
    /// A sample field was NaN or infinite.
    #[error("Earth-orientation sample has a non-finite {field:?} value")]
    NonFiniteSample {
        /// Invalid field.
        field: EarthOrientationField,
    },
    /// Fewer than two samples were supplied.
    #[error("Earth-orientation interpolation requires at least two samples, got {actual}")]
    InsufficientSamples {
        /// Supplied sample count.
        actual: usize,
    },
    /// Samples were not strictly increasing.
    #[error("Earth-orientation sample {index} does not follow a strictly earlier epoch")]
    NonIncreasingEpoch {
        /// Index of the invalid sample.
        index: usize,
    },
    /// The interpolation limit was zero or negative.
    #[error("maximum Earth-orientation interpolation span must be positive")]
    NonPositiveMaximumInterpolationSpan,
    /// Artifact coverage did not exactly describe the sample interval.
    #[error(
        "Earth-orientation artifact coverage {declared:?} does not match sample interval [{sample_start}, {sample_end}]"
    )]
    CoverageMismatch {
        /// Coverage retained by the verified artifact.
        declared: ArtifactCoverage,
        /// First sample epoch.
        sample_start: Epoch,
        /// Last sample epoch.
        sample_end: Epoch,
    },
    /// Requested epoch was outside the verified artifact interval.
    #[error(transparent)]
    Coverage(#[from] CoverageError),
    /// Bracketing samples were farther apart than permitted.
    #[error(
        "Earth-orientation samples [{before}, {after}] exceed maximum interpolation span {maximum}"
    )]
    InterpolationGap {
        /// Earlier sample.
        before: Epoch,
        /// Later sample.
        after: Epoch,
        /// Configured maximum span.
        maximum: Duration,
    },
    /// The data interval was too short for the velocity derivative.
    #[error(
        "Earth-orientation coverage [{start}, {end}] cannot support the velocity derivative stencil at {epoch}"
    )]
    DerivativeStencilOutsideCoverage {
        /// Requested epoch.
        epoch: Epoch,
        /// First sample epoch.
        start: Epoch,
        /// Last sample epoch.
        end: Epoch,
    },
    /// Only GCRF and ITRF2020 are supported by this provider.
    #[error("IERS 2010 provider does not support frame pair {frames:?}")]
    UnsupportedTransform {
        /// Input and requested output frame, respectively.
        frames: Box<(ReferenceFrame, ReferenceFrame)>,
    },
    /// Numerical output was not finite.
    #[error(transparent)]
    InvalidKinematics(#[from] FrameKinematicsError),
}

type Matrix3 = [[f64; 3]; 3];

fn interpolate(before: f64, after: f64, fraction: f64) -> f64 {
    (after - before).mul_add(fraction, before)
}

fn split_julian_date(mjd: Duration, offset_seconds: f64) -> (f64, f64) {
    let whole_days = mjd.floor(Unit::Day * 1.0);
    (
        JULIAN_DATE_MJD_ORIGIN + whole_days.to_unit(Unit::Day),
        (mjd - whole_days).to_seconds() / SECONDS_PER_DAY + offset_seconds / SECONDS_PER_DAY,
    )
}

fn multiply(matrix: Matrix3, vector: [f64; 3]) -> [f64; 3] {
    matrix.map(|row| row[0].mul_add(vector[0], row[1].mul_add(vector[1], row[2] * vector[2])))
}

fn transpose(matrix: Matrix3) -> Matrix3 {
    [
        [matrix[0][0], matrix[1][0], matrix[2][0]],
        [matrix[0][1], matrix[1][1], matrix[2][1]],
        [matrix[0][2], matrix[1][2], matrix[2][2]],
    ]
}

fn add(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn subtract(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn subtract_matrix(left: Matrix3, right: Matrix3) -> Matrix3 {
    std::array::from_fn(|row| std::array::from_fn(|column| left[row][column] - right[row][column]))
}

fn scale_matrix(matrix: Matrix3, scale: f64) -> Matrix3 {
    matrix.map(|row| row.map(|value| value * scale))
}

fn linear_combination<const N: usize>(terms: [(f64, Matrix3); N]) -> Matrix3 {
    std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            terms
                .iter()
                .map(|(factor, matrix)| factor * matrix[row][column])
                .sum()
        })
    })
}

#[cfg(test)]
mod tests {
    use hifitime::Duration;
    use orskit_data::{ArtifactCoverage, ArtifactDescriptor, Sha256Digest, TimeCoverage};
    use units::uom::si::{angle::radian, time::second};

    use super::*;

    const SOFA_TT_JULIAN_DATE: f64 = 2_453_736.5;
    const XP_RADIANS: f64 = 2.550_602_38e-7;
    const YP_RADIANS: f64 = 1.860_359_247e-6;

    fn reference_epoch() -> Epoch {
        Epoch::from_tt_seconds(
            (SOFA_TT_JULIAN_DATE - JULIAN_DATE_MJD_ORIGIN - hifitime::MJD_J1900) * SECONDS_PER_DAY,
        )
    }

    fn sample(epoch: Epoch) -> EarthOrientationSample {
        EarthOrientationSample::new(
            epoch,
            Time::new::<second>(32.184),
            Angle::new::<radian>(XP_RADIANS),
            Angle::new::<radian>(YP_RADIANS),
        )
        .expect("finite sample")
    }

    fn artifact(start: Epoch, end: Epoch) -> VerifiedArtifact {
        let bytes = b"project-authored normalized EOP fixture".to_vec();
        let descriptor = ArtifactDescriptor::new(
            "test",
            "normalized EOP fixture",
            "1",
            Sha256Digest::compute(&bytes),
            ArtifactCoverage::Interval(
                TimeCoverage::new(start, end).expect("ordered test coverage"),
            ),
        )
        .expect("complete descriptor");
        VerifiedArtifact::from_bytes(descriptor, bytes).expect("matching digest")
    }

    fn provider_with_span(
        sample_spacing: Duration,
        maximum_interpolation_span: Duration,
    ) -> Iers2010EarthOrientation {
        let epoch = reference_epoch();
        let start = epoch - sample_spacing;
        let end = epoch + sample_spacing;
        Iers2010EarthOrientation::new(
            artifact(start, end),
            vec![sample(start), sample(end)],
            maximum_interpolation_span,
        )
        .expect("valid provider")
    }

    fn provider() -> Iers2010EarthOrientation {
        provider_with_span(Duration::from_days(1.0), Duration::from_days(2.0))
    }

    fn assert_vector_close(actual: [f64; 3], expected: [f64; 3], tolerance: f64) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() <= tolerance,
                "{actual:.16e} differs from {expected:.16e} by more than {tolerance:.3e}"
            );
        }
    }

    #[test]
    fn position_matches_the_authoritative_sofa_validation_result() {
        // The input date, EOP values, and expected celestial-to-terrestrial
        // direction are published by the IAU SOFA Board's 2023-10-11 C
        // validation release for the IAU 2006/2000A transformation. Only the
        // numerical standard vector is used; no validation source code is
        // copied.
        let transformed = provider()
            .transform_kinematics(
                reference_epoch(),
                FrameKinematics::new(
                    Position::from_metres(1.0, 0.0, 0.0),
                    VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
                    ReferenceFrame::GCRF,
                )
                .expect("finite input"),
                ReferenceFrame::ITRF2020,
            )
            .expect("covered transform");

        assert_vector_close(
            transformed.position().to_metres(),
            [
                -0.181_033_212_830_589_73,
                -0.983_476_813_413_621_5,
                0.000_577_347_402_474_854_6,
            ],
            2.0e-10,
        );
    }

    #[test]
    fn inverse_composition_preserves_position_and_velocity() {
        let provider = provider();
        let epoch = reference_epoch();
        let initial = FrameKinematics::new(
            Position::from_metres(7_000_000.0, -1_200_000.0, 800_000.0),
            VelocityVector::from_metres_per_second(1_000.0, 7_300.0, -900.0),
            ReferenceFrame::GCRF,
        )
        .expect("finite input");

        let terrestrial = provider
            .transform_kinematics(epoch, initial, ReferenceFrame::ITRF2020)
            .expect("forward transform");
        let restored = provider
            .transform_kinematics(epoch, terrestrial, ReferenceFrame::GCRF)
            .expect("inverse transform");

        assert_vector_close(
            restored.position().to_metres(),
            initial.position().to_metres(),
            3.0e-9,
        );
        assert_vector_close(
            restored.velocity().to_metres_per_second(),
            initial.velocity().to_metres_per_second(),
            3.0e-12,
        );
    }

    #[test]
    fn transformed_velocity_is_the_time_derivative_of_position() {
        let provider = provider();
        let epoch = reference_epoch();
        let input = FrameKinematics::new(
            Position::from_metres(6_378_137.0, 1_000_000.0, -500_000.0),
            VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
            ReferenceFrame::GCRF,
        )
        .expect("finite input");
        let half_span = Duration::from_seconds(0.25);

        let before = provider
            .transform_kinematics(epoch - half_span, input, ReferenceFrame::ITRF2020)
            .expect("before");
        let after = provider
            .transform_kinematics(epoch + half_span, input, ReferenceFrame::ITRF2020)
            .expect("after");
        let current = provider
            .transform_kinematics(epoch, input, ReferenceFrame::ITRF2020)
            .expect("current");
        let before = before.position().to_metres();
        let after = after.position().to_metres();
        let finite_difference = std::array::from_fn(|index| (after[index] - before[index]) / 0.5);

        assert_vector_close(
            current.velocity().to_metres_per_second(),
            finite_difference,
            2.0e-6,
        );
    }

    #[test]
    fn coverage_and_interpolation_failures_remain_distinct() {
        let provider = provider();
        let input = FrameKinematics::new(
            Position::from_metres(1.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
            ReferenceFrame::GCRF,
        )
        .expect("finite input");
        let outside = provider
            .transform_kinematics(
                reference_epoch() + Duration::from_days(2.0),
                input,
                ReferenceFrame::ITRF2020,
            )
            .expect_err("outside coverage");
        assert!(matches!(outside, EarthOrientationError::Coverage(_)));

        let gap_provider = provider_with_span(Duration::from_days(1.0), Duration::from_hours(12.0));
        assert!(matches!(
            gap_provider.transform_kinematics(reference_epoch(), input, ReferenceFrame::ITRF2020,),
            Err(EarthOrientationError::InterpolationGap { .. })
        ));
    }

    #[test]
    fn construction_and_derivative_failures_are_explicit() {
        assert!(matches!(
            EarthOrientationSample::new(
                reference_epoch(),
                Time::new::<second>(f64::NAN),
                Angle::new::<radian>(0.0),
                Angle::new::<radian>(0.0),
            ),
            Err(EarthOrientationError::NonFiniteSample {
                field: EarthOrientationField::Ut1MinusTai
            })
        ));

        let start = reference_epoch() - Duration::from_seconds(1.0);
        let end = reference_epoch() + Duration::from_seconds(1.0);
        assert!(matches!(
            Iers2010EarthOrientation::new(
                artifact(start, end),
                vec![sample(start)],
                Duration::from_seconds(1.0),
            ),
            Err(EarthOrientationError::InsufficientSamples { actual: 1 })
        ));
        assert!(matches!(
            Iers2010EarthOrientation::new(
                artifact(start, end),
                vec![sample(end), sample(start)],
                Duration::from_seconds(2.0),
            ),
            Err(EarthOrientationError::NonIncreasingEpoch { index: 1 })
        ));
        assert!(matches!(
            Iers2010EarthOrientation::new(
                artifact(start, end),
                vec![sample(start), sample(end)],
                Duration::ZERO,
            ),
            Err(EarthOrientationError::NonPositiveMaximumInterpolationSpan)
        ));
        assert!(matches!(
            Iers2010EarthOrientation::new(
                artifact(start, end),
                vec![sample(start), sample(reference_epoch())],
                Duration::from_seconds(2.0),
            ),
            Err(EarthOrientationError::CoverageMismatch { .. })
        ));

        let provider =
            provider_with_span(Duration::from_seconds(0.25), Duration::from_seconds(0.5));
        let input = FrameKinematics::new(
            Position::from_metres(1.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
            ReferenceFrame::GCRF,
        )
        .expect("finite input");
        assert!(matches!(
            provider.transform_kinematics(reference_epoch(), input, ReferenceFrame::ITRF2020,),
            Err(EarthOrientationError::DerivativeStencilOutsideCoverage { .. })
        ));
    }

    #[test]
    fn unsupported_frames_are_not_relabelled() {
        let input = FrameKinematics::new(
            Position::from_metres(1.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
            ReferenceFrame::EME2000,
        )
        .expect("finite input");
        assert!(matches!(
            provider().transform_kinematics(reference_epoch(), input, ReferenceFrame::ITRF2020,),
            Err(EarthOrientationError::UnsupportedTransform { frames })
                if *frames == (ReferenceFrame::EME2000, ReferenceFrame::ITRF2020)
        ));
    }
}
