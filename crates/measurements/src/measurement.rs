//! Composable, unit-qualified measurement implementations.

use std::{fmt, ops::Add};

use frames::ReferenceFrame;
use hifitime::Epoch;
use nalgebra::{Cholesky, SMatrix};
use thiserror::Error;
#[cfg(any(
    feature = "azimuth-elevation",
    feature = "angular-ra-dec",
    feature = "phase"
))]
use units::uom::si::angle::radian;
#[cfg(any(feature = "doppler", feature = "fdoa"))]
use units::uom::si::frequency::hertz;
#[cfg(feature = "tdoa")]
use units::uom::si::time::second;
#[cfg(any(feature = "range-rate", feature = "bistatic-range-rate"))]
use units::uom::si::velocity::meter_per_second;
#[cfg(any(
    feature = "range",
    feature = "bistatic-range",
    feature = "turnaround-range"
))]
use units::uom::si::{area::square_meter, length::meter};
#[cfg(any(
    feature = "azimuth-elevation",
    feature = "angular-ra-dec",
    feature = "phase"
))]
use units::{Angle, AngularVariance};
#[cfg(any(
    feature = "range",
    feature = "bistatic-range",
    feature = "turnaround-range"
))]
use units::{Area, Length};
#[cfg(any(feature = "doppler", feature = "fdoa"))]
use units::{Frequency, FrequencyVariance};
#[cfg(feature = "tdoa")]
use units::{Time, TimeVariance};
#[cfg(any(feature = "range-rate", feature = "bistatic-range-rate"))]
use units::{Velocity, VelocityVariance};

use crate::SignalPath;

/// A unit-bearing one-value measurement with an explicitly known or unknown error.
///
/// ```compile_fail
/// use measurements::Measured;
///
/// let _invalid = Measured::new([42.0_f64], None);
/// ```
pub type Measured<Q> = MeasurementValues<Q, 1>;

/// Unit-bearing values observed together with their measurement error.
///
/// `N` is the number of values in this measurement. The error is absent only
/// when uncertainty is unknown. [`MeasurementValues::new`] accepts one
/// [`MeasurementUncertaintyInput`] enum for both cases: scalar standard error
/// for one-value observations, or an `N × N` covariance matrix otherwise.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasurementValues<Q: MeasurementQuantity, const N: usize> {
    values: [Q; N],
    error: Option<MeasurementUncertainty<Q, N>>,
}

/// Input error for constructing an observation.
///
/// The scalar variant is valid only for one-value observations. The covariance
/// variant is valid only for multi-value observations and is decomposed during
/// construction; the full covariance is not retained.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeasurementUncertaintyInput<Q: MeasurementQuantity, const N: usize> {
    /// Scalar standard error with the same unit as the observed value.
    Scalar(Q),
    /// Full, unit-qualified covariance matrix supplied for decomposition.
    Covariance([[Q::Variance; N]; N]),
}

/// A validated measurement error retained by an observation.
///
/// Scalar errors are stored directly. Multi-value covariance errors are stored
/// only as their lower-triangular Cholesky matrix, whose entries have the same
/// unit as the observed values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasurementUncertainty<Q: MeasurementQuantity, const N: usize> {
    representation: MeasurementUncertaintyRepresentation<Q, N>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum MeasurementUncertaintyRepresentation<Q: MeasurementQuantity, const N: usize> {
    Scalar(Q),
    LowerTriangular([[Q; N]; N]),
}

/// A unit-bearing scalar quantity admitted by [`Measured`].
///
/// Implement this contract for an application-owned unit-bearing quantity to
/// use the shared uncertainty mechanics with an application-defined observable.
/// Raw scalars remain excluded by Rust's orphan rules: an external crate may
/// implement this trait only for a type it owns.
pub trait MeasurementQuantity: Copy + Add<Output = Self> {
    /// The square physical quantity used for covariance entries.
    type Variance: Copy + fmt::Debug + PartialEq + Add<Output = Self::Variance>;

    #[doc(hidden)]
    fn to_si_scalar(self) -> f64;

    #[doc(hidden)]
    fn from_si_scalar(value: f64) -> Self;

    #[doc(hidden)]
    fn variance_to_si_scalar(value: Self::Variance) -> f64;

