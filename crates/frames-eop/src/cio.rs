use std::f64::consts::{PI, TAU};

use frames::{
    BodyFixedTransform, BodyFixedTransformDirection, BodyFixedTransformProvider,
    BodyFixedTransformProviderError, BodyFixedTransformRequest, DirectionCosineMatrix,
    InertialFrame, ProvenancedBodyFixedTransform, ReferenceDataDescriptor, ReferenceFrame,
};
use hifitime::{Epoch, TimeScale};
use thiserror::Error;
use units::uom::si::{angle::radian, time::second};
use units::{Angle, AngularVelocityVector, Time};

use crate::ProvenanceField;

const SECONDS_PER_DAY: f64 = 86_400.0;
const NANOSECONDS_PER_DAY: i128 = 86_400_000_000_000;
const SECONDS_PER_CENTURY: f64 = 36_525.0 * SECONDS_PER_DAY;
const ARCSEC_TO_RAD: f64 = PI / (180.0 * 3_600.0);
const MICROARCSEC_TO_RAD: f64 = ARCSEC_TO_RAD * 1.0e-6;
const ERA_REFERENCE_FRACTION: f64 = 0.779_057_273_264_0;
const ERA_EXCESS_TURNS_PER_DAY: f64 = 0.002_737_811_911_354_48;
const ERA_TURNS_PER_DAY: f64 = 1.0 + ERA_EXCESS_TURNS_PER_DAY;

const X_TABLE: &str = include_str!("../data/iers-2010/iau06xtab5.2.a.dat");
const Y_TABLE: &str = include_str!("../data/iers-2010/iau06ytab5.2.b.dat");
const S_TABLE: &str = include_str!("../data/iers-2010/iau06stab5.2.d.dat");

/// One caller-owned sample of the Earth-orientation parameters used by the
/// IAU 2006/2000A CIO transformation.
///
/// Angular fields are typed SI angles in radians. Samples use UTC epochs and
/// UT1−UTC; the provider interpolates the continuous UT1−TAI offset and the
/// angular fields in elapsed TAI seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CioEarthOrientationSample {
    /// UTC epoch at which the Earth-orientation values apply.
    pub epoch_utc: Epoch,
    /// UT1−UTC at this epoch.
    pub ut1_minus_utc: Time,
    /// Celestial pole offset dX from the IAU 2006/2000A model.
    pub d_x: Angle,
    /// Celestial pole offset dY from the IAU 2006/2000A model.
    pub d_y: Angle,
    /// IERS polar-motion coordinate xp.
    pub xp: Angle,
    /// IERS polar-motion coordinate yp.
    pub yp: Angle,
}

/// IAU 2006/2000A CIO-based Earth-orientation provider.
///
/// The provider evaluates the full CIO-based GCRF↔ITRF2020 rotation using the
/// IERS 2010 X, Y, and s+XY/2 tables, caller-supplied dX/dY and xp/yp, ERA,
/// and the TIO locator s'. Samples are immutable, versioned by caller-supplied
/// provenance, and define closed coverage; extrapolation is rejected. No EOP
/// time series is bundled or fetched.
#[derive(Debug)]
pub struct Iau2006CioProvider {
    samples: Vec<CioEarthOrientationSample>,
    reference_data: Vec<ReferenceDataDescriptor>,
    model: CioSeries,
}

