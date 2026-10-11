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

const J2000_JD: f64 = 2_451_545.0;
const CELESTIAL_RATE_STEP_SECONDS: f64 = 512.0;

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
/// unmodified `erfa` 0.2.1 IAU 2006/2000A model, caller-supplied dX/dY and xp/yp, ERA,
/// and the TIO locator s'. Samples are immutable, versioned by caller-supplied
/// provenance, and define closed coverage; extrapolation is rejected. No EOP
/// time series is bundled or fetched. Celestial rates use a sixth-order
/// centered derivative in TT; all other rates and the rotation-chain product
/// rule are analytic. See the Earth-orientation guide for numerical evidence
/// and the dependency's separate MPL-2.0 and ERFA license notices.
#[derive(Debug)]
pub struct Iau2006CioProvider {
    samples: Vec<CioEarthOrientationSample>,
    reference_data: Vec<ReferenceDataDescriptor>,
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
                    "Christopher H. Jordan; ERFA / NumFOCUS",
                    "erfa: IAU 2006/2000A CIP X, Y and CIO locator s",
                    "0.2.1; source 83190c4a59f61ca6e6e3958592d581ccae20481c",
                    Some("sha256:f63880def87bd7d612b89f9046f5db0961949417f88d831d24596eae555915e5"),
                ),
                descriptor(
                    "International Astronomical Union",
                    "IAU 2000 Earth Rotation Angle; IERS Conventions 2010 Chapter 5",
                    "IAU 2000; IERS TN 36 (2010)",
                    None,
                ),
                descriptor("Hifitime", "UTC-to-TAI leap-second table", "4.3.0", None),
            ],
        })
    }

    /// Returns the immutable EOP, ERFA dependency, convention, and time-scale
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
        let days = (epoch_tt - Epoch::from_gregorian(2000, 1, 1, 12, 0, 0, 0, TimeScale::TT))
            .to_seconds()
            / SECONDS_PER_DAY;
        let centuries = days / 36_525.0;
        let celestial = celestial_pole(days);
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

fn celestial_coordinates(days: f64) -> [f64; 3] {
    let (x, y) = erfa::prenut::bpn_to_xy(erfa::prenut::pn_matrix_06a(J2000_JD, days));
    let s = erfa::time::S06(J2000_JD, days, x, y);
    [x, y, s]
}

fn celestial_pole(days: f64) -> CelestialPole {
    let [x, y, s] = celestial_coordinates(days);
    let [x_rate, y_rate, s_rate] = celestial_rates(days, CELESTIAL_RATE_STEP_SECONDS);
    CelestialPole {
        x,
        y,
        s,
        x_rate,
        y_rate,
        s_rate,
    }
}