    #[doc(hidden)]
    fn variance_from_si_scalar(value: f64) -> Self::Variance;
}

impl<Q: MeasurementQuantity, const N: usize> MeasurementValues<Q, N> {
    /// Creates unit-qualified values observed together with their known or unknown error.
    ///
    /// A scalar error belongs to a one-value observation. A multi-value
    /// observation accepts a full covariance matrix, validates that it is
    /// strictly positive definite, and retains only its lower-triangular
    /// Cholesky matrix.
    pub fn new(
        values: [Q; N],
        error: Option<MeasurementUncertaintyInput<Q, N>>,
    ) -> Result<Self, MeasurementValueError> {
        if N == 0 {
            return Err(MeasurementValueError::EmptyValues);
        }
        if values
            .into_iter()
            .any(|value| !value.to_si_scalar().is_finite())
        {
            return Err(MeasurementValueError::NonFiniteValue);
        }
        let error = error.map(MeasurementUncertainty::from_input).transpose()?;
        Ok(Self { values, error })
    }

    /// Returns the unit-bearing values in their declared component order.
    #[must_use]
    pub const fn values(&self) -> &[Q; N] {
        &self.values
    }

    /// Returns the known error, or `None` if it is unknown.
    #[must_use]
    pub const fn error(&self) -> Option<MeasurementUncertainty<Q, N>> {
        self.error
    }

    #[cfg(any(
        feature = "range",
        feature = "range-rate",
        feature = "azimuth-elevation",
        feature = "doppler"
    ))]
    pub(crate) fn corrected(self, correction: Self) -> Result<Self, MeasurementValueError> {
        let values = std::array::from_fn(|index| self.values[index] + correction.values[index]);
        let error = match (self.error, correction.error) {
            (Some(left), Some(right)) => Some((left + right)?),
            _ => None,
        };
        Ok(Self { values, error })
    }
}

impl<Q: MeasurementQuantity> MeasurementValues<Q, 1> {
    /// Returns the scalar value.
    #[must_use]
    pub const fn value(&self) -> Q {
        self.values[0]
    }

    /// Returns the scalar standard error, or `None` if it is unknown.
    #[must_use]
    pub fn standard_error(&self) -> Option<Q> {
        self.error.and_then(|error| error.standard_error())
    }
}

impl<Q: MeasurementQuantity, const N: usize> MeasurementUncertainty<Q, N> {
    fn from_input(
        input: MeasurementUncertaintyInput<Q, N>,
    ) -> Result<Self, MeasurementUncertaintyError> {
        match input {
            MeasurementUncertaintyInput::Scalar(error) => Self::scalar(error),
            MeasurementUncertaintyInput::Covariance(matrix) => Self::from_covariance(matrix),
        }
    }

    fn from_covariance(matrix: [[Q::Variance; N]; N]) -> Result<Self, MeasurementUncertaintyError> {
        if N == 0 {
            return Err(MeasurementUncertaintyError::Empty);
        }
        if N == 1 {
            return Err(MeasurementUncertaintyError::ScalarRequired);
        }
        for (row, values) in matrix.iter().enumerate() {
            for (column, value) in values.iter().enumerate() {
                let value = Q::variance_to_si_scalar(*value);
                if !value.is_finite() {
                    return Err(MeasurementUncertaintyError::NonFinite { row, column });
                }
                if value != Q::variance_to_si_scalar(matrix[column][row]) {
                    return Err(MeasurementUncertaintyError::NotSymmetric { row, column });
                }
            }
        }
        let matrix_si = SMatrix::<f64, N, N>::from_fn(|row, column| {
            Q::variance_to_si_scalar(matrix[row][column])
        });
        let lower = Cholesky::new(matrix_si)
            .ok_or(MeasurementUncertaintyError::NotPositiveDefinite)?
            .unpack();
        Ok(Self {
            representation: MeasurementUncertaintyRepresentation::LowerTriangular(
                std::array::from_fn(|row| {
                    std::array::from_fn(|column| Q::from_si_scalar(lower[(row, column)]))
                }),
            ),
        })
    }

