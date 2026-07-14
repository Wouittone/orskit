//! Typed additive measurement corrections and their composition.

use std::fmt;

use thiserror::Error;
#[cfg(feature = "azimuth-elevation")]
use units::Angle;
#[cfg(feature = "doppler")]
use units::Frequency;
#[cfg(feature = "range")]
use units::Length;
#[cfg(feature = "range-rate")]
use units::Velocity;

#[cfg(feature = "doppler")]
use crate::DopplerMeasurement;
#[cfg(feature = "range")]
use crate::RangeMeasurement;
#[cfg(feature = "range-rate")]
use crate::RangeRateMeasurement;
#[cfg(feature = "azimuth-elevation")]
use crate::{AzimuthElevationMeasurement, MeasurementValues};
use crate::{Measured, MeasurementError, MeasurementQuantity, MeasurementValueError};

/// Physical provenance of a measurement correction.
///
/// This is deliberately an open trait. Applications can define their own
/// provenance marker and implement this trait, rather than being limited to a
/// crate-owned enumeration.
pub trait CorrectionKind: fmt::Debug + Send + Sync + 'static {}

#[cfg(any(
    feature = "clock-correction",
    feature = "troposphere-correction",
    feature = "ionosphere-correction",
    feature = "relativity-correction",
    feature = "instrument-correction"
))]
macro_rules! correction_kind {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name;

        impl CorrectionKind for $name {}
    };
}

#[cfg(feature = "clock-correction")]
correction_kind!(
    /// A station or spacecraft clock contribution.
    Clock
);
#[cfg(feature = "troposphere-correction")]
correction_kind!(
    /// A neutral-atmosphere contribution.
    Troposphere
);
#[cfg(feature = "ionosphere-correction")]
correction_kind!(
    /// An ionospheric contribution.
    Ionosphere
);
#[cfg(feature = "relativity-correction")]
correction_kind!(
    /// A relativistic signal-propagation contribution.
    Relativity
);
#[cfg(feature = "instrument-correction")]
correction_kind!(
    /// An antenna, transponder, or receiver contribution.
    Instrument
);

/// One unit-qualified additive correction and its uncertainty state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdditiveCorrection<K: CorrectionKind, Q: MeasurementQuantity> {
    kind: K,
    value: Measured<Q>,
}

impl<K: CorrectionKind, Q: MeasurementQuantity> AdditiveCorrection<K, Q> {
    /// Creates an additive correction. Its sign is applied algebraically.
    #[must_use]
    pub const fn new(kind: K, value: Measured<Q>) -> Self {
        Self { kind, value }
    }

    /// Returns the correction's physical provenance category.
    #[must_use]
    pub const fn kind(&self) -> &K {
        &self.kind
    }

    /// Returns the unit-qualified correction and uncertainty state.
    #[must_use]
    pub const fn value(&self) -> Measured<Q> {
        self.value
    }
}

/// A correction applicable to one concrete measurement implementation.
///
/// Generic composition keeps corrections dimension-safe: a range correction
/// cannot be accidentally added to Doppler or angle data. Trait objects such
/// as `Box<dyn MeasurementCorrection<RangeMeasurement>>` compose heterogeneous
/// correction models for one observable family.
pub trait MeasurementCorrection<M>: fmt::Debug + Send + Sync {
    /// Applies this correction while retaining the observation metadata.
    fn apply(&self, measurement: &M) -> Result<M, CorrectionError>;
}

/// An ordered composition of corrections for one measurement implementation.
#[derive(Debug, Default)]
pub struct CorrectionChain<M> {
    corrections: Vec<Box<dyn MeasurementCorrection<M>>>,
}

impl<M> CorrectionChain<M> {
    /// Creates an empty correction chain.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            corrections: Vec::new(),
        }
    }

    /// Appends one correction, preserving application order.
    pub fn push(&mut self, correction: impl MeasurementCorrection<M> + 'static) {
        self.corrections.push(Box::new(correction));
    }

    /// Returns the number of composed corrections.
    #[must_use]
    pub fn len(&self) -> usize {
        self.corrections.len()
    }

    /// Returns whether no correction is configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.corrections.is_empty()
    }
}

impl<M: Clone> CorrectionChain<M> {
    /// Applies every correction in insertion order.
    pub fn apply(&self, measurement: &M) -> Result<M, CorrectionError> {
        self.corrections
            .iter()
            .try_fold(measurement.clone(), |value, correction| {
                correction.apply(&value)
            })
    }
}

