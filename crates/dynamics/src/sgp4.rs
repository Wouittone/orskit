#![forbid(unsafe_code)]

//! Fixed-model SGP4 propagation.
//!
//! [`Sgp4Propagator`] is stateless and non-configurable. It advances
//! [`Sgp4Elements`] with WGS-72 and AFSPC-compatible SGP4/SDP4 behavior and
//! returns typed Cartesian states in TEME.

use std::f64::consts::PI;

use dynamics_core::Propagator;
use frames::ReferenceFrame;
use hifitime::Epoch;
use orbits::cartesian::{CartesianState, StateError};
use orskit_core::{Orbit, OrbitParts, SpacecraftState};
use sgp4::chrono::NaiveDate;
use thiserror::Error;
use units::uom::si::{
    angle::radian, angular_velocity::radian_per_second, length::kilometer, ratio::ratio,
    velocity::kilometer_per_second,
};
use units::{Angle, AngularVelocity, Length, Position, Ratio, Velocity, VelocityVector};

/// SGP4 mean elements at the epoch supplied by their surrounding [`Orbit`].
#[derive(Debug, Clone, PartialEq)]
pub struct Sgp4Elements {
    inclination: Angle,
    right_ascension_of_ascending_node: Angle,
    eccentricity: Ratio,
    argument_of_perigee: Angle,
    mean_anomaly: Angle,
    mean_motion: AngularVelocity,
    b_star_inverse_earth_radii: f64,
}

impl Sgp4Elements {
    /// Constructs validated SGP4 mean elements.
    pub fn new(
        inclination: Angle,
        right_ascension_of_ascending_node: Angle,
        eccentricity: Ratio,
        argument_of_perigee: Angle,
        mean_anomaly: Angle,
        mean_motion: AngularVelocity,
        b_star_inverse_earth_radii: f64,
    ) -> Result<Self, Sgp4ElementsError> {
        let inclination_radians = inclination.get::<radian>();
        if !inclination_radians.is_finite() || !(0.0..=PI).contains(&inclination_radians) {
            return Err(Sgp4ElementsError::Inclination);
        }
        for angle in [
            right_ascension_of_ascending_node,
            argument_of_perigee,
            mean_anomaly,
        ] {
            if !angle.get::<radian>().is_finite() {
                return Err(Sgp4ElementsError::Angle);
            }
        }
        let eccentricity_value = eccentricity.get::<ratio>();
        if !eccentricity_value.is_finite() || !(0.0..1.0).contains(&eccentricity_value) {
            return Err(Sgp4ElementsError::Eccentricity);
        }
        let mean_motion_value = mean_motion.get::<radian_per_second>();
        if !mean_motion_value.is_finite() || mean_motion_value <= 0.0 {
            return Err(Sgp4ElementsError::MeanMotion);
        }
        if !b_star_inverse_earth_radii.is_finite() {
            return Err(Sgp4ElementsError::BStar);
        }
        Ok(Self {
            inclination,
            right_ascension_of_ascending_node,
            eccentricity,
            argument_of_perigee,
            mean_anomaly,
            mean_motion,
            b_star_inverse_earth_radii,
        })
    }
}

impl SpacecraftState for Sgp4Elements {
    fn frame(&self) -> ReferenceFrame {
        ReferenceFrame::TEME
    }
}

/// Invalid model-specific mean elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum Sgp4ElementsError {
    /// Inclination is not finite or outside 0..=pi.
    #[error("SGP4 inclination must be finite and within 0..=pi radians")]
    Inclination,
    /// An angular element is not finite.
    #[error("SGP4 angular elements must be finite")]
    Angle,
    /// Eccentricity is not finite or outside 0..1.
    #[error("SGP4 eccentricity must be finite and within 0..1")]
    Eccentricity,
    /// Mean motion is not finite and positive.
    #[error("SGP4 mean motion must be finite and positive")]
    MeanMotion,
    /// B* is not finite.
    #[error("SGP4 B* must be finite")]
    BStar,
}