    /// Returns the stored lower-triangular Cholesky matrix for a multi-value observation.
    #[must_use]
    pub const fn lower_triangular_matrix(&self) -> Option<&[[Q; N]; N]> {
        match &self.representation {
            MeasurementUncertaintyRepresentation::Scalar(_) => None,
            MeasurementUncertaintyRepresentation::LowerTriangular(matrix) => Some(matrix),
        }
    }

    fn combine(self, other: Self) -> Result<Self, MeasurementUncertaintyError> {
        match (self.representation, other.representation) {
            (
                MeasurementUncertaintyRepresentation::Scalar(left),
                MeasurementUncertaintyRepresentation::Scalar(right),
            ) => {
                let error = Q::from_si_scalar(left.to_si_scalar().hypot(right.to_si_scalar()));
                if !error.to_si_scalar().is_finite() {
                    return Err(MeasurementUncertaintyError::NonFiniteScalar);
                }
                Ok(Self {
                    representation: MeasurementUncertaintyRepresentation::Scalar(error),
                })
            }
            (
                MeasurementUncertaintyRepresentation::LowerTriangular(left),
                MeasurementUncertaintyRepresentation::LowerTriangular(right),
            ) => {
                let left =
                    SMatrix::<f64, N, N>::from_fn(|row, column| left[row][column].to_si_scalar());
                let right =
                    SMatrix::<f64, N, N>::from_fn(|row, column| right[row][column].to_si_scalar());
                let covariance = left * left.transpose() + right * right.transpose();
                let lower = Cholesky::new(covariance)
                    .ok_or(MeasurementUncertaintyError::NotPositiveDefinite)?
                    .unpack();
                Ok(Self {
                    representation: MeasurementUncertaintyRepresentation::LowerTriangular(
                        std::array::from_fn(|row| {
                            std::array::from_fn(|column| Q::from_si_scalar(lower[(row, column)]))
                        }),
                    ),
                })
            }
            _ => Err(MeasurementUncertaintyError::MismatchedRepresentations),
        }
    }
}

impl<Q: MeasurementQuantity, const N: usize> MeasurementUncertainty<Q, N> {
    fn scalar(error: Q) -> Result<Self, MeasurementUncertaintyError> {
        if N != 1 {
            return Err(MeasurementUncertaintyError::CovarianceRequired);
        }
        let error_si = error.to_si_scalar();
        if !error_si.is_finite() {
            return Err(MeasurementUncertaintyError::NonFiniteScalar);
        }
        if error_si <= 0.0 {
            return Err(MeasurementUncertaintyError::NotPositiveScalar);
        }
        Ok(Self {
            representation: MeasurementUncertaintyRepresentation::Scalar(error),
        })
    }

    /// Returns the scalar standard error for a one-value observation.
    #[must_use]
    pub const fn standard_error(&self) -> Option<Q> {
        match self.representation {
            MeasurementUncertaintyRepresentation::Scalar(error) => Some(error),
            MeasurementUncertaintyRepresentation::LowerTriangular(_) => None,
        }
    }
}

impl<Q: MeasurementQuantity, const N: usize> Add for MeasurementUncertainty<Q, N> {
    /// Combines independent measurement errors.
    ///
    /// Scalar standard errors are combined by root-sum-of-squares. Stored
    /// lower-triangular matrices combine as `L_left L_leftᵀ + L_right L_rightᵀ`;
    /// the result is decomposed again and retained as a lower-triangular matrix.
    type Output = Result<Self, MeasurementUncertaintyError>;

    fn add(self, other: Self) -> Self::Output {
        self.combine(other)
    }
}

#[cfg(any(
    feature = "range",
    feature = "range-rate",
    feature = "azimuth-elevation",
    feature = "doppler",
    feature = "angular-ra-dec",
    feature = "bistatic-range",
    feature = "bistatic-range-rate",
    feature = "turnaround-range",
    feature = "tdoa",
    feature = "fdoa",
    feature = "phase"
))]
macro_rules! measurement_quantity {
    ($quantity:ty, $unit:ty, $variance:ty, $variance_to_si:expr, $variance_from_si:expr) => {
        impl MeasurementQuantity for $quantity {
            type Variance = $variance;

            fn to_si_scalar(self) -> f64 {
                self.get::<$unit>()
            }

            fn from_si_scalar(value: f64) -> Self {
                Self::new::<$unit>(value)
            }

            fn variance_to_si_scalar(value: Self::Variance) -> f64 {
                $variance_to_si(value)
            }

            fn variance_from_si_scalar(value: f64) -> Self::Variance {
                $variance_from_si(value)
            }
        }
    };
}