#[cfg(any(feature = "range", feature = "range-rate", feature = "doppler"))]
macro_rules! scalar_correction {
    ($quantity:ty, $measurement:ty) => {
        impl<K: CorrectionKind> MeasurementCorrection<$measurement>
            for AdditiveCorrection<K, $quantity>
        {
            fn apply(&self, measurement: &$measurement) -> Result<$measurement, CorrectionError> {
                let value = measurement.value().corrected(self.value)?;
                measurement.with_value(value).map_err(Into::into)
            }
        }
    };
}

#[cfg(feature = "range")]
scalar_correction!(Length, RangeMeasurement);
#[cfg(feature = "range-rate")]
scalar_correction!(Velocity, RangeRateMeasurement);
#[cfg(feature = "doppler")]
scalar_correction!(Frequency, DopplerMeasurement);

/// A two-axis angular correction for an azimuth/elevation observation.
#[cfg(feature = "azimuth-elevation")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AzimuthElevationCorrection<K: CorrectionKind> {
    kind: K,
    values: MeasurementValues<Angle, 2>,
}

#[cfg(feature = "azimuth-elevation")]
impl<K: CorrectionKind> AzimuthElevationCorrection<K> {
    /// Creates a correction with azimuth/elevation values and their `2 × 2` covariance matrix.
    #[must_use]
    pub const fn new(kind: K, values: MeasurementValues<Angle, 2>) -> Self {
        Self { kind, values }
    }

    /// Returns the physical provenance category.
    #[must_use]
    pub const fn kind(&self) -> &K {
        &self.kind
    }
}

#[cfg(feature = "azimuth-elevation")]
impl<K: CorrectionKind> MeasurementCorrection<AzimuthElevationMeasurement>
    for AzimuthElevationCorrection<K>
{
    fn apply(
        &self,
        measurement: &AzimuthElevationMeasurement,
    ) -> Result<AzimuthElevationMeasurement, CorrectionError> {
        let values = (*measurement.values()).corrected(self.values)?;
        measurement.with_values(values).map_err(Into::into)
    }
}

/// Failure while applying a correction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum CorrectionError {
    /// Arithmetic produced a non-finite corrected value or uncertainty.
    #[error(transparent)]
    InvalidValue(#[from] MeasurementValueError),
    /// The corrected data no longer meet its observable's semantic contract.
    #[error(transparent)]
    InvalidMeasurement(#[from] MeasurementError),
}

#[cfg(all(
    test,
    feature = "range",
    feature = "range-rate",
    feature = "azimuth-elevation",
    feature = "doppler",
    feature = "clock-correction",
    feature = "troposphere-correction",
    feature = "instrument-correction"
))]
mod tests {
    use frames::ReferenceFrame;
    use hifitime::Epoch;
    use units::uom::si::{
        angle::radian, frequency::hertz, length::meter, velocity::meter_per_second,
    };
    use units::{Angle, AngularVariance, Frequency, Length, Velocity};

    use super::*;
    use crate::{
        AzimuthElevationConvention, Measurement, MeasurementUncertaintyInput, MeasurementValues,
        ParticipantId, RangeConvention, SignalPath,
    };

    fn path() -> SignalPath {
        let station = ParticipantId::new("DSS-14").expect("station ID");
        let spacecraft = ParticipantId::new("SC-01").expect("spacecraft ID");
        SignalPath::new(vec![station, spacecraft]).expect("signal path")
    }

    fn length(value: f64, standard_deviation: Option<f64>) -> Measured<Length> {
        Measured::new(
            [Length::new::<meter>(value)],
            standard_deviation
                .map(|value| MeasurementUncertaintyInput::Scalar(Length::new::<meter>(value))),
        )
        .expect("finite length")
    }

    fn angles(
        azimuth: f64,
        elevation: f64,
        covariance: Option<[[f64; 2]; 2]>,
    ) -> MeasurementValues<Angle, 2> {
        MeasurementValues::new(
            [
                Angle::new::<radian>(azimuth),
                Angle::new::<radian>(elevation),
            ],
            covariance.map(|entries| {
                MeasurementUncertaintyInput::Covariance(
                    entries.map(|row| row.map(AngularVariance::from_square_radians)),
                )
            }),
        )
        .expect("angles")
    }

    #[test]
    fn correction_chain_preserves_units_and_combines_known_errors() {
        let range = RangeMeasurement::new(
            path(),
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::ITRF2020,
            RangeConvention::PathLength,
            length(100.0, Some(3.0)),
        )
        .expect("range");
        let mut corrections = CorrectionChain::new();
        corrections.push(AdditiveCorrection::new(Troposphere, length(2.0, Some(4.0))));

        let corrected = corrections.apply(&range).expect("corrected range");
        assert_eq!(corrected.path(), range.path());
        assert_eq!(corrected.epoch(), range.epoch());
        assert_eq!(corrected.frame(), range.frame());
        assert_eq!(corrected.value().value(), Length::new::<meter>(102.0));
        assert_eq!(
            corrected.value().standard_error(),
            Some(Length::new::<meter>(5.0))
        );
    }