impl Iau2006CioProvider {
    /// Validates caller-supplied EOP samples and their provenance.
    pub fn new(
        samples: Vec<CioEarthOrientationSample>,
        authority: impl Into<String>,
        product: impl Into<String>,
        revision: impl Into<String>,
        checksum: Option<&str>,
    ) -> Result<Self, Iau2006CioError> {
        if samples.len() < 2 {
            return Err(Iau2006CioError::InsufficientSamples {
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
                return Err(Iau2006CioError::BeforeLeapSecondEra { sample: index });
            }
            if !sample.ut1_minus_utc.get::<second>().is_finite() {
                return Err(Iau2006CioError::NonFiniteSample {
                    sample: index,
                    field: CioEopField::Ut1MinusUtc,
                });
            }
            for (field, value) in [
                (CioEopField::Dx, sample.d_x.get::<radian>()),
                (CioEopField::Dy, sample.d_y.get::<radian>()),
                (CioEopField::Xp, sample.xp.get::<radian>()),
                (CioEopField::Yp, sample.yp.get::<radian>()),
            ] {
                if !value.is_finite() {
                    return Err(Iau2006CioError::NonFiniteSample {
                        sample: index,
                        field,
                    });
                }
            }
            if index > 0 && sample.epoch_utc <= samples[index - 1].epoch_utc {
                return Err(Iau2006CioError::NonChronologicalSamples { sample: index });
            }
        }

        let eop = ReferenceDataDescriptor {
            authority: authority.into(),
            product: product.into(),
            revision: revision.into(),
            checksum: checksum.map(str::to_owned),
        };
        let invalid_field = if eop.authority.trim().is_empty() {
            Some(ProvenanceField::Authority)
        } else if eop.product.trim().is_empty() {
            Some(ProvenanceField::Product)
        } else if eop.revision.trim().is_empty() {
            Some(ProvenanceField::Revision)
        } else if eop
            .checksum
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            Some(ProvenanceField::Checksum)
        } else {
            None
        };
        if let Some(field) = invalid_field {
            return Err(Iau2006CioError::InvalidProvenance { field });
        }

        Ok(Self {
            samples,
            reference_data: vec![
                eop,
                descriptor(
                    "International Earth Rotation and Reference Systems Service",
                    "IERS Conventions 2010 Tables 5.2a and 5.2b: IAU 2006/2000A_R06 X and Y",
                    "Technical Note 36 (2010)",
                    Some("sha256:7e1b0933afb3aa5c4e1e34e72d0592f215bad6e6afd919bab2fe6312b86e02ee;sha256:56be4e178fd6aa07a5871bdec6cfa5bd6574320f7eb50475775d3817d1d9701c"),
                ),
                descriptor(
                    "International Earth Rotation and Reference Systems Service",
                    "IERS Conventions 2010 Table 5.2d: CIO locator s+XY/2",
                    "Technical Note 36 (2010)",
                    Some("sha256:8f247a429ed0a82dbcb3b6a326c7f24ec9e28f4f00dedcda01d9f9f0841b024b"),
                ),
                descriptor(
                    "International Astronomical Union",
                    "IAU 2000 Earth Rotation Angle; IERS Conventions 2010 Chapter 5",
                    "IAU 2000; IERS TN 36 (2010)",
                    None,
                ),
                descriptor(
                    "Hifitime",
                    "UTC-to-TAI leap-second table",
                    "4.3.0",
                    None,
                ),
            ],
            model: CioSeries::from_iers_tables(),
        })
    }

    /// Returns the immutable EOP, IERS-series, convention, and time-scale
    /// provenance records used by this provider.
    #[must_use]
    pub fn reference_data(&self) -> &[ReferenceDataDescriptor] {
        &self.reference_data
    }

    /// Resolves a GCRF↔ITRF2020 request with its selected provenance.
    pub fn transform(
        &self,
        request: BodyFixedTransformRequest,
    ) -> Result<ProvenancedBodyFixedTransform<'_>, Iau2006CioError> {
        if request.inertial_frame() != InertialFrame::GCRF
            || request.body_fixed_frame() != ReferenceFrame::ITRF2020
        {
            return Err(Iau2006CioError::UnsupportedFramePair {
                request: Box::new(request),
            });
        }
        let epoch_tai = request.epoch().to_time_scale(TimeScale::TAI);
        let eop = self.interpolate(epoch_tai, request)?;
        let epoch_tt = request.epoch().to_time_scale(TimeScale::TT);
        let centuries = (epoch_tt - Epoch::from_gregorian(2000, 1, 1, 12, 0, 0, 0, TimeScale::TT))
            .to_seconds()
            / SECONDS_PER_CENTURY;
        let celestial = self.model.evaluate(centuries);
        let x = celestial.x + eop.dx;
        let y = celestial.y + eop.dy;
        let x_rate = celestial.x_rate + eop.dx_rate;
        let y_rate = celestial.y_rate + eop.dy_rate;
        let s = celestial.s;
        let s_rate = celestial.s_rate;
        let era = earth_rotation_angle(epoch_tai, eop.ut1_minus_tai);
        let era_rate = TAU * ERA_TURNS_PER_DAY / SECONDS_PER_DAY * (1.0 + eop.ut1_minus_tai_rate);
        let tio_locator = -47.0 * MICROARCSEC_TO_RAD * centuries;
        let tio_locator_rate = -47.0 * MICROARCSEC_TO_RAD / SECONDS_PER_CENTURY;