#[cfg(any(
    feature = "range",
    feature = "bistatic-range",
    feature = "turnaround-range"
))]
measurement_quantity!(
    Length,
    meter,
    Area,
    |value: Area| value.get::<square_meter>(),
    Area::new::<square_meter>
);
#[cfg(any(feature = "range-rate", feature = "bistatic-range-rate"))]
measurement_quantity!(
    Velocity,
    meter_per_second,
    VelocityVariance,
    VelocityVariance::as_square_metres_per_square_second,
    VelocityVariance::from_square_metres_per_square_second
);
#[cfg(any(
    feature = "azimuth-elevation",
    feature = "angular-ra-dec",
    feature = "phase"
))]
measurement_quantity!(
    Angle,
    radian,
    AngularVariance,
    AngularVariance::as_square_radians,
    AngularVariance::from_square_radians
);
#[cfg(any(feature = "doppler", feature = "fdoa"))]
measurement_quantity!(
    Frequency,
    hertz,
    FrequencyVariance,
    FrequencyVariance::as_square_hertz,
    FrequencyVariance::from_square_hertz
);
#[cfg(feature = "tdoa")]
measurement_quantity!(
    Time,
    second,
    TimeVariance,
    TimeVariance::as_square_seconds,
    TimeVariance::from_square_seconds
);

/// Family identity for a measurement implementation.
///
/// Applications can define their own marker rather than being restricted to a
/// crate-owned enumeration. The name is intended for routing and diagnostics;
/// the concrete marker type remains available to typed application code.
pub trait MeasurementKind: fmt::Debug + Send + Sync + 'static {
    /// Returns a stable family name for diagnostics and heterogeneous routing.
    fn name(&self) -> &'static str;
}

#[cfg(any(
    feature = "range",
    feature = "range-rate",
    feature = "azimuth-elevation",
    feature = "doppler"
))]
macro_rules! measurement_kind {
    ($(#[$meta:meta])* $name:ident, $family_name:literal) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name;

        impl MeasurementKind for $name {
            fn name(&self) -> &'static str {
                $family_name
            }
        }
    };
}

#[cfg(feature = "range")]
measurement_kind!(
    /// Scalar range measurement family.
    RangeKind,
    "range"
);
#[cfg(feature = "range-rate")]
measurement_kind!(
    /// Signed range-rate measurement family.
    RangeRateKind,
    "range-rate"
);
#[cfg(feature = "azimuth-elevation")]
measurement_kind!(
    /// Azimuth/elevation measurement family.
    AzimuthElevationKind,
    "azimuth-elevation"
);
#[cfg(feature = "doppler")]
measurement_kind!(
    /// Signed Doppler frequency-shift measurement family.
    DopplerKind,
    "doppler"
);

/// Object-safe common contract for heterogeneous measurement composition.
///
/// The concrete implementation carries its typed values. This trait exposes
/// only metadata and family identity so a collection of measurements never
/// erases dimensions into raw floating-point values.
pub trait Measurement: fmt::Debug + Send + Sync {
    /// Returns the ordered signal path for this measurement.
    fn path(&self) -> &SignalPath;

    /// Returns this measurement's explicitly declared epoch.
    fn epoch(&self) -> Epoch;

    /// Returns the frame in which this measurement's components are interpreted.
    fn frame(&self) -> ReferenceFrame;

    /// Returns this implementation's observable family.
    fn kind(&self) -> &'static dyn MeasurementKind;
}

/// Meaning of the scalar stored in a range observation.
#[cfg(feature = "range")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RangeConvention {
    /// Sum of geometric path lengths over every leg in the signal path.
    PathLength,
    /// Half of a two-leg returning path length.
    RoundTripOneWayEquivalent,
}

/// A participant-qualified scalar range observation.
#[cfg(feature = "range")]
#[derive(Debug, Clone, PartialEq)]
pub struct RangeMeasurement {
    path: SignalPath,
    epoch: Epoch,
    frame: ReferenceFrame,
    convention: RangeConvention,
    value: Measured<Length>,
}