    #[test]
    fn unknown_correction_error_remains_explicitly_unknown() {
        let range = RangeMeasurement::new(
            path(),
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::ITRF2020,
            RangeConvention::PathLength,
            length(100.0, Some(3.0)),
        )
        .expect("range");
        let correction = AdditiveCorrection::new(
            Clock,
            Measured::new([Length::new::<meter>(2.0)], None).expect("correction"),
        );

        let corrected = correction.apply(&range).expect("corrected range");
        assert_eq!(corrected.value().error(), None);
    }

    #[test]
    fn concrete_observables_share_only_the_object_safe_metadata_contract() {
        let range = RangeMeasurement::new(
            path(),
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::ITRF2020,
            RangeConvention::PathLength,
            length(1.0, None),
        )
        .expect("range");
        let range_rate = RangeRateMeasurement::new(
            path(),
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::ITRF2020,
            Measured::new(
                [Velocity::new::<meter_per_second>(1.0)],
                Some(MeasurementUncertaintyInput::Scalar(Velocity::new::<
                    meter_per_second,
                >(0.1))),
            )
            .expect("range rate"),
        );
        let doppler = DopplerMeasurement::new(
            path(),
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::ITRF2020,
            Measured::new(
                [Frequency::new::<hertz>(10.0)],
                Some(MeasurementUncertaintyInput::Scalar(
                    Frequency::new::<hertz>(0.1),
                )),
            )
            .expect("Doppler"),
        );
        let angle_measurement = AzimuthElevationMeasurement::new(
            path(),
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::ITRF2020,
            AzimuthElevationConvention::ClockwiseFromNorthAboveHorizon,
            angles(1.0, 0.5, Some([[0.1, 0.05], [0.05, 0.2]])),
        )
        .expect("angles");

        let measurements: [&dyn Measurement; 4] =
            [&range, &range_rate, &doppler, &angle_measurement];
        assert_eq!(
            measurements.map(|measurement| measurement.kind().name()),
            ["range", "range-rate", "doppler", "azimuth-elevation"]
        );
    }

    #[test]
    fn angular_correction_retains_only_the_combined_lower_triangular_matrix() {
        let measurement = AzimuthElevationMeasurement::new(
            path(),
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::ITRF2020,
            AzimuthElevationConvention::ClockwiseFromNorthAboveHorizon,
            angles(1.0, 0.5, Some([[1.0, 0.0], [0.0, 1.0]])),
        )
        .expect("angles");
        let correction = AzimuthElevationCorrection::new(
            Troposphere,
            angles(0.1, 0.0, Some([[3.0, 0.0], [0.0, 3.0]])),
        );

        let corrected = correction.apply(&measurement).expect("corrected angles");
        let error = corrected.values().error().expect("known covariance");
        let lower = error
            .lower_triangular_matrix()
            .expect("stored lower matrix");
        assert_eq!(lower[0][1], Angle::new::<radian>(0.0));
        assert_eq!(lower[1][0], Angle::new::<radian>(0.0));
        assert!(lower[0][0] > Angle::new::<radian>(1.0));
        assert!(lower[1][1] > Angle::new::<radian>(1.0));
    }

    #[test]
    fn angular_corrections_cannot_silently_cross_declared_bounds() {
        let angle_measurement = AzimuthElevationMeasurement::new(
            path(),
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::ITRF2020,
            AzimuthElevationConvention::ClockwiseFromNorthAboveHorizon,
            angles(6.0, 0.0, None),
        )
        .expect("angles");
        let correction = AzimuthElevationCorrection::new(Instrument, angles(0.5, 0.0, None));

        assert_eq!(
            correction.apply(&angle_measurement),
            Err(CorrectionError::InvalidMeasurement(
                MeasurementError::AzimuthOutOfRange
            ))
        );
    }

    #[test]
    fn correction_provenance_is_open_to_downstream_models() {
        #[derive(Debug, Clone, Copy)]
        struct SolarPlasma;

        impl CorrectionKind for SolarPlasma {}

        let range = RangeMeasurement::new(
            path(),
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::ITRF2020,
            RangeConvention::PathLength,
            length(100.0, None),
        )
        .expect("range");
        let correction = AdditiveCorrection::new(SolarPlasma, length(1.0, None));

        assert_eq!(
            correction
                .apply(&range)
                .expect("corrected range")
                .value()
                .value(),
            Length::new::<meter>(101.0)
        );
    }
}
