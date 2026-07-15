//! Feature-gated, ground-based observation implementations.
//!
//! Each type owns its signal path, epoch, frame, and every additional
//! ground-station role explicitly.
//! Branched observations (such as TDOA and FDOA) do not fabricate a sequential
//! signal path for the second receiver; it is named separately instead.

use thiserror::Error;

use frames::ReferenceFrame;
use hifitime::Epoch;

#[cfg(any(
    feature = "bistatic-range",
    feature = "bistatic-range-rate",
    feature = "turnaround-range",
    feature = "tdoa",
    feature = "fdoa",
    feature = "phase"
))]
use crate::ParticipantId;
use crate::{Measurement, MeasurementError, MeasurementKind, SignalPath};

#[cfg(feature = "angular-ra-dec")]
use units::uom::si::angle::radian;
#[cfg(any(feature = "angular-ra-dec", feature = "phase"))]
use units::Angle;
#[cfg(any(feature = "fdoa", feature = "phase"))]
use units::Frequency;
#[cfg(any(feature = "bistatic-range", feature = "turnaround-range"))]
use units::Length;
#[cfg(feature = "tdoa")]
use units::Time;
#[cfg(feature = "bistatic-range-rate")]
use units::Velocity;

#[cfg(any(
    feature = "bistatic-range",
    feature = "bistatic-range-rate",
    feature = "turnaround-range",
    feature = "tdoa",
    feature = "fdoa",
    feature = "phase"
))]
use crate::Measured;
#[cfg(feature = "angular-ra-dec")]
use crate::MeasurementValues;

/// A distinct pair of ground-station identities used by a multi-station observation.
#[cfg(any(
    feature = "bistatic-range",
    feature = "bistatic-range-rate",
    feature = "turnaround-range",
    feature = "tdoa",
    feature = "fdoa"
))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GroundStationPair {
    primary: ParticipantId,
    secondary: ParticipantId,
}

#[cfg(any(
    feature = "bistatic-range",
    feature = "bistatic-range-rate",
    feature = "turnaround-range",
    feature = "tdoa",
    feature = "fdoa"
))]
impl GroundStationPair {
    /// Creates two explicitly distinct ground-station roles.
    pub fn new(
        primary: ParticipantId,
        secondary: ParticipantId,
    ) -> Result<Self, GroundObservationError> {
        if primary == secondary {
            return Err(GroundObservationError::SameGroundStation);
        }
        Ok(Self { primary, secondary })
    }

    /// Returns the observation's primary station.
    #[must_use]
    pub const fn primary(&self) -> &ParticipantId {
        &self.primary
    }

    /// Returns the observation's secondary station.
    #[must_use]
    pub const fn secondary(&self) -> &ParticipantId {
        &self.secondary
    }
}

/// Invalid ground-observation metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum GroundObservationError {
    /// Two roles that require separate stations named one participant.
    #[error("a multi-station observation requires distinct ground stations")]
    SameGroundStation,
    /// Right ascension does not use the declared normalized interval.
    #[error("right ascension must be in [0, 2π)")]
    RightAscensionOutOfRange,
    /// Declination does not use the declared equatorial interval.
    #[error("declination must be in [-π/2, π/2]")]
    DeclinationOutOfRange,
    /// A shared angular observation value was invalid.
    #[error(transparent)]
    Measurement(#[from] MeasurementError),
}

#[cfg(any(
    feature = "angular-ra-dec",
    feature = "bistatic-range",
    feature = "bistatic-range-rate",
    feature = "turnaround-range",
    feature = "tdoa",
    feature = "fdoa",
    feature = "phase"
))]
macro_rules! ground_kind {
    ($(#[$meta:meta])* $name:ident, $identifier:literal) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name;

        impl MeasurementKind for $name {
            fn name(&self) -> &'static str {
                $identifier
            }
        }
    };
}