        let q = celestial_intermediate_matrix(x, y, s, x_rate, y_rate, s_rate);
        let r = rotation_z(-era, -era_rate);
        let w = polar_motion_matrix(
            eop.xp,
            eop.yp,
            tio_locator,
            eop.xp_rate,
            eop.yp_rate,
            tio_locator_rate,
        );
        let direct = w.multiply(r).multiply(q);
        let omega_matrix = direct.derivative.multiply(direct.value.transpose());
        let omega = AngularVelocityVector::from_radians_per_second(
            omega_matrix.0[1][2],
            omega_matrix.0[2][0],
            omega_matrix.0[0][1],
        );
        let (rotation, angular_velocity) = match request.direction() {
            BodyFixedTransformDirection::InertialToBodyFixed => (direct.value, omega),
            BodyFixedTransformDirection::BodyFixedToInertial => (direct.value.transpose(), omega),
        };
        let rotation =
            DirectionCosineMatrix::new(rotation.0).map_err(Iau2006CioError::InvalidRotation)?;
        let transform = BodyFixedTransform::new(request, rotation, angular_velocity)
            .map_err(Iau2006CioError::InvalidTransform)?;
        Ok(ProvenancedBodyFixedTransform::new(
            transform,
            &self.reference_data,
        ))
    }

    fn interpolate(
        &self,
        epoch_tai: Epoch,
        request: BodyFixedTransformRequest,
    ) -> Result<InterpolatedEop, Iau2006CioError> {
        let start = self.samples[0].epoch_utc.to_time_scale(TimeScale::TAI);
        let end = self.samples[self.samples.len() - 1]
            .epoch_utc
            .to_time_scale(TimeScale::TAI);
        if epoch_tai < start || epoch_tai > end {
            return Err(Iau2006CioError::EpochOutOfRange {
                request: Box::new(request),
                start,
                end,
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
        let left_tai = left.epoch_utc.to_time_scale(TimeScale::TAI);
        let right_tai = right.epoch_utc.to_time_scale(TimeScale::TAI);
        let interval = (right_tai - left_tai).to_seconds();
        let fraction = (epoch_tai - left_tai).to_seconds() / interval;
        let left_ut1_tai =
            left.ut1_minus_utc.get::<second>() - f64::from(left.epoch_utc.leap_seconds_iers());
        let right_ut1_tai =
            right.ut1_minus_utc.get::<second>() - f64::from(right.epoch_utc.leap_seconds_iers());
        let (ut1_minus_tai, ut1_rate) =
            interpolate_scalar(left_ut1_tai, right_ut1_tai, fraction, interval);
        let (dx, dx_rate) = interpolate_angle(left.d_x, right.d_x, fraction, interval);
        let (dy, dy_rate) = interpolate_angle(left.d_y, right.d_y, fraction, interval);
        let (xp, xp_rate) = interpolate_angle(left.xp, right.xp, fraction, interval);
        let (yp, yp_rate) = interpolate_angle(left.yp, right.yp, fraction, interval);
        if [
            interval,
            fraction,
            ut1_minus_tai,
            ut1_rate,
            dx,
            dx_rate,
            dy,
            dy_rate,
            xp,
            xp_rate,
            yp,
            yp_rate,
        ]
        .into_iter()
        .any(|value| !value.is_finite())
            || interval <= 0.0
        {
            return Err(Iau2006CioError::NonFiniteEvaluation {
                request: Box::new(request),
            });
        }
        Ok(InterpolatedEop {
            ut1_minus_tai,
            ut1_minus_tai_rate: ut1_rate,
            dx,
            dx_rate,
            dy,
            dy_rate,
            xp,
            xp_rate,
            yp,
            yp_rate,
        })
    }
}

impl BodyFixedTransformProvider for Iau2006CioProvider {
    fn reference_data(&self) -> &[ReferenceDataDescriptor] {
        &self.reference_data
    }

    fn body_fixed_transform(
        &self,
        request: BodyFixedTransformRequest,
    ) -> Result<BodyFixedTransform, BodyFixedTransformProviderError> {
        self.transform(request)
            .map(ProvenancedBodyFixedTransform::transform)
            .map_err(|source| match source {
                Iau2006CioError::UnsupportedFramePair { .. } => {
                    BodyFixedTransformProviderError::UnsupportedRequest {
                        request: Box::new(request),
                    }
                }
                Iau2006CioError::EpochOutOfRange { .. } => {
                    BodyFixedTransformProviderError::EpochOutOfRange {
                        epoch: request.epoch(),
                        time_scale: request.time_scale(),
                    }
                }
                Iau2006CioError::InvalidRotation(source)
                | Iau2006CioError::InvalidTransform(source) => {
                    BodyFixedTransformProviderError::InvalidTransform {
                        source: Box::new(source),
                    }
                }
                source => BodyFixedTransformProviderError::Source {
                    source: Box::new(source),
                },
            })
    }
}

/// Typed failures from the IAU 2006/2000A CIO Earth-orientation provider.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Iau2006CioError {
    /// At least two records are required for interpolation and coverage.
    #[error("Earth-orientation data require at least two samples, got {actual}")]
    InsufficientSamples { actual: usize },
    /// An input record predates the supported leap-second era.
    #[error("Earth-orientation sample {sample} predates 1972-01-01 UTC")]
    BeforeLeapSecondEra { sample: usize },
    /// A sample contains a non-finite EOP quantity.
    #[error("Earth-orientation sample {sample} has non-finite {field}")]
    NonFiniteSample { sample: usize, field: CioEopField },
    /// Input records are not strictly chronological.
    #[error("Earth-orientation sample {sample} is not strictly later than its predecessor")]
    NonChronologicalSamples { sample: usize },
    /// A required provenance identity is blank.
    #[error("Earth-orientation provenance has an empty {field}")]
    InvalidProvenance { field: ProvenanceField },
    /// The requested frame identities are not GCRF and ITRF2020.
    #[error("CIO provider does not support requested frame pair in {request:?}")]
    UnsupportedFramePair {
        request: Box<BodyFixedTransformRequest>,
    },
    /// The epoch is outside the inclusive sample coverage.
    #[error("request {request:?} is outside Earth-orientation coverage [{start:?}, {end:?}]")]
    EpochOutOfRange {
        request: Box<BodyFixedTransformRequest>,
        start: Epoch,
        end: Epoch,
    },
    /// A numerical intermediate is not finite.
    #[error("Earth-orientation evaluation for {request:?} produced a non-finite value")]
    NonFiniteEvaluation {
        request: Box<BodyFixedTransformRequest>,
    },
    /// The orientation matrix failed direction-cosine validation.
    #[error("CIO provider produced an invalid rotation matrix")]
    InvalidRotation(#[source] frames::BodyFixedTransformError),
    /// The complete transform failed frame-layer validation.
    #[error("CIO provider produced an invalid body-fixed transform")]
    InvalidTransform(#[source] frames::BodyFixedTransformError),
}

/// Earth-orientation sample field that failed validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CioEopField {
    Ut1MinusUtc,
    Dx,
    Dy,
    Xp,
    Yp,
}

impl std::fmt::Display for CioEopField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Ut1MinusUtc => "UT1−UTC",
            Self::Dx => "dX",
            Self::Dy => "dY",
            Self::Xp => "xp",
            Self::Yp => "yp",
        })
    }
}