/// Failure while evaluating the fixed SGP4 model.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Sgp4Error {
    /// The input epoch cannot be represented by the dependency's civil time.
    #[error("SGP4 epoch is outside the supported civil-time range")]
    EpochOutOfRange,
    /// Mean motion could not be converted from the Kozai convention.
    #[error("invalid mean motion for SGP4")]
    KozaiElements(#[source] sgp4::KozaiElementsError),
    /// Eccentricity was invalid at model initialization.
    #[error("invalid epoch eccentricity for SGP4")]
    EpochEccentricity(#[source] sgp4::OutOfRangeEpochEccentricity),
    /// The model diverged at the requested epoch.
    #[error("SGP4 propagation failed")]
    Propagation(#[source] sgp4::Error),
    /// The dependency returned a non-finite Cartesian prediction.
    #[error("SGP4 returned an invalid Cartesian prediction")]
    CartesianState(#[source] StateError),
}

/// Stateless, non-configurable SGP4/SDP4 propagation using WGS-72.
///
/// ```
/// use std::f64::consts::TAU;
///
/// use dynamics_core::Propagator;
/// use dynamics::sgp4::{Sgp4Elements, Sgp4Propagator};
/// use hifitime::{Duration, Epoch};
/// use orskit_core::Orbit;
/// use units::uom::si::{
///     angle::radian, angular_velocity::radian_per_second, ratio::ratio,
/// };
/// use units::{Angle, AngularVelocity, Ratio};
///
/// let epoch = Epoch::from_gregorian_utc_at_midnight(2026, 1, 1);
/// let elements = Sgp4Elements::new(
///     Angle::new::<radian>(51.6_f64.to_radians()),
///     Angle::new::<radian>(20.0_f64.to_radians()),
///     Ratio::new::<ratio>(0.001),
///     Angle::new::<radian>(30.0_f64.to_radians()),
///     Angle::new::<radian>(40.0_f64.to_radians()),
///     AngularVelocity::new::<radian_per_second>(15.5 * TAU / 86_400.0),
///     1.0e-5,
/// )?;
/// let result = Sgp4Propagator.propagate(
///     Orbit::new(epoch, elements),
///     epoch + Duration::from_seconds(60.0),
/// )?;
/// assert_eq!(result.epoch(), epoch + Duration::from_seconds(60.0));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct Sgp4Propagator;

impl Propagator<Sgp4Elements, CartesianState> for Sgp4Propagator {
    type Error = Sgp4Error;

    fn propagate(
        &self,
        initial: Orbit<Sgp4Elements>,
        target: Epoch,
    ) -> Result<Orbit<CartesianState>, Self::Error> {
        let OrbitParts {
            epoch,
            state: elements,
        } = initial.into();
        let (year, month, day, hour, minute, second, nanosecond) = epoch.to_gregorian_utc();
        let datetime = NaiveDate::from_ymd_opt(year, u32::from(month), u32::from(day))
            .and_then(|date| {
                date.and_hms_nano_opt(
                    u32::from(hour),
                    u32::from(minute),
                    u32::from(second),
                    nanosecond,
                )
            })
            .ok_or(Sgp4Error::EpochOutOfRange)?;
        let model_epoch = sgp4::julian_years_since_j2000_afspc_compatibility_mode(&datetime);
        let orbit = sgp4::Orbit::from_kozai_elements(
            &sgp4::WGS72,
            elements.inclination.get::<radian>(),
            elements.right_ascension_of_ascending_node.get::<radian>(),
            elements.eccentricity.get::<ratio>(),
            elements.argument_of_perigee.get::<radian>(),
            elements.mean_anomaly.get::<radian>(),
            elements.mean_motion.get::<radian_per_second>() * 60.0,
        )
        .map_err(Sgp4Error::KozaiElements)?;
        let constants = sgp4::Constants::new(
            sgp4::WGS72,
            sgp4::afspc_epoch_to_sidereal_time,
            model_epoch,
            elements.b_star_inverse_earth_radii,
            orbit,
        )
        .map_err(Sgp4Error::EpochEccentricity)?;
        let minutes = (target - epoch).to_seconds() / 60.0;
        let prediction = constants
            .propagate_afspc_compatibility_mode(sgp4::MinutesSinceEpoch(minutes))
            .map_err(Sgp4Error::Propagation)?;
        let state = CartesianState::new(
            ReferenceFrame::TEME,
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

#[cfg(test)]
mod tests {
    use std::error::Error as _;
    use std::f64::consts::TAU;

    use hifitime::Duration;

    use super::*;

    const POSITION_TOLERANCE_METRES: f64 = 1.0;
    const VELOCITY_TOLERANCE_METRES_PER_SECOND: f64 = 0.001;

    #[test]
    fn published_near_earth_teme_vectors_match() {
        let initial = elements(
            Epoch::from_gregorian_utc(2000, 6, 27, 18, 50, 19, 733_568_000),
            [
                34.2682,
                348.7242,
                0.1859667,
                331.7664,
                19.3264,
                10.82419157,
                2.8098e-5,
            ],
        );
        assert_prediction(
            initial,
            360.0,
            [-7_154.031_202_02, -3_783.176_825_04, -3_536.194_122_94],
            [4.741_887_409, -4.151_817_765, -2.093_935_425],
        );
    }

    #[test]
    fn published_deep_space_teme_vectors_match() {
        let initial = elements(
            Epoch::from_gregorian_utc(2004, 1, 31, 21, 51, 25, 308_576_000),
            [
                11.4628, 273.1101, 0.1450506, 207.6000, 143.9350, 1.20231981, 1.0e-4,
            ],
        );
        assert_prediction(
            initial,
            -5_184.0,
            [-29_020.025_871_28, 13_819.844_190_63, -5_713.336_791_83],
            [-1.768_068_390, -3.235_371_192, -0.395_206_135],
        );
    }

    #[test]
    fn propagation_failure_preserves_its_source() {
        let initial = elements(
            Epoch::from_gregorian_utc(2000, 6, 27, 18, 50, 19, 733_568_000),
            [
                34.2682,
                348.7242,
                0.1859667,
                331.7664,
                19.3264,
                10.82419157,
                9.9999e8,
            ],
        );
        let target = initial.epoch() + Duration::from_seconds(1.0e12);
        let error = Sgp4Propagator
            .propagate(initial, target)
            .expect_err("extreme extrapolation diverges");
        assert!(matches!(error, Sgp4Error::Propagation(_)));
        assert!(error.source().is_some());
    }

    fn elements(epoch: Epoch, values: [f64; 7]) -> Orbit<Sgp4Elements> {
        Orbit::new(
            epoch,
            Sgp4Elements::new(
                Angle::new::<radian>(values[0].to_radians()),
                Angle::new::<radian>(values[1].to_radians()),
                Ratio::new::<ratio>(values[2]),
                Angle::new::<radian>(values[3].to_radians()),
                Angle::new::<radian>(values[4].to_radians()),
                AngularVelocity::new::<radian_per_second>(values[5] * TAU / 86_400.0),
                values[6],
            )
            .expect("valid elements"),
        )
    }

    fn assert_prediction(
        initial: Orbit<Sgp4Elements>,
        minutes_since_epoch: f64,
        expected_position_kilometres: [f64; 3],
        expected_velocity_kilometres_per_second: [f64; 3],
    ) {
        let target = initial.epoch() + Duration::from_seconds(minutes_since_epoch * 60.0);
        let prediction = Sgp4Propagator
            .propagate(initial, target)
            .expect("published prediction");
        assert_eq!(prediction.epoch(), target);
        assert_eq!(prediction.as_ref().frame(), ReferenceFrame::TEME);
        assert_vector_close(
            prediction.as_ref().position().to_metres(),
            expected_position_kilometres.map(|value| value * 1_000.0),
            POSITION_TOLERANCE_METRES,
        );
        assert_vector_close(
            prediction.as_ref().velocity().to_metres_per_second(),
            expected_velocity_kilometres_per_second.map(|value| value * 1_000.0),
            VELOCITY_TOLERANCE_METRES_PER_SECOND,
        );
    }

    fn assert_vector_close(actual: [f64; 3], expected: [f64; 3], tolerance: f64) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!((actual - expected).abs() <= tolerance);
        }
    }
}