fn celestial_rates(days: f64, step_seconds: f64) -> [f64; 3] {
    // Sixth-order centered stencil, derived by cancelling the odd Taylor
    // terms through degree five. Difference pairs avoid subtracting large
    // secular offsets in the weighted sum. Only the smooth TT model is sampled:
    // EOP slopes, ERA, s' and the matrix product rule remain analytic.
    let pairs: [[f64; 3]; 3] = std::array::from_fn(|index| {
        let delta = (index + 1) as f64 * step_seconds / SECONDS_PER_DAY;
        let before = celestial_coordinates(days - delta);
        let after = celestial_coordinates(days + delta);
        std::array::from_fn(|axis| after[axis] - before[axis])
    });
    std::array::from_fn(|axis| {
        (45.0 * pairs[0][axis] - 9.0 * pairs[1][axis] + pairs[2][axis]) / (60.0 * step_seconds)
    })
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
    // Active column-vector TIRS-to-ITRS order (IERS TN 36, Chapter 5, §5.4);
    // MatrixRate::multiply applies the product rule at each factor.
    rotation_x(yp, yp_rate)
        .multiply(rotation_y(xp, xp_rate))
        .multiply(rotation_z(-s_prime, -s_prime_rate))
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
    fn full_model_preserves_the_previous_analytical_rate_regressions() {
        // Black-box outputs of orskit 836fec5's original analytic evaluator,
        // captured before replacement. These are regression observations, not
        // an independent SOFA reference or redistributed coefficient tables.
        let cases = [
            (
                -10_227.5,
                [
                    -2.693_700_041_935_697_4e-3,
                    1.526_867_496_628_253_6e-5,
                    3.388_825_999_372_527e-8,
                ],
                [
                    6.971_366_732_384_832e-12,
                    1.665_898_288_408_496e-12,
                    2.295_960_774_171_829_6e-15,
                ],
            ),
            (
                0.0,
                [
                    -2.694_637_956_852_230_6e-5,
                    -2.800_472_282_279_197_2e-5,
                    -1.013_396_519_177_576_2e-8,
                ],
                [
                    3.242_961_564_165_550_2e-12,
                    -1.152_177_960_555_024_2e-12,
                    -6.286_239_511_296_65e-17,
                ],
            ),
            (
                2_191.5,
                [
                    5.791_308_486_706_512e-4,
                    4.020_579_816_734_917_5e-5,
                    -1.220_032_213_076_960_2e-8,
                ],
                [
                    7.328_429_855_874_865e-12,
                    2.981_989_458_824_976_3e-12,
                    -7.227_762_238_711_812e-16,
                ],
            ),
            (
                9_131.5,
                [
                    2.429_600_921_731_267_3e-3,
                    3.437_084_850_624_645e-5,
                    -4.252_194_300_395_270_6e-8,
                ],
                [
                    6.409_839_542_152_867e-12,
                    2.684_104_857_543_004_5e-12,
                    -3.147_070_255_749_303_5e-15,
                ],
            ),
            (
                18_262.5,
                [
                    4.886_533_763_528_418e-3,
                    -5.341_831_990_200_602_4e-5,
                    1.058_366_160_189_923_4e-7,
                ],
                [
                    2.447_188_716_582_037_6e-12,
                    -1.860_380_271_181_446e-12,
                    4.484_862_624_196_736_5e-15,
                ],
            ),
            (
                36_524.5,
                [
                    9.720_602_149_458_657e-3,
                    -6.740_577_573_360_242e-5,
                    -4.315_960_021_234_412e-9,
                ],
                [
                    2.613_994_710_044_114e-12,
                    2.616_126_249_698_375e-12,
                    -1.280_649_544_749_513e-14,
                ],
            ),
        ];
        let mut maximum_value_shift = 0.0_f64;
        let mut maximum_rate_shift = 0.0_f64;
        for (days, values, rates) in cases {
            let actual = celestial_pole(days);
            for (actual, expected) in [actual.x, actual.y, actual.s].into_iter().zip(values) {
                let shift = (actual - expected).abs();
                maximum_value_shift = maximum_value_shift.max(shift);
                assert!(
                    shift < 6.0e-12,
                    "value shift at TT offset {days}: {shift:e}"
                );
            }
            for (actual, expected) in [actual.x_rate, actual.y_rate, actual.s_rate]
                .into_iter()
                .zip(rates)
            {
                let shift = (actual - expected).abs();
                maximum_rate_shift = maximum_rate_shift.max(shift);
                assert!(shift < 4.0e-17, "rate shift at TT offset {days}: {shift:e}");
            }
        }
        println!("maximum legacy coordinate/rate shifts: {maximum_value_shift:e} rad, {maximum_rate_shift:e} rad/s");
    }

    #[test]
    fn celestial_rates_converge_across_the_modern_epoch_range() {
        let mut maximum_difference = [0.0_f64; 3];
        // 1972-01-01 TT to 2100-01-01 TT, with fractional-day samples.
        // This evidence interval does not impose a new EOP coverage restriction.
        for index in 0..=256 {
            let days = -10_227.5 + 46_752.0 * f64::from(index) / 256.0;
            let rates = celestial_rates(days, CELESTIAL_RATE_STEP_SECONDS);
            let refined = celestial_rates(days, 256.0);
            // Independent fourth-order stencil and a larger step test both
            // truncation and floating-point cancellation, rather than only
            // comparing a stencil with itself.
            let step = 1_024.0;
            let before = celestial_coordinates(days - step / SECONDS_PER_DAY);
            let after = celestial_coordinates(days + step / SECONDS_PER_DAY);
            let before_two = celestial_coordinates(days - 2.0 * step / SECONDS_PER_DAY);
            let after_two = celestial_coordinates(days + 2.0 * step / SECONDS_PER_DAY);
            for axis in 0..3 {
                let fourth_order = (8.0 * (after[axis] - before[axis])
                    - (after_two[axis] - before_two[axis]))
                    / (12.0 * step);
                for other in [refined[axis], fourth_order] {
                    let difference = (rates[axis] - other).abs();
                    maximum_difference[axis] = maximum_difference[axis].max(difference);
                    let tolerance = if axis == 2 { 2.0e-20 } else { 2.0e-18 };
                    assert!(
                        difference < tolerance,
                        "TT offset {days}, axis {axis}: {difference:e} rad/s"
                    );
                }
            }
        }
        println!("maximum celestial rate convergence differences (rad/s): {maximum_difference:?}");
    }

    #[test]
    fn cio_series_matches_sofa_xys06a_reference_vector() {
        // ERFA v2.0.1, commit 9915ba38c9365f8b0738269b8c2ac1fdd5f8dee3:
        // src/t_erfa_c.c, eraXys06a vector (revision 2013-08-07),
        // TT JD = 2400000.5 + 53736.0. See the provenance ledger.
        let epoch_tt = Epoch::from_gregorian(2006, 1, 1, 0, 0, 0, 0, TimeScale::TT);
        let days = (epoch_tt - Epoch::from_gregorian(2000, 1, 1, 12, 0, 0, 0, TimeScale::TT))
            .to_seconds()
            / SECONDS_PER_DAY;
        let actual = celestial_pole(days);
        for (actual, expected, tolerance) in [
            (actual.x, 5.791_308_482_835_292e-4, 1.0e-14),
            (actual.y, 4.020_580_099_454_020_5e-5, 1.0e-14),
            (actual.s, -1.220_032_294_164_58e-8, 1.0e-18),
        ] {
            assert!(
                (actual - expected).abs() <= tolerance,
                "{actual:.18e} != {expected:.18e}"
            );
        }
    }

    #[test]
    fn cio_matrix_matches_sofa_c2t06a_reference_vector() {
        // ERFA v2.0.1, commit 9915ba38c9365f8b0738269b8c2ac1fdd5f8dee3:
        // src/t_erfa_c.c, eraC2t06a vector (revision 2013-08-07),
        // TT JD = UT1 JD = 2400000.5 + 53736.0. See the provenance ledger.
        let epoch_tt = Epoch::from_gregorian(2006, 1, 1, 0, 0, 0, 0, TimeScale::TT);
        let epoch_utc = epoch_tt.to_time_scale(TimeScale::UTC);
        let tt_minus_tai = 32.184;
        let left_epoch = epoch_utc - Duration::from_seconds(86_400.0);
        let right_epoch = epoch_utc + Duration::from_seconds(86_400.0);
        let left_dut1 = tt_minus_tai + f64::from(left_epoch.leap_seconds_iers());
        let right_dut1 = tt_minus_tai + f64::from(right_epoch.leap_seconds_iers());
        let xp = 2.550_602_38e-7;
        let yp = 1.860_359_247e-6;
        let provider = Iau2006CioProvider::new(
            vec![
                sample(left_epoch, left_dut1, 0.0, 0.0, xp, yp),
                sample(right_epoch, right_dut1, 0.0, 0.0, xp, yp),
            ],
            "IERS",
            "SOFA/ERFA published validation case",
            "ERFA t_erfa_c.c revision 2013-08-07",
            None,
        )
        .expect("valid reference-case samples");
        let reference_eop = provider
            .interpolate(
                epoch_tt.to_time_scale(TimeScale::TAI),
                BodyFixedTransformRequest::new(
                    epoch_tt,
                    TimeScale::TT,
                    InertialFrame::GCRF,
                    ReferenceFrame::ITRF2020,
                    BodyFixedTransformDirection::InertialToBodyFixed,
                )
                .expect("valid TT reference request"),
            )
            .expect("reference epoch is covered");
        assert!(
            (reference_eop.ut1_minus_tai - tt_minus_tai).abs() < 1.0e-9,
            "{reference_eop:?} vs TT−TAI {tt_minus_tai}"
        );
        let actual_era = earth_rotation_angle(
            epoch_tt.to_time_scale(TimeScale::TAI),
            reference_eop.ut1_minus_tai,
        );
        let expected_era = (TAU
            * (ERA_REFERENCE_FRACTION + ERA_TURNS_PER_DAY * 2_191.5).rem_euclid(1.0))
        .rem_euclid(TAU);
        assert!(
            (actual_era - expected_era).abs() < 1.0e-12,
            "ERA {actual_era} != {expected_era}"
        );
        let transform = provider
            .transform(
                BodyFixedTransformRequest::new(
                    epoch_tt,
                    TimeScale::TT,
                    InertialFrame::GCRF,
                    ReferenceFrame::ITRF2020,
                    BodyFixedTransformDirection::InertialToBodyFixed,
                )
                .expect("valid TT reference request"),
            )
            .expect("reference epoch is covered");
        let expected = [
            [
                -0.181_033_212_830_589_73,
                0.983_476_980_693_859_2,
                6.555_550_962_998_436e-5,
            ],
            [
                -0.983_476_813_413_621_5,
                -0.181_033_220_364_913_1,
                5.749_800_844_905_594e-4,
            ],
            [
                5.773_474_024_748_546e-4,
                3.961_816_829_632_690_6e-5,
                0.999_999_832_550_174_8,
            ],
        ];
        for (actual_row, expected_row) in transform.rotation().rows().iter().zip(expected) {
            for (actual, expected) in actual_row.iter().zip(expected_row) {
                assert!(
                    (actual - expected).abs() < 5.0e-12,
                    "{actual:.16e} != {expected:.16e}"
                );
            }
        }
    }

    #[test]
    fn polar_motion_uses_the_tirs_to_itrs_active_rotation_order() {
        let (xp, yp, s_prime) = (0.013, -0.021, 0.008);
        let (xp_rate, yp_rate, s_prime_rate) = (2.0e-5, -3.0e-5, 1.0e-5);
        let actual = polar_motion_matrix(xp, yp, s_prime, xp_rate, yp_rate, s_prime_rate);
        let expected = rotation_x(yp, yp_rate)
            .multiply(rotation_y(xp, xp_rate))
            .multiply(rotation_z(-s_prime, -s_prime_rate));
        for row in 0..3 {
            for column in 0..3 {
                assert!(
                    (actual.value.0[row][column] - expected.value.0[row][column]).abs() < 1.0e-15
                );
                assert!(
                    (actual.derivative.0[row][column] - expected.derivative.0[row][column]).abs()
                        < 1.0e-15
                );
            }
        }
        let reversed = rotation_z(-s_prime, -s_prime_rate)
            .multiply(rotation_y(xp, xp_rate))
            .multiply(rotation_x(yp, yp_rate));
        assert!(
            (actual.value.0[0][2] - reversed.value.0[0][2]).abs() > 1.0e-6,
            "the test angles must make the cross-term order observable"
        );
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

    #[test]
    fn angular_velocity_matches_finite_difference_with_all_changing_eop_fields() {
        let start = Epoch::from_gregorian_utc(2025, 1, 1, 0, 0, 0, 0);
        let provider = Iau2006CioProvider::new(
            vec![
                sample(start, -0.2, -2.0e-4, 3.0e-4, -5.0e-4, 7.0e-4),
                sample(
                    start + Duration::from_seconds(600.0),
                    0.4,
                    2.0e-4,
                    -3.0e-4,
                    5.0e-4,
                    -7.0e-4,
                ),
            ],
            "IERS",
            "synthetic changing EOP",
            "finite-difference fixture",
            None,
        )
        .expect("valid changing EOP samples");
        for elapsed in [150.0, 300.0, 450.0] {
            let epoch = start + Duration::from_seconds(elapsed);
            let step = 0.25;
            let before = provider
                .transform(request(
                    epoch - Duration::from_seconds(step),
                    BodyFixedTransformDirection::InertialToBodyFixed,
                ))
                .expect("earlier transform");
            let centre = provider
                .transform(request(
                    epoch,
                    BodyFixedTransformDirection::InertialToBodyFixed,
                ))
                .expect("central transform");
            let after = provider
                .transform(request(
                    epoch + Duration::from_seconds(step),
                    BodyFixedTransformDirection::InertialToBodyFixed,
                ))
                .expect("later transform");
            let before = before.rotation().rows();
            let centre_rows = centre.rotation().rows();
            let after = after.rotation().rows();
            let derivative = |row: usize, column: usize| {
                (after[row][column] - before[row][column]) / (2.0 * step)
            };
            let skew = |row: usize, column: usize| {
                (0..3)
                    .map(|k| derivative(row, k) * centre_rows[column][k])
                    .sum::<f64>()
            };
            let finite_difference = [skew(1, 2), skew(2, 0), skew(0, 1)];
            let analytic = centre.angular_velocity().to_radians_per_second();
            for (actual, expected) in analytic.into_iter().zip(finite_difference) {
                assert!(
                    (actual - expected).abs() < 2.0e-12,
                    "{actual:.16e} != {expected:.16e}"
                );
            }
        }
    }

    #[test]
    fn leap_boundary_keeps_ut1_tai_continuous_with_closed_coverage() {
        let before_leap = Epoch::from_gregorian_utc(2016, 12, 31, 0, 0, 0, 0);
        let after_leap = Epoch::from_gregorian_utc(2017, 1, 1, 0, 0, 0, 0);
        let provider = Iau2006CioProvider::new(
            vec![
                sample(before_leap, -0.4, 0.0, 0.0, 0.0, 0.0),
                sample(after_leap, 0.6, 0.0, 0.0, 0.0, 0.0),
            ],
            "IERS",
            "leap-boundary fixture",
            "test",
            None,
        )
        .expect("valid leap-boundary EOP samples");
        let first = provider
            .interpolate(
                before_leap.to_time_scale(TimeScale::TAI),
                request(
                    before_leap,
                    BodyFixedTransformDirection::InertialToBodyFixed,
                ),
            )
            .expect("first coverage endpoint is included");
        let last = provider
            .interpolate(
                after_leap.to_time_scale(TimeScale::TAI),
                request(after_leap, BodyFixedTransformDirection::InertialToBodyFixed),
            )
            .expect("last coverage endpoint is included");
        assert!((first.ut1_minus_tai + 36.4).abs() < 1.0e-12);
        assert!((last.ut1_minus_tai + 36.4).abs() < 1.0e-12);

        let before = Epoch::from_gregorian_utc(2016, 12, 31, 23, 59, 59, 0);
        let after = Epoch::from_gregorian_utc(2017, 1, 1, 0, 0, 0, 0);
        let before_angle =
            earth_rotation_angle(before.to_time_scale(TimeScale::TAI), first.ut1_minus_tai);
        let after_angle =
            earth_rotation_angle(after.to_time_scale(TimeScale::TAI), last.ut1_minus_tai);
        let angle_step = (after_angle - before_angle).rem_euclid(TAU);
        let expected_step = TAU * ERA_TURNS_PER_DAY * 2.0 / SECONDS_PER_DAY;
        assert!((angle_step - expected_step).abs() < 1.0e-12);

        assert!(matches!(
            provider.interpolate(
                before_leap.to_time_scale(TimeScale::TAI) - Duration::from_seconds(1.0e-9),
                request(
                    before_leap - Duration::from_seconds(1.0e-9),
                    BodyFixedTransformDirection::InertialToBodyFixed,
                ),
            ),
            Err(Iau2006CioError::EpochOutOfRange { .. })
        ));
        assert!(matches!(
            provider.interpolate(
                after_leap.to_time_scale(TimeScale::TAI) + Duration::from_seconds(1.0e-9),
                request(
                    after_leap + Duration::from_seconds(1.0e-9),
                    BodyFixedTransformDirection::InertialToBodyFixed,
                ),
            ),
            Err(Iau2006CioError::EpochOutOfRange { .. })
        ));
    }
}