#[cfg(feature = "range")]
impl RangeMeasurement {
    /// Creates a scalar range measurement.
    pub fn new(
        path: SignalPath,
        epoch: Epoch,
        frame: ReferenceFrame,
        convention: RangeConvention,
        value: Measured<Length>,
    ) -> Result<Self, MeasurementError> {
        if convention == RangeConvention::RoundTripOneWayEquivalent
            && (path.participant_count() != 3 || path.participant(0) != path.participant(2))
        {
            return Err(MeasurementError::InvalidRoundTripPath);
        }
        if value.value().get::<meter>() < 0.0 {
            return Err(MeasurementError::NegativeRange);
        }
        Ok(Self {
            path,
            epoch,
            frame,
            convention,
            value,
        })
    }

    /// Returns the declared scalar range convention.
    #[must_use]
    pub const fn convention(&self) -> RangeConvention {
        self.convention
    }

    /// Returns the range and its explicit uncertainty state.
    #[must_use]
    pub const fn value(&self) -> Measured<Length> {
        self.value
    }

    pub(crate) fn with_value(&self, value: Measured<Length>) -> Result<Self, MeasurementError> {
        Self::new(
            self.path.clone(),
            self.epoch,
            self.frame,
            self.convention,
            value,
        )
    }

    pub(crate) fn into_value(self, value: Measured<Length>) -> Result<Self, MeasurementError> {
        Self::new(self.path, self.epoch, self.frame, self.convention, value)
    }
}

#[cfg(feature = "range")]
impl Measurement for RangeMeasurement {
    fn path(&self) -> &SignalPath {
        &self.path
    }

    fn epoch(&self) -> Epoch {
        self.epoch
    }

    fn frame(&self) -> ReferenceFrame {
        self.frame
    }

    fn kind(&self) -> &'static dyn MeasurementKind {
        &RangeKind
    }
}

/// A signed line-of-sight range-rate observation.
#[cfg(feature = "range-rate")]
#[derive(Debug, Clone, PartialEq)]
pub struct RangeRateMeasurement {
    path: SignalPath,
    epoch: Epoch,
    frame: ReferenceFrame,
    value: Measured<Velocity>,
}

#[cfg(feature = "range-rate")]
impl RangeRateMeasurement {
    /// Creates a signed range-rate measurement. Positive sign means increasing range.
    #[must_use]
    pub const fn new(
        path: SignalPath,
        epoch: Epoch,
        frame: ReferenceFrame,
        value: Measured<Velocity>,
    ) -> Self {
        Self {
            path,
            epoch,
            frame,
            value,
        }
    }

    /// Returns the range rate and its explicit uncertainty state.
    #[must_use]
    pub const fn value(&self) -> Measured<Velocity> {
        self.value
    }

    pub(crate) fn into_value(self, value: Measured<Velocity>) -> Result<Self, MeasurementError> {
        Ok(Self::new(self.path, self.epoch, self.frame, value))
    }
}

#[cfg(feature = "range-rate")]
impl Measurement for RangeRateMeasurement {
    fn path(&self) -> &SignalPath {
        &self.path
    }

    fn epoch(&self) -> Epoch {
        self.epoch
    }

    fn frame(&self) -> ReferenceFrame {
        self.frame
    }

    fn kind(&self) -> &'static dyn MeasurementKind {
        &RangeRateKind
    }
}

/// Explicit convention for ground-relative angular measurements.
#[cfg(feature = "azimuth-elevation")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AzimuthElevationConvention {
    /// Azimuth is clockwise from local north; elevation is above local horizon.
    ClockwiseFromNorthAboveHorizon,
}

/// A ground-relative azimuth/elevation observation.
#[cfg(feature = "azimuth-elevation")]
#[derive(Debug, Clone, PartialEq)]
pub struct AzimuthElevationMeasurement {
    path: SignalPath,
    epoch: Epoch,
    frame: ReferenceFrame,
    convention: AzimuthElevationConvention,
    values: MeasurementValues<Angle, 2>,
}