#[cfg(feature = "angular-ra-dec")]
ground_kind!(
    /// Right-ascension/declination observation family.
    RightAscensionDeclinationKind,
    "right-ascension-declination"
);
#[cfg(feature = "bistatic-range")]
ground_kind!(
    /// Bistatic range observation family.
    BistaticRangeKind,
    "bistatic-range"
);
#[cfg(feature = "bistatic-range-rate")]
ground_kind!(
    /// Bistatic range-rate observation family.
    BistaticRangeRateKind,
    "bistatic-range-rate"
);
#[cfg(feature = "turnaround-range")]
ground_kind!(
    /// Turn-around range observation family.
    TurnaroundRangeKind,
    "turnaround-range"
);
#[cfg(feature = "tdoa")]
ground_kind!(
    /// Time-difference-of-arrival observation family.
    TdoaKind,
    "tdoa"
);
#[cfg(feature = "fdoa")]
ground_kind!(
    /// Frequency-difference-of-arrival observation family.
    FdoaKind,
    "fdoa"
);
#[cfg(feature = "phase")]
ground_kind!(
    /// Ground-received carrier-phase observation family.
    PhaseKind,
    "phase"
);

/// Equatorial convention for a ground optical observation.
#[cfg(feature = "angular-ra-dec")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RightAscensionDeclinationConvention {
    /// Right ascension increases eastward and declination northward in the context frame.
    Equatorial,
}

/// A ground-based right-ascension/declination observation.
#[cfg(feature = "angular-ra-dec")]
#[derive(Debug, Clone, PartialEq)]
pub struct RightAscensionDeclinationMeasurement {
    path: SignalPath,
    epoch: Epoch,
    frame: ReferenceFrame,
    convention: RightAscensionDeclinationConvention,
    values: MeasurementValues<Angle, 2>,
}

#[cfg(feature = "angular-ra-dec")]
impl RightAscensionDeclinationMeasurement {
    /// Creates an equatorial right-ascension/declination observation.
    pub fn new(
        path: SignalPath,
        epoch: Epoch,
        frame: ReferenceFrame,
        convention: RightAscensionDeclinationConvention,
        values: MeasurementValues<Angle, 2>,
    ) -> Result<Self, GroundObservationError> {
        let [right_ascension, declination] = *values.values();
        if !(0.0..std::f64::consts::TAU).contains(&right_ascension.get::<radian>()) {
            return Err(GroundObservationError::RightAscensionOutOfRange);
        }
        if !(-std::f64::consts::FRAC_PI_2..=std::f64::consts::FRAC_PI_2)
            .contains(&declination.get::<radian>())
        {
            return Err(GroundObservationError::DeclinationOutOfRange);
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
    pub const fn convention(&self) -> RightAscensionDeclinationConvention {
        self.convention
    }

    /// Returns right ascension and declination with their joint covariance state.
    #[must_use]
    pub const fn values(&self) -> &MeasurementValues<Angle, 2> {
        &self.values
    }
}

#[cfg(feature = "angular-ra-dec")]
impl Measurement for RightAscensionDeclinationMeasurement {
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
        &RightAscensionDeclinationKind
    }
}

#[cfg(any(
    feature = "bistatic-range",
    feature = "bistatic-range-rate",
    feature = "turnaround-range",
    feature = "tdoa"
))]
macro_rules! paired_scalar_measurement {
    ($(#[$meta:meta])* $name:ident, $kind:ident, $quantity:ty) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name {
            path: SignalPath,
            epoch: Epoch,
            frame: ReferenceFrame,
            stations: GroundStationPair,
            value: Measured<$quantity>,
        }

        impl $name {
            /// Creates an observation with explicitly named primary and secondary stations.
            #[must_use]
            pub const fn new(
                path: SignalPath,
                epoch: Epoch,
                frame: ReferenceFrame,
                stations: GroundStationPair,
                value: Measured<$quantity>,
            ) -> Self {
                Self { path, epoch, frame, stations, value }
            }

            /// Returns the two ground-station roles.
            #[must_use]
            pub const fn stations(&self) -> &GroundStationPair {
                &self.stations
            }

            /// Returns the observed value and its explicit uncertainty state.
            #[must_use]
            pub const fn value(&self) -> Measured<$quantity> {
                self.value
            }
        }

        impl Measurement for $name {
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
                &$kind
            }
        }
    };
}