fn descriptor(
    authority: &str,
    product: &str,
    revision: &str,
    checksum: Option<&str>,
) -> ReferenceDataDescriptor {
    ReferenceDataDescriptor {
        authority: authority.to_owned(),
        product: product.to_owned(),
        revision: revision.to_owned(),
        checksum: checksum.map(str::to_owned),
    }
}

fn interpolate_scalar(left: f64, right: f64, fraction: f64, interval: f64) -> (f64, f64) {
    let difference = right - left;
    (difference.mul_add(fraction, left), difference / interval)
}

fn interpolate_angle(left: Angle, right: Angle, fraction: f64, interval: f64) -> (f64, f64) {
    interpolate_scalar(
        left.get::<radian>(),
        right.get::<radian>(),
        fraction,
        interval,
    )
}

fn earth_rotation_angle(epoch_tai: Epoch, ut1_minus_tai: f64) -> f64 {
    let tai_nanoseconds =
        (epoch_tai - Epoch::from_gregorian_tai(2000, 1, 1, 12, 0, 0, 0)).total_nanoseconds();
    let days = tai_nanoseconds.div_euclid(NANOSECONDS_PER_DAY) as f64;
    let subday = tai_nanoseconds.rem_euclid(NANOSECONDS_PER_DAY) as f64 / 1.0e9 + ut1_minus_tai;
    let turns = ERA_REFERENCE_FRACTION
        + (ERA_EXCESS_TURNS_PER_DAY * days).rem_euclid(1.0)
        + ERA_TURNS_PER_DAY * subday / SECONDS_PER_DAY;
    TAU * turns.rem_euclid(1.0)
}

#[derive(Debug, Clone, Copy)]
struct Term {
    sine: f64,
    cosine: f64,
    arguments: [i8; 14],
}

fn read_table(source: &str, expected_terms: usize) -> Vec<Term> {
    let terms: Vec<_> = source
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
        .map(|line| {
            let values: Vec<_> = line
                .split_whitespace()
                .map(|value| value.parse::<f64>().expect("valid IERS table number"))
                .collect();
            assert_eq!(values.len(), 17, "valid IERS table row");
            let mut arguments = [0i8; 14];
            for (target, value) in arguments.iter_mut().zip(&values[3..]) {
                *target = *value as i8;
            }
            Term {
                sine: values[1],
                cosine: values[2],
                arguments,
            }
        })
        .collect();
    assert_eq!(
        terms.len(),
        expected_terms,
        "complete IERS coefficient table"
    );
    terms
}

#[derive(Debug)]
struct CioSeries {
    x: Vec<Term>,
    y: Vec<Term>,
    s: Vec<Term>,
}

impl CioSeries {
    fn from_iers_tables() -> Self {
        Self {
            x: read_table(X_TABLE, 1600),
            y: read_table(Y_TABLE, 1275),
            s: read_table(S_TABLE, 66),
        }
    }