#[cfg(feature = "azimuth-elevation")]
impl AzimuthElevationMeasurement {
    /// Creates a ground-relative angular measurement.
    ///
    /// Azimuth is normalized to `[0, 2π)` and elevation is in `[-π/2, π/2]`.
    pub fn new(
        path: SignalPath,
        epoch: Epoch,
        frame: ReferenceFrame,
        convention: AzimuthElevationConvention,
        values: MeasurementValues<Angle, 2>,
    ) -> Result<Self, MeasurementError> {
        let [azimuth, elevation] = *values.values();
        let azimuth_value = azimuth.get::<radian>();
        let elevation_value = elevation.get::<radian>();
        if !(0.0..std::f64::consts::TAU).contains(&azimuth_value) {
            return Err(MeasurementError::AzimuthOutOfRange);
        }
        if !(-std::f64::consts::FRAC_PI_2..=std::f64::consts::FRAC_PI_2).contains(&elevation_value)
        {
            return Err(MeasurementError::ElevationOutOfRange);
        }
        Ok(Self {
            path,
            epoch,
            frame,
            convention,
            values,
        })
    }

    /// Returns the angular convention.
    #[must_use]
    pub const fn convention(&self) -> AzimuthElevationConvention {
        self.convention
    }

    /// Returns azimuth and elevation together with their `2 × 2` covariance.
    #[must_use]
    pub const fn values(&self) -> &MeasurementValues<Angle, 2> {
        &self.values
    }

    pub(crate) fn into_values(
        self,
        values: MeasurementValues<Angle, 2>,
    ) -> Result<Self, MeasurementError> {
        Self::new(self.path, self.epoch, self.frame, self.convention, values)
    }
}

#[cfg(feature = "azimuth-elevation")]
impl Measurement for AzimuthElevationMeasurement {
    fn path(&self) -> &SignalPath {
        &self.path
    }

    fn epoch(&self) -> Epoch {
        self.epoch
    }

    fn frame(&self) -> ReferenceFrame {
        self.frame
    }

    fn kind(&self) -> &'static dyn MeasurementKind {
        &AzimuthElevationKind
    }
}

/// A signed Doppler frequency-shift observation.
#[cfg(feature = "doppler")]
#[derive(Debug, Clone, PartialEq)]
pub struct DopplerMeasurement {
    path: SignalPath,
    epoch: Epoch,
    frame: ReferenceFrame,
    value: Measured<Frequency>,
}

#[cfg(feature = "doppler")]
impl DopplerMeasurement {
    /// Creates a signed received-frequency-shift measurement.
    #[must_use]
    pub const fn new(
        path: SignalPath,
        epoch: Epoch,
        frame: ReferenceFrame,
        value: Measured<Frequency>,
    ) -> Self {
        Self {
            path,
            epoch,
            frame,
            value,
        }
    }

    /// Returns the frequency shift and its explicit uncertainty state.
    #[must_use]
    pub const fn value(&self) -> Measured<Frequency> {
        self.value
    }

    pub(crate) fn into_value(self, value: Measured<Frequency>) -> Result<Self, MeasurementError> {
        Ok(Self::new(self.path, self.epoch, self.frame, value))
    }
}

#[cfg(feature = "doppler")]
impl Measurement for DopplerMeasurement {
    fn path(&self) -> &SignalPath {
        &self.path
    }

    fn epoch(&self) -> Epoch {
        self.epoch
    }

    fn frame(&self) -> ReferenceFrame {
        self.frame
    }

    fn kind(&self) -> &'static dyn MeasurementKind {
        &DopplerKind
    }
}

/// Invalid measured scalar data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum MeasurementValueError {
    /// Measurements must contain at least one unit-qualified value.
    #[error("a measurement must contain at least one value")]
    EmptyValues,
    /// The observed value is NaN or infinite.
    #[error("measurement value must be finite")]
    NonFiniteValue,
    /// Correction error could not be propagated as a valid measurement error.
    #[error(transparent)]
    Uncertainty(#[from] MeasurementUncertaintyError),
}