#[cfg(feature = "bistatic-range")]
paired_scalar_measurement!(
    /// Ground-to-spacecraft-to-ground bistatic range, tagged at its receiving station.
    BistaticRangeMeasurement,
    BistaticRangeKind,
    Length
);
#[cfg(feature = "bistatic-range-rate")]
paired_scalar_measurement!(
    /// Ground-to-spacecraft-to-ground bistatic range rate, tagged at its receiving station.
    BistaticRangeRateMeasurement,
    BistaticRangeRateKind,
    Velocity
);
#[cfg(feature = "turnaround-range")]
paired_scalar_measurement!(
    /// Turn-around range between explicitly named primary and secondary ground stations.
    TurnaroundRangeMeasurement,
    TurnaroundRangeKind,
    Length
);
#[cfg(feature = "tdoa")]
paired_scalar_measurement!(
    /// Difference between arrivals at the primary and secondary ground stations.
    TdoaMeasurement,
    TdoaKind,
    Time
);

/// Frequency difference of arrival at two ground stations.
#[cfg(feature = "fdoa")]
#[derive(Debug, Clone, PartialEq)]
pub struct FdoaMeasurement {
    path: SignalPath,
    epoch: Epoch,
    frame: ReferenceFrame,
    stations: GroundStationPair,
    emitter_frequency: Frequency,
    value: Measured<Frequency>,
}

#[cfg(feature = "fdoa")]
impl FdoaMeasurement {
    /// Creates an FDOA observation as primary-station frequency minus secondary-station frequency.
    #[must_use]
    pub const fn new(
        path: SignalPath,
        epoch: Epoch,
        frame: ReferenceFrame,
        stations: GroundStationPair,
        emitter_frequency: Frequency,
        value: Measured<Frequency>,
    ) -> Self {
        Self {
            path,
            epoch,
            frame,
            stations,
            emitter_frequency,
            value,
        }
    }

    /// Returns the explicitly named primary and secondary stations.
    #[must_use]
    pub const fn stations(&self) -> &GroundStationPair {
        &self.stations
    }

    /// Returns the transmitter centre frequency.
    #[must_use]
    pub const fn emitter_frequency(&self) -> Frequency {
        self.emitter_frequency
    }

    /// Returns the observed frequency difference and its explicit uncertainty state.
    #[must_use]
    pub const fn value(&self) -> Measured<Frequency> {
        self.value
    }
}

#[cfg(feature = "fdoa")]
impl Measurement for FdoaMeasurement {
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
        &FdoaKind
    }
}

/// Ground-received carrier phase expressed as an angle, with its carrier frequency.
#[cfg(feature = "phase")]
#[derive(Debug, Clone, PartialEq)]
pub struct PhaseMeasurement {
    path: SignalPath,
    epoch: Epoch,
    frame: ReferenceFrame,
    receiver: ParticipantId,
    carrier_frequency: Frequency,
    value: Measured<Angle>,
}

#[cfg(feature = "phase")]
impl PhaseMeasurement {
    /// Creates a ground-received carrier-phase observation.
    #[must_use]
    pub const fn new(
        path: SignalPath,
        epoch: Epoch,
        frame: ReferenceFrame,
        receiver: ParticipantId,
        carrier_frequency: Frequency,
        value: Measured<Angle>,
    ) -> Self {
        Self {
            path,
            epoch,
            frame,
            receiver,
            carrier_frequency,
            value,
        }
    }

    /// Returns the ground receiver identity.
    #[must_use]
    pub const fn receiver(&self) -> &ParticipantId {
        &self.receiver
    }

    /// Returns the carrier frequency.
    #[must_use]
    pub const fn carrier_frequency(&self) -> Frequency {
        self.carrier_frequency
    }

    /// Returns carrier phase as an angle and its explicit uncertainty state.
    #[must_use]
    pub const fn value(&self) -> Measured<Angle> {
        self.value
    }
}

#[cfg(feature = "phase")]
impl Measurement for PhaseMeasurement {
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
        &PhaseKind
    }
}

