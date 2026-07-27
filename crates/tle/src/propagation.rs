use std::f64::consts::{PI, TAU};

use hifitime::Epoch;
use orbits::cartesian::{CartesianState, StateError};
use orskit_core::Orbit;
use sgp4::chrono::{Datelike, Duration, NaiveDate, Timelike};
use thiserror::Error;
use units::uom::si::{length::kilometer, velocity::kilometer_per_second};
use units::{Length, Position, Velocity, VelocityVector};

use crate::TwoLineElement;

const MINUTES_PER_DAY: f64 = 1_440.0;
const NANOSECONDS_PER_EPOCH_FRACTION: u64 = 864_000;

/// Failure while constructing or evaluating an SGP4 propagator.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Sgp4Error {
    /// The TLE explicitly selects a legacy propagator other than the
    /// distributed-data default SGP4/SDP4 model.
    #[error("TLE ephemeris type {found} is not supported; expected distributed-data type 0")]
    UnsupportedEphemerisType {
        /// Unsupported fixed-column ephemeris type.
        found: u8,
    },
    /// The validated TLE epoch could not be represented by the dependency's
    /// civil-time type.
    #[error("TLE epoch is outside the supported SGP4 civil-time range")]
    EpochOutOfRange,
    /// Mean motion could not be converted from the TLE's Kozai convention.
    #[error("invalid TLE mean motion for SGP4")]
    KozaiElements(#[source] sgp4::KozaiElementsError),
    /// Eccentricity was invalid at model initialization.
    #[error("invalid epoch eccentricity for SGP4")]
    EpochEccentricity(#[source] sgp4::OutOfRangeEpochEccentricity),
    /// The SGP4 model diverged at the requested epoch.
    #[error("SGP4 propagation failed")]
    Propagation(#[source] sgp4::Error),
    /// The dependency returned a non-finite Cartesian prediction.
    #[error("SGP4 returned an invalid Cartesian prediction")]
    CartesianState(#[source] StateError),
}

/// Immutable SGP4 model initialized from one strict [`TwoLineElement`].
///
/// The model uses WGS-72 and the AFSPC-compatible epoch, sidereal-time, and
/// propagation modes used by the published Vallado/CelesTrak verification
/// cases. Results are geocentric TEME coordinates. No conversion from TEME to
/// GCRF, EME2000, or a terrestrial frame is performed.
///
/// Elapsed time is measured in signed SI minutes from the TLE's UTC epoch.
/// Consequently, a request spanning an inserted UTC leap second differs from
/// a convention that assigns every UTC civil day exactly 1,440 minutes.
///
/// ```
/// use hifitime::Duration;
/// use tle::{Sgp4Propagator, TwoLineElement};
///
/// let elements = TwoLineElement::parse(
///     "1 00005U 58002B   00179.78495062  .00000023  00000-0  28098-4 0  4753",
///     "2 00005  34.2682 348.7242 1859667 331.7664  19.3264 10.82419157413667",
/// )?;
/// let propagator = Sgp4Propagator::from_tle(&elements)?;
/// let prediction =
///     propagator.propagate(propagator.epoch() + Duration::from_seconds(360.0 * 60.0))?;
///
/// assert_eq!(prediction.as_ref().frame(), frames::ReferenceFrame::TEME);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone)]
pub struct Sgp4Propagator {
    epoch: Epoch,
    constants: sgp4::Constants,
}

impl Sgp4Propagator {
    /// Initializes the model directly from the project's validated TLE fields.
    ///
    /// This does not call the dependency's TLE parser.
    pub fn from_tle(tle: &TwoLineElement) -> Result<Self, Sgp4Error> {
        if tle.ephemeris_type() != 0 {
            return Err(Sgp4Error::UnsupportedEphemerisType {
                found: tle.ephemeris_type(),
            });
        }
        let datetime = tle_datetime(tle)?;
        let epoch = Epoch::from_gregorian_utc(
            datetime.year(),
            datetime.month() as u8,
            datetime.day() as u8,
            datetime.hour() as u8,
            datetime.minute() as u8,
            datetime.second() as u8,
            datetime.nanosecond(),
        );
        let model_epoch = sgp4::julian_years_since_j2000_afspc_compatibility_mode(&datetime);
        let orbit = sgp4::Orbit::from_kozai_elements(
            &sgp4::WGS72,
            tle.inclination_deg() * PI / 180.0,
            tle.right_ascension_of_ascending_node_deg() * PI / 180.0,
            tle.eccentricity(),
            tle.argument_of_perigee_deg() * PI / 180.0,
            tle.mean_anomaly_deg() * PI / 180.0,
            tle.mean_motion_rev_per_day() * TAU / MINUTES_PER_DAY,
        )
        .map_err(Sgp4Error::KozaiElements)?;
        let constants = sgp4::Constants::new(
            sgp4::WGS72,
            sgp4::afspc_epoch_to_sidereal_time,
            model_epoch,
            tle.b_star_inverse_earth_radii(),
            orbit,
        )
        .map_err(Sgp4Error::EpochEccentricity)?;

        Ok(Self { epoch, constants })
    }

    /// Returns the UTC epoch encoded by the source TLE.
    #[must_use]
    pub const fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Propagates to `target` and returns a typed Cartesian state in TEME.
    ///
    /// SGP4 is a TLE-specific model. Accuracy degrades as the requested epoch
    /// moves outside the useful age of the source element set. A propagation
    /// error is not promoted into a separate operational decay classification.
    pub fn propagate(&self, target: Epoch) -> Result<Orbit<CartesianState>, Sgp4Error> {
        let minutes = (target - self.epoch).to_seconds() / 60.0;
        let prediction = self
            .constants
            .propagate_afspc_compatibility_mode(sgp4::MinutesSinceEpoch(minutes))
            .map_err(Sgp4Error::Propagation)?;
        let state = CartesianState::new(
            frames::ReferenceFrame::TEME,
            Position::new(
                Length::new::<kilometer>(prediction.position[0]),
                Length::new::<kilometer>(prediction.position[1]),
                Length::new::<kilometer>(prediction.position[2]),
            ),
            VelocityVector::new(
                Velocity::new::<kilometer_per_second>(prediction.velocity[0]),
                Velocity::new::<kilometer_per_second>(prediction.velocity[1]),
                Velocity::new::<kilometer_per_second>(prediction.velocity[2]),
            ),
        )
        .map_err(Sgp4Error::CartesianState)?;

        Ok(Orbit::new(target, state))
    }
}

fn tle_datetime(tle: &TwoLineElement) -> Result<sgp4::chrono::NaiveDateTime, Sgp4Error> {
    let scaled = tle.epoch_day_scaled;
    let whole_day = (scaled / super::SCALE_8) as i64;
    let fraction = scaled % super::SCALE_8;
    let nanoseconds = fraction * NANOSECONDS_PER_EPOCH_FRACTION;
    let seconds = nanoseconds / 1_000_000_000;
    let subsecond = (nanoseconds % 1_000_000_000) as u32;

    NaiveDate::from_ymd_opt(i32::from(tle.epoch_year()), 1, 1)
        .and_then(|date| date.checked_add_signed(Duration::days(whole_day - 1)))
        .and_then(|date| {
            date.and_hms_nano_opt(
                (seconds / 3_600) as u32,
                ((seconds % 3_600) / 60) as u32,
                (seconds % 60) as u32,
                subsecond,
            )
        })
        .ok_or(Sgp4Error::EpochOutOfRange)
}

#[cfg(test)]
mod tests {
    use hifitime::Duration;

    use super::*;

    const NEAR_EARTH_LINE_1: &str =
        "1 00005U 58002B   00179.78495062  .00000023  00000-0  28098-4 0  4753";
    const NEAR_EARTH_LINE_2: &str =
        "2 00005  34.2682 348.7242 1859667 331.7664  19.3264 10.82419157413667";
    const DEEP_SPACE_LINE_1: &str =
        "1 04632U 70093B   04031.91070959 -.00000084  00000-0  10000-3 0  9955";
    const DEEP_SPACE_LINE_2: &str =
        "2 04632  11.4628 273.1101 1450506 207.6000 143.9350  1.20231981 44145";

    // Vallado et al., AIAA-2006-6753 Revision 3, Appendix E:
    // https://celestrak.org/publications/AIAA/2006-6753/AIAA-2006-6753-Rev3.pdf
    //
    // The paper prints positions to 1e-8 km and velocities to 1e-9 km/s. The
    // acceptance bounds allow cross-platform floating-point variation without
    // implying operational orbit accuracy.
    const POSITION_TOLERANCE_METRES: f64 = 1.0;
    const VELOCITY_TOLERANCE_METRES_PER_SECOND: f64 = 0.001;

    #[test]
    fn published_near_earth_teme_vectors_match_at_multiple_times() {
        let tle =
            TwoLineElement::parse(NEAR_EARTH_LINE_1, NEAR_EARTH_LINE_2).expect("published TLE");
        let propagator = Sgp4Propagator::from_tle(&tle).expect("valid SGP4 model");

        assert_eq!(
            propagator.epoch(),
            Epoch::from_gregorian_utc(2000, 6, 27, 18, 50, 19, 733_568_000)
        );
        assert_prediction(
            &propagator,
            0.0,
            [7_022.465_292_66, -1_400.082_967_55, 0.039_951_55],
            [1.893_841_015, 6.405_893_759, 4.534_807_250],
        );
        assert_prediction(
            &propagator,
            360.0,
            [-7_154.031_202_02, -3_783.176_825_04, -3_536.194_122_94],
            [4.741_887_409, -4.151_817_765, -2.093_935_425],
        );
        assert_prediction(
            &propagator,
            4_320.0,
            [-9_060.473_735_69, 4_658.709_525_02, 813.686_731_53],
            [-2.232_832_783, -4.110_453_490, -3.157_345_433],
        );
    }

    #[test]
    fn published_deep_space_teme_vectors_match_at_multiple_times() {
        let tle =
            TwoLineElement::parse(DEEP_SPACE_LINE_1, DEEP_SPACE_LINE_2).expect("published TLE");
        let propagator = Sgp4Propagator::from_tle(&tle).expect("valid SGP4 model");

        assert_eq!(
            propagator.epoch(),
            Epoch::from_gregorian_utc(2004, 1, 31, 21, 51, 25, 308_576_000)
        );
        assert_prediction(
            &propagator,
            0.0,
            [2_334.114_500_85, -41_920.440_353_49, -0.038_674_37],
            [2.826_321_032, -0.065_091_664, 0.570_936_053],
        );
        assert_prediction(
            &propagator,
            -5_184.0,
            [-29_020.025_871_28, 13_819.844_190_63, -5_713.336_791_83],
            [-1.768_068_390, -3.235_371_192, -0.395_206_135],
        );
        assert_prediction(
            &propagator,
            -4_896.0,
            [-15_129.946_945_45, -36_907.745_262_21, -3_487.562_567_01],
            [2.581_167_187, -1.524_204_737, 0.504_805_763],
        );
    }

    #[test]
    fn rejects_explicit_legacy_ephemeris_models() {
        for ephemeris_type in 1..=9 {
            let line_one =
                replace_field_and_checksum(NEAR_EARTH_LINE_1, 62..63, &ephemeris_type.to_string());
            let tle = TwoLineElement::parse(&line_one, NEAR_EARTH_LINE_2)
                .expect("syntactically valid legacy model selector");
            assert!(matches!(
                Sgp4Propagator::from_tle(&tle),
                Err(Sgp4Error::UnsupportedEphemerisType { found })
                    if found == ephemeris_type
            ));
        }
    }

    #[test]
    fn constructible_propagation_failure_preserves_its_source() {
        use std::error::Error as _;

        let line_one = replace_field_and_checksum(NEAR_EARTH_LINE_1, 53..61, " 99999+9");
        let tle = TwoLineElement::parse(&line_one, NEAR_EARTH_LINE_2)
            .expect("extreme but syntactically valid drag term");
        let propagator = Sgp4Propagator::from_tle(&tle).expect("model initializes");
        let error = propagator
            .propagate(propagator.epoch() + Duration::from_seconds(1.0e12))
            .expect_err("extreme extrapolation diverges");
        assert!(matches!(error, Sgp4Error::Propagation(_)));
        assert!(error.source().is_some());
    }

    fn assert_prediction(
        propagator: &Sgp4Propagator,
        minutes_since_epoch: f64,
        expected_position_kilometres: [f64; 3],
        expected_velocity_kilometres_per_second: [f64; 3],
    ) {
        let target = propagator.epoch() + Duration::from_seconds(minutes_since_epoch * 60.0);
        let prediction = propagator.propagate(target).expect("published prediction");

        assert_eq!(prediction.epoch(), target);
        assert_eq!(prediction.as_ref().frame(), frames::ReferenceFrame::TEME);
        assert_vector_close(
            prediction.as_ref().position().to_metres(),
            expected_position_kilometres.map(|component| component * 1_000.0),
            POSITION_TOLERANCE_METRES,
        );
        assert_vector_close(
            prediction.as_ref().velocity().to_metres_per_second(),
            expected_velocity_kilometres_per_second.map(|component| component * 1_000.0),
            VELOCITY_TOLERANCE_METRES_PER_SECOND,
        );
    }

    fn assert_vector_close(actual: [f64; 3], expected: [f64; 3], tolerance: f64) {
        for (component, expected_component) in actual.into_iter().zip(expected) {
            assert!(
                (component - expected_component).abs() <= tolerance,
                "expected {expected_component}, got {component}, tolerance {tolerance}"
            );
        }
    }

    fn replace_field_and_checksum(
        source: &str,
        range: std::ops::Range<usize>,
        replacement: &str,
    ) -> String {
        let mut result = source.to_owned();
        result.replace_range(range, replacement);
        let calculated = crate::checksum(&result.as_bytes()[..68]);
        result.replace_range(68..69, &calculated.to_string());
        result
    }
}