/// Invalid scalar standard error or covariance-matrix input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum MeasurementUncertaintyError {
    /// A covariance must have one row and column for every observed value.
    #[error("a covariance matrix must not be empty")]
    Empty,
    /// A scalar observation must use a scalar standard error.
    #[error("a one-value measurement error must be scalar")]
    ScalarRequired,
    /// A multi-value observation must use a covariance matrix.
    #[error("a multi-value measurement error must be a covariance matrix")]
    CovarianceRequired,
    /// A covariance entry is NaN or infinite.
    #[error("covariance entry at ({row}, {column}) must be finite")]
    NonFinite {
        /// Matrix row.
        row: usize,
        /// Matrix column.
        column: usize,
    },
    /// Two mirrored covariance entries differ.
    #[error("covariance entries at ({row}, {column}) and ({column}, {row}) must match")]
    NotSymmetric {
        /// Matrix row.
        row: usize,
        /// Matrix column.
        column: usize,
    },
    /// A scalar standard error is NaN or infinite.
    #[error("scalar standard error must be finite")]
    NonFiniteScalar,
    /// A scalar standard error is zero or negative.
    #[error("scalar standard error must be strictly positive")]
    NotPositiveScalar,
    /// A covariance matrix is not strictly positive definite.
    #[error("covariance matrix must be strictly positive definite")]
    NotPositiveDefinite,
    /// Errors with different representations cannot be combined.
    #[error("measurement errors use incompatible representations")]
    MismatchedRepresentations,
}

/// Invalid measurement semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum MeasurementError {
    /// One-way-equivalent round-trip semantics were attached to another topology.
    #[error(
        "round-trip one-way-equivalent range requires a three-event path with matching endpoints"
    )]
    InvalidRoundTripPath,
    /// A geometric range cannot be negative.
    #[error("measurement range must be non-negative")]
    NegativeRange,
    /// Azimuth does not use the declared normalized interval.
    #[error("azimuth must be in [0, 2π)")]
    AzimuthOutOfRange,
    /// Elevation does not use the declared horizon-relative interval.
    #[error("elevation must be in [-π/2, π/2]")]
    ElevationOutOfRange,
}

#[cfg(all(
    test,
    feature = "range",
    feature = "range-rate",
    feature = "azimuth-elevation",
    feature = "doppler"
))]
mod tests {
    use super::*;
    use crate::ParticipantId;

    fn path() -> SignalPath {
        SignalPath::new(vec![
            ParticipantId::try_from("DSS-14".to_owned()).expect("station ID"),
            ParticipantId::try_from("SC-01".to_owned()).expect("spacecraft ID"),
        ])
        .expect("signal path")
    }

    #[test]
    fn values_reject_invalid_input() {
        assert_eq!(
            Measured::new([Length::new::<meter>(f64::NAN)], None),
            Err(MeasurementValueError::NonFiniteValue)
        );
        assert_eq!(
            Measured::new(
                [Length::new::<meter>(1.0)],
                Some(MeasurementUncertaintyInput::Covariance([[Area::new::<
                    square_meter,
                >(
                    1.0
                ),]])),
            ),
            Err(MeasurementValueError::Uncertainty(
                MeasurementUncertaintyError::ScalarRequired
            ))
        );
        assert_eq!(
            MeasurementValues::new(
                [Length::new::<meter>(1.0), Length::new::<meter>(2.0)],
                Some(MeasurementUncertaintyInput::Covariance([
                    [
                        Area::new::<square_meter>(1.0),
                        Area::new::<square_meter>(0.1),
                    ],
                    [
                        Area::new::<square_meter>(0.0),
                        Area::new::<square_meter>(1.0),
                    ],
                ])),
            ),
            Err(MeasurementValueError::Uncertainty(
                MeasurementUncertaintyError::NotSymmetric { row: 0, column: 1 }
            ))
        );
        assert_eq!(
            MeasurementValues::new(
                [Length::new::<meter>(1.0), Length::new::<meter>(2.0)],
                Some(MeasurementUncertaintyInput::Covariance([
                    [
                        Area::new::<square_meter>(1.0),
                        Area::new::<square_meter>(2.0),
                    ],
                    [
                        Area::new::<square_meter>(2.0),
                        Area::new::<square_meter>(1.0),
                    ],
                ])),
            ),
            Err(MeasurementValueError::Uncertainty(
                MeasurementUncertaintyError::NotPositiveDefinite
            ))
        );
        assert_eq!(
            MeasurementValues::new(
                [Length::new::<meter>(1.0), Length::new::<meter>(2.0)],
                Some(MeasurementUncertaintyInput::Scalar(Length::new::<meter>(
                    1.0
                ))),
            ),
            Err(MeasurementValueError::Uncertainty(
                MeasurementUncertaintyError::CovarianceRequired
            ))
        );
    }