#[cfg(all(
    test,
    feature = "angular-ra-dec",
    feature = "bistatic-range",
    feature = "bistatic-range-rate",
    feature = "turnaround-range",
    feature = "tdoa",
    feature = "fdoa",
    feature = "phase"
))]
mod tests {
    use frames::ReferenceFrame;
    use hifitime::Epoch;
    use units::uom::si::{
        angle::radian, frequency::hertz, length::meter, time::second, velocity::meter_per_second,
    };
    use units::{Angle, AngularVariance, Frequency, Length, Time, Velocity};

    use super::*;
    use crate::{MeasurementUncertaintyInput, ParticipantId, SignalPath};

    fn id(value: &str) -> ParticipantId {
        ParticipantId::new(value).expect("participant")
    }

    fn path() -> SignalPath {
        SignalPath::new(vec![id("DSS-14"), id("SC-01"), id("DSS-25")]).expect("path")
    }

    fn pair() -> GroundStationPair {
        GroundStationPair::new(id("DSS-14"), id("DSS-25")).expect("distinct stations")
    }

    #[test]
    fn ground_observation_types_preserve_roles_units_and_joint_covariance() {
        let angles = MeasurementValues::new(
            [Angle::new::<radian>(1.0), Angle::new::<radian>(0.5)],
            Some(MeasurementUncertaintyInput::Covariance([
                [
                    AngularVariance::from_square_radians(1.0),
                    AngularVariance::from_square_radians(0.0),
                ],
                [
                    AngularVariance::from_square_radians(0.0),
                    AngularVariance::from_square_radians(1.0),
                ],
            ])),
        )
        .expect("angles");
        let ra_dec = RightAscensionDeclinationMeasurement::new(
            path(),
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::GCRF,
            RightAscensionDeclinationConvention::Equatorial,
            angles,
        )
        .expect("RA/Dec");
        assert_eq!(ra_dec.kind().name(), "right-ascension-declination");

        let range = Measured::new([Length::new::<meter>(100.0)], None).expect("range");
        assert_eq!(
            BistaticRangeMeasurement::new(
                path(),
                Epoch::from_tai_seconds(1.0),
                ReferenceFrame::GCRF,
                pair(),
                range,
            )
            .kind()
            .name(),
            "bistatic-range"
        );
        assert_eq!(
            TurnaroundRangeMeasurement::new(
                path(),
                Epoch::from_tai_seconds(2.0),
                ReferenceFrame::GCRF,
                pair(),
                range,
            )
            .stations()
            .primary()
            .as_str(),
            "DSS-14"
        );

        let rate = Measured::new([Velocity::new::<meter_per_second>(1.0)], None).expect("rate");
        assert_eq!(
            BistaticRangeRateMeasurement::new(
                path(),
                Epoch::from_tai_seconds(3.0),
                ReferenceFrame::GCRF,
                pair(),
                rate,
            )
            .kind()
            .name(),
            "bistatic-range-rate"
        );

        let delay = Measured::new([Time::new::<second>(0.001)], None).expect("delay");
        assert_eq!(
            TdoaMeasurement::new(
                path(),
                Epoch::from_tai_seconds(4.0),
                ReferenceFrame::GCRF,
                pair(),
                delay,
            )
            .kind()
            .name(),
            "tdoa"
        );

        let difference = Measured::new([Frequency::new::<hertz>(2.0)], None).expect("FDOA");
        assert_eq!(
            FdoaMeasurement::new(
                path(),
                Epoch::from_tai_seconds(5.0),
                ReferenceFrame::GCRF,
                pair(),
                Frequency::new::<hertz>(8.4e9),
                difference
            )
            .kind()
            .name(),
            "fdoa"
        );

        let phase = Measured::new([Angle::new::<radian>(0.1)], None).expect("phase");
        assert_eq!(
            PhaseMeasurement::new(
                path(),
                Epoch::from_tai_seconds(6.0),
                ReferenceFrame::GCRF,
                id("DSS-25"),
                Frequency::new::<hertz>(8.4e9),
                phase
            )
            .kind()
            .name(),
            "phase"
        );
    }
}