    fn evaluate(&self, t: f64) -> CelestialPole {
        let arguments = fundamental_arguments(t);
        let mut x = polynomial(
            &[
                -0.016_617,
                2_004.191_898,
                -0.429_782_9,
                -0.198_618_34,
                0.000_007_578,
                0.000_005_928_5,
            ],
            t,
            ARCSEC_TO_RAD,
        );
        let mut y = polynomial(
            &[
                -0.006_951,
                -0.025_896,
                -22.407_274_7,
                0.001_900_59,
                0.001_112_526,
                0.000_000_135_8,
            ],
            t,
            ARCSEC_TO_RAD,
        );
        let mut s_plus_xy = polynomial(
            &[94.0, 3808.65, -122.68, -72_574.11, 27.98, 15.62],
            t,
            MICROARCSEC_TO_RAD,
        );
        add_series(&mut x, &self.x, &X_BLOCKS, t, &arguments);
        add_series(&mut y, &self.y, &Y_BLOCKS, t, &arguments);
        add_series(&mut s_plus_xy, &self.s, &S_BLOCKS, t, &arguments);
        let s = s_plus_xy.0 - x.0 * y.0 / 2.0;
        let s_rate = s_plus_xy.1 - (x.1 * y.0 + x.0 * y.1) / 2.0;
        CelestialPole {
            x: x.0,
            y: y.0,
            s,
            x_rate: x.1,
            y_rate: y.1,
            s_rate,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct CelestialPole {
    x: f64,
    y: f64,
    s: f64,
    x_rate: f64,
    y_rate: f64,
    s_rate: f64,
}

fn polynomial(coefficients: &[f64], t: f64, scale: f64) -> (f64, f64) {
    let mut value = 0.0_f64;
    let mut derivative = 0.0_f64;
    for coefficient in coefficients.iter().rev() {
        derivative = derivative.mul_add(t, value);
        value = value.mul_add(t, *coefficient);
    }
    (value * scale, derivative * scale / SECONDS_PER_CENTURY)
}

fn fundamental_arguments(t: f64) -> [(f64, f64); 14] {
    let arcsec = [
        [
            485_868.249_036,
            1_717_915_923.217_8,
            31.879_2,
            0.051_635,
            -0.000_244_70,
        ],
        [
            1_287_104.793_05,
            129_596_581.048_1,
            -0.553_2,
            0.000_136,
            -0.000_011_49,
        ],
        [
            335_779.526_232,
            1_739_527_262.847_8,
            -12.751_2,
            -0.001_037,
            0.000_004_17,
        ],
        [
            1_072_260.703_69,
            1_602_961_601.209_0,
            -6.370_6,
            0.006_593,
            -0.000_031_69,
        ],
        [
            450_160.398_036,
            -6_962_890.543_1,
            7.472_2,
            0.007_702,
            -0.000_059_39,
        ],
    ];
    let mut result = [(0.0, 0.0); 14];
    for (index, coefficients) in arcsec.iter().enumerate() {
        let (value, rate) = polynomial(coefficients, t, ARCSEC_TO_RAD);
        result[index] = (value.rem_euclid(TAU), rate);
    }
    let planetary = [
        [4.402_608_842, 2_608.790_314_157_4, 0.0],
        [3.176_146_697, 1_021.328_554_621_1, 0.0],
        [1.753_470_314, 628.307_584_999_1, 0.0],
        [6.203_480_913, 334.061_242_670_0, 0.0],
        [0.599_546_497, 52.969_096_264_1, 0.0],
        [0.874_016_757, 21.329_910_496_0, 0.0],
        [5.481_293_872, 7.478_159_856_7, 0.0],
        [5.311_886_287, 3.813_303_563_8, 0.0],
        [0.0, 0.024_381_750, 0.000_005_386_91],
    ];
    for (offset, coefficients) in planetary.iter().enumerate() {
        let (value, rate) = polynomial(coefficients, t, 1.0);
        result[offset + 5] = (value.rem_euclid(TAU), rate);
    }
    result
}

/// Term counts of the t^j blocks (j = 0..=4) of IERS Conventions (2010) Tables 5.2a, 5.2b and 5.2d.
const X_BLOCKS: [usize; 5] = [1306, 253, 36, 4, 1];
const Y_BLOCKS: [usize; 5] = [962, 277, 30, 5, 1];
const S_BLOCKS: [usize; 5] = [33, 3, 25, 4, 1];

fn add_series(
    sum: &mut (f64, f64),
    terms: &[Term],
    blocks: &[usize; 5],
    t: f64,
    arguments: &[(f64, f64); 14],
) {
    let (mut value, mut value_correction) = (sum.0, 0.0);
    let (mut rate, mut rate_correction) = (sum.1, 0.0);
    let mut power = 0usize;
    let mut remaining = *blocks.first().expect("block table");
    for term in terms {
        while remaining == 0 {
            power += 1;
            remaining = blocks[power];
        }
        remaining -= 1;
        let (phase, phase_rate) = term.arguments.iter().zip(arguments).fold(
            (0.0, 0.0),
            |(phase, rate), (multiplier, (argument, argument_rate))| {
                (
                    phase + f64::from(*multiplier) * argument,
                    rate + f64::from(*multiplier) * argument_rate,
                )
            },
        );
        let (sin_phase, cos_phase) = phase.sin_cos();
        let amplitude = (term.sine * sin_phase + term.cosine * cos_phase) * MICROARCSEC_TO_RAD;
        let amplitude_rate =
            (term.sine * cos_phase - term.cosine * sin_phase) * phase_rate * MICROARCSEC_TO_RAD;
        let t_power = t.powi(power as i32);
        let term_value = amplitude * t_power;
        let term_rate = amplitude_rate * t_power
            + if power == 0 {
                0.0
            } else {
                amplitude * power as f64 * t.powi(power as i32 - 1) / SECONDS_PER_CENTURY
            };
        kahan_add(&mut value, &mut value_correction, term_value);
        kahan_add(&mut rate, &mut rate_correction, term_rate);
    }
    *sum = (value, rate);
}

fn kahan_add(sum: &mut f64, correction: &mut f64, value: f64) {
    let adjusted = value - *correction;
    let next = *sum + adjusted;
    *correction = (next - *sum) - adjusted;
    *sum = next;
}

#[derive(Debug, Clone, Copy)]
struct InterpolatedEop {
    ut1_minus_tai: f64,
    ut1_minus_tai_rate: f64,
    dx: f64,
    dx_rate: f64,
    dy: f64,
    dy_rate: f64,
    xp: f64,
    xp_rate: f64,
    yp: f64,
    yp_rate: f64,
}

#[derive(Debug, Clone, Copy)]
struct Matrix([[f64; 3]; 3]);

impl Matrix {
    fn multiply(self, right: Self) -> Self {
        let mut result = [[0.0; 3]; 3];
        for (row, result_row) in result.iter_mut().enumerate() {
            for (column, element) in result_row.iter_mut().enumerate() {
                *element = (0..3)
                    .map(|index| self.0[row][index] * right.0[index][column])
                    .sum();
            }
        }
        Self(result)
    }

    fn transpose(self) -> Self {
        Self([
            [self.0[0][0], self.0[1][0], self.0[2][0]],
            [self.0[0][1], self.0[1][1], self.0[2][1]],
            [self.0[0][2], self.0[1][2], self.0[2][2]],
        ])
    }
}

#[derive(Debug, Clone, Copy)]
struct MatrixRate {
    value: Matrix,
    derivative: Matrix,
}

impl MatrixRate {
    fn multiply(self, right: Self) -> Self {
        Self {
            value: self.value.multiply(right.value),
            derivative: Matrix(
                self.derivative
                    .multiply(right.value)
                    .0
                    .into_iter()
                    .zip(self.value.multiply(right.derivative).0)
                    .map(|(left, right)| std::array::from_fn(|i| left[i] + right[i]))
                    .collect::<Vec<_>>()
                    .try_into()
                    .expect("3x3 derivative matrix"),
            ),
        }
    }
}

fn rotation_x(angle: f64, rate: f64) -> MatrixRate {
    let (sin, cos) = angle.sin_cos();
    MatrixRate {
        value: Matrix([[1.0, 0.0, 0.0], [0.0, cos, -sin], [0.0, sin, cos]]),
        derivative: Matrix([
            [0.0, 0.0, 0.0],
            [0.0, -sin * rate, -cos * rate],
            [0.0, cos * rate, -sin * rate],
        ]),
    }
}

fn rotation_y(angle: f64, rate: f64) -> MatrixRate {
    let (sin, cos) = angle.sin_cos();
    MatrixRate {
        value: Matrix([[cos, 0.0, sin], [0.0, 1.0, 0.0], [-sin, 0.0, cos]]),
        derivative: Matrix([
            [-sin * rate, 0.0, cos * rate],
            [0.0, 0.0, 0.0],
            [-cos * rate, 0.0, -sin * rate],
        ]),
    }
}

fn rotation_z(angle: f64, rate: f64) -> MatrixRate {
    let (sin, cos) = angle.sin_cos();
    MatrixRate {
        value: Matrix([[cos, -sin, 0.0], [sin, cos, 0.0], [0.0, 0.0, 1.0]]),
        derivative: Matrix([
            [-sin * rate, -cos * rate, 0.0],
            [cos * rate, -sin * rate, 0.0],
            [0.0, 0.0, 0.0],
        ]),
    }
}

fn celestial_intermediate_matrix(
    x: f64,
    y: f64,
    s: f64,
    x_rate: f64,
    y_rate: f64,
    s_rate: f64,
) -> MatrixRate {
    let z = (1.0 - x * x - y * y).sqrt();
    let a = 1.0 / (1.0 + z);
    let z_rate = -(x * x_rate + y * y_rate) / z;
    let a_rate = -z_rate / ((1.0 + z) * (1.0 + z));
    let intermediate = MatrixRate {
        value: Matrix([
            [1.0 - a * x * x, -a * x * y, -x],
            [-a * x * y, 1.0 - a * y * y, -y],
            [x, y, 1.0 - a * (x * x + y * y)],
        ]),
        derivative: Matrix([
            [
                -a_rate * x * x - 2.0 * a * x * x_rate,
                -a_rate * x * y - a * (x_rate * y + x * y_rate),
                -x_rate,
            ],
            [
                -a_rate * x * y - a * (x_rate * y + x * y_rate),
                -a_rate * y * y - 2.0 * a * y * y_rate,
                -y_rate,
            ],
            [
                x_rate,
                y_rate,
                -a_rate * (x * x + y * y) - 2.0 * a * (x * x_rate + y * y_rate),
            ],
        ]),
    };
    rotation_z(s, s_rate).multiply(intermediate)
}

fn polar_motion_matrix(
    xp: f64,
    yp: f64,
    s_prime: f64,
    xp_rate: f64,
    yp_rate: f64,
    s_prime_rate: f64,
) -> MatrixRate {
    rotation_z(-s_prime, -s_prime_rate)
        .multiply(rotation_y(xp, xp_rate))
        .multiply(rotation_x(yp, yp_rate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use frames::{Body, FrameKinematics, FrameOrientation, FrameOrigin, ReferenceFrame};
    use hifitime::Duration;
    use units::{Position, VelocityVector};

    fn sample(
        epoch_utc: Epoch,
        dut1: f64,
        dx: f64,
        dy: f64,
        xp: f64,
        yp: f64,
    ) -> CioEarthOrientationSample {
        CioEarthOrientationSample {
            epoch_utc,
            ut1_minus_utc: Time::new::<second>(dut1),
            d_x: Angle::new::<radian>(dx),
            d_y: Angle::new::<radian>(dy),
            xp: Angle::new::<radian>(xp),
            yp: Angle::new::<radian>(yp),
        }
    }

    fn provider() -> Iau2006CioProvider {
        let start = Epoch::from_gregorian_utc(2025, 1, 1, 0, 0, 0, 0);
        Iau2006CioProvider::new(
            vec![
                sample(start, 0.1, 1.0e-9, -2.0e-9, 3.0e-7, -4.0e-7),
                sample(
                    start + Duration::from_seconds(86_400.0),
                    0.1,
                    1.0e-9,
                    -2.0e-9,
                    3.0e-7,
                    -4.0e-7,
                ),
            ],
            "IERS",
            "test EOP",
            "test revision",
            Some("sha256:test"),
        )
        .expect("valid full EOP samples")
    }

    fn request(epoch: Epoch, direction: BodyFixedTransformDirection) -> BodyFixedTransformRequest {
        BodyFixedTransformRequest::new(
            epoch,
            TimeScale::UTC,
            InertialFrame::GCRF,
            ReferenceFrame::ITRF2020,
            direction,
        )
        .expect("valid Earth-centered request")
    }

    #[test]
    fn full_cio_matrix_matches_independent_erfa_reference_vector() {
        let epoch = Epoch::from_gregorian_utc(2025, 1, 1, 12, 0, 0, 0);
        let provider = provider();
        let transform = provider
            .transform(request(
                epoch,
                BodyFixedTransformDirection::InertialToBodyFixed,
            ))
            .expect("covered GCRF to ITRF transform");
        let expected = [
            [
                0.192_049_635_106_953_28,
                -0.981_385_118_389_597_4,
                -0.000_432_502_858_153_379,
            ],
            [
                0.981_382_206_674_854_9,
                0.192_050_118_975_223_84,
                -0.002_390_862_521_080_701,
            ],
            [
                0.002_429_419_123_669_515,
                3.471_366_543_669_406e-5,
                0.999_997_048_354_485_4,
            ],
        ];
        for (actual_row, expected_row) in transform.rotation().rows().iter().zip(expected) {
            for (actual, expected) in actual_row.iter().zip(expected_row) {
                assert!(
                    (actual - expected).abs() < 1.0e-11,
                    "{actual} != {expected}"
                );
            }
        }
    }

    #[test]
    fn angular_velocity_matches_finite_difference_of_the_rotation() {
        let provider = provider();
        let epoch = Epoch::from_gregorian_utc(2025, 1, 1, 12, 0, 0, 0);
        let direction = BodyFixedTransformDirection::InertialToBodyFixed;
        let step = 0.5;
        let before = provider
            .transform(request(epoch - Duration::from_seconds(step), direction))
            .expect("earlier transform");
        let after = provider
            .transform(request(epoch + Duration::from_seconds(step), direction))
            .expect("later transform");
        let centre = provider
            .transform(request(epoch, direction))
            .expect("central transform");
        let (before, after, centre) = (
            before.rotation().rows(),
            after.rotation().rows(),
            centre.rotation().rows(),
        );
        let derivative =
            |row: usize, column: usize| (after[row][column] - before[row][column]) / (2.0 * step);
        let skew = |row: usize, column: usize| {
            (0..3)
                .map(|k| derivative(row, k) * centre[column][k])
                .sum::<f64>()
        };
        let expected = [skew(1, 2), skew(2, 0), skew(0, 1)];
        let actual = provider
            .transform(request(epoch, direction))
            .expect("central transform")
            .angular_velocity()
            .to_radians_per_second();
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() < 1.0e-13,
                "{actual} != {expected}"
            );
        }
    }
    #[test]
    fn direct_and_inverse_kinematics_round_trip_and_keep_provenance() {
        let provider = provider();
        let epoch = Epoch::from_gregorian_utc(2025, 1, 1, 12, 0, 0, 0);
        let inertial = FrameKinematics::new(
            Position::from_metres(6_900_000.0, -800_000.0, 1_100_000.0),
            VelocityVector::from_metres_per_second(900.0, 7_300.0, -300.0),
            ReferenceFrame::GCRF,
        )
        .expect("finite inertial state");
        let forward = provider
            .transform(request(
                epoch,
                BodyFixedTransformDirection::InertialToBodyFixed,
            ))
            .expect("forward transform");
        assert_eq!(forward.reference_data(), provider.reference_data());
        let body_fixed = forward
            .transform_kinematics(inertial)
            .expect("forward kinematics");
        let reverse = provider
            .transform(request(
                epoch,
                BodyFixedTransformDirection::BodyFixedToInertial,
            ))
            .expect("inverse transform")
            .transform_kinematics(body_fixed)
            .expect("inverse kinematics");
        for (actual, expected) in reverse
            .position()
            .to_metres()
            .into_iter()
            .zip(inertial.position().to_metres())
        {
            assert!((actual - expected).abs() < 2.0e-8);
        }
        for (actual, expected) in reverse
            .velocity()
            .to_metres_per_second()
            .into_iter()
            .zip(inertial.velocity().to_metres_per_second())
        {
            assert!((actual - expected).abs() < 2.0e-11);
        }
    }

    #[test]
    fn coverage_is_inclusive_and_provenance_or_frame_errors_are_typed() {
        let provider = provider();
        let first = provider.samples[0].epoch_utc;
        let last = provider.samples[1].epoch_utc;
        for epoch in [first, last] {
            provider
                .transform(request(
                    epoch,
                    BodyFixedTransformDirection::InertialToBodyFixed,
                ))
                .expect("coverage endpoints are included");
        }
        let outside = request(
            last + Duration::from_seconds(1.0e-9),
            BodyFixedTransformDirection::InertialToBodyFixed,
        );
        assert!(matches!(
            provider.transform(outside),
            Err(Iau2006CioError::EpochOutOfRange { .. })
        ));
        let wrong_frame = BodyFixedTransformRequest::new(
            first,
            TimeScale::UTC,
            InertialFrame::GCRF,
            ReferenceFrame::new(FrameOrigin::Body(Body::EARTH), FrameOrientation::Itrf(2014)),
            BodyFixedTransformDirection::InertialToBodyFixed,
        )
        .expect("same-origin ITRF request");
        assert!(matches!(
            provider.transform(wrong_frame),
            Err(Iau2006CioError::UnsupportedFramePair { .. })
        ));
        assert!(matches!(
            Iau2006CioProvider::new(
                vec![
                    sample(first, 0.0, 0.0, 0.0, 0.0, 0.0),
                    sample(last, 0.0, 0.0, 0.0, 0.0, 0.0),
                ],
                "IERS",
                "EOP",
                " ",
                None,
            ),
            Err(Iau2006CioError::InvalidProvenance {
                field: ProvenanceField::Revision
            })
        ));
    }

    #[test]
    fn eop_interpolation_rates_reach_the_angular_velocity() {
        let start = Epoch::from_gregorian_utc(2025, 1, 1, 0, 0, 0, 0);
        let provider = Iau2006CioProvider::new(
            vec![
                sample(start, -0.1, 0.0, 0.0, 0.0, 0.0),
                sample(
                    start + Duration::from_seconds(86_400.0),
                    0.1,
                    1.0e-6,
                    -2.0e-6,
                    3.0e-6,
                    -4.0e-6,
                ),
            ],
            "IERS",
            "changing EOP",
            "test",
            None,
        )
        .expect("valid samples");
        let transform = provider
            .transform(request(
                start + Duration::from_seconds(43_200.0),
                BodyFixedTransformDirection::InertialToBodyFixed,
            ))
            .expect("covered transform");
        assert!(transform.angular_velocity().is_finite());
        assert!(transform.angular_velocity().to_radians_per_second()[2] > 7.0e-5);
    }
}