    #[test]
    fn range_topology_and_sign_are_not_inferred() {
        let value = Measured::new([Length::new::<meter>(1.0)], None).expect("value");
        assert_eq!(
            RangeMeasurement::new(
                path(),
                Epoch::from_tai_seconds(0.0),
                ReferenceFrame::ITRF2020,
                RangeConvention::RoundTripOneWayEquivalent,
                value,
            ),
            Err(MeasurementError::InvalidRoundTripPath)
        );
        assert_eq!(
            RangeMeasurement::new(
                path(),
                Epoch::from_tai_seconds(0.0),
                ReferenceFrame::ITRF2020,
                RangeConvention::PathLength,
                Measured::new([Length::new::<meter>(-1.0)], None).expect("value"),
            ),
            Err(MeasurementError::NegativeRange)
        );
    }

    #[test]
    fn scalar_errors_combine_with_the_add_operator() {
        let left = Measured::new(
            [Length::new::<meter>(1.0)],
            Some(MeasurementUncertaintyInput::Scalar(Length::new::<meter>(
                3.0,
            ))),
        )
        .expect("left measurement")
        .error()
        .expect("left error");
        let right = Measured::new(
            [Length::new::<meter>(1.0)],
            Some(MeasurementUncertaintyInput::Scalar(Length::new::<meter>(
                4.0,
            ))),
        )
        .expect("right measurement")
        .error()
        .expect("right error");

        let sum = (left + right).expect("summed error");
        assert_eq!(sum.standard_error(), Some(Length::new::<meter>(5.0)));
    }

    #[test]
    fn application_owned_quantities_and_measurement_families_compose() {
        #[derive(Debug, Clone, Copy, PartialEq)]
        struct SignalStrength(f64);
        #[derive(Debug, Clone, Copy, PartialEq)]
        struct SignalStrengthVariance(f64);

        impl std::ops::Add for SignalStrength {
            type Output = Self;

            fn add(self, other: Self) -> Self {
                Self(self.0 + other.0)
            }
        }

        impl std::ops::Add for SignalStrengthVariance {
            type Output = Self;

            fn add(self, other: Self) -> Self {
                Self(self.0 + other.0)
            }
        }

        impl MeasurementQuantity for SignalStrength {
            type Variance = SignalStrengthVariance;

            fn to_si_scalar(self) -> f64 {
                self.0
            }

            fn from_si_scalar(value: f64) -> Self {
                Self(value)
            }

            fn variance_to_si_scalar(value: Self::Variance) -> f64 {
                value.0
            }

            fn variance_from_si_scalar(value: f64) -> Self::Variance {
                SignalStrengthVariance(value)
            }
        }

        #[derive(Debug)]
        struct CarrierPhaseKind;
        impl MeasurementKind for CarrierPhaseKind {
            fn name(&self) -> &'static str {
                "carrier-phase"
            }
        }

        #[derive(Debug)]
        struct CarrierPhaseMeasurement {
            path: SignalPath,
            epoch: Epoch,
            frame: ReferenceFrame,
        }
        impl Measurement for CarrierPhaseMeasurement {
            fn path(&self) -> &SignalPath {
                &self.path
            }

            fn epoch(&self) -> Epoch {
                self.epoch
            }

            fn frame(&self) -> ReferenceFrame {
                self.frame
            }

            fn kind(&self) -> &'static dyn MeasurementKind {
                &CarrierPhaseKind
            }
        }

        let values = MeasurementValues::new(
            [SignalStrength(1.0), SignalStrength(2.0)],
            Some(MeasurementUncertaintyInput::Covariance([
                [SignalStrengthVariance(1.0), SignalStrengthVariance(0.0)],
                [SignalStrengthVariance(0.0), SignalStrengthVariance(1.0)],
            ])),
        )
        .expect("application quantity is accepted");
        assert!(values.error().is_some());
        assert_eq!(
            CarrierPhaseMeasurement {
                path: path(),
                epoch: Epoch::from_tai_seconds(0.0),
                frame: ReferenceFrame::ITRF2020,
            }
            .kind()
            .name(),
            "carrier-phase"
        );
    }
}
