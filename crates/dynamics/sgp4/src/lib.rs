#![forbid(unsafe_code)]

//! SGP4/SDP4 propagation to geocentric TEME.
//!
//! This crate uses the published Vallado SGP4 model through the unmodified,
//! MIT-licensed `sgp4` crate. It uses WGS-72 and AFSPC-compatible time
//! conventions. Returned position and velocity are converted from the
//! dependency's kilometres and kilometres per second to SI units immediately.
//! No TEME frame conversion is performed.
//!
//! The current dynamics contract has the same input and output state type.
//! Therefore an [`Sgp4Propagator`] is constructed from mean elements and their
//! epoch, and [`Sgp4Propagator::initial_orbit`] supplies its validated seed.
//! The `Propagator<CartesianState>` implementation accepts only a seed exactly
//! matching this model's state at the seed epoch.
//!
//! # Example
//!
//! ```
//! use dynamics_core::Propagator;
//! use dynamics_sgp4::{Sgp4Elements, Sgp4Propagator};
//! use hifitime::{Duration, Epoch};
//! use units::uom::si::{
//!     angle::radian, angular_velocity::radian_per_second, ratio::ratio,
//! };
//! use units::{Angle, AngularVelocity, Ratio};
//!
//! let epoch = Epoch::from_gregorian_utc_at_midnight(2026, 1, 1);
//! let elements = Sgp4Elements::new(
//!     Angle::new::<radian>(51.6_f64.to_radians()),
//!     Angle::new::<radian>(20.0_f64.to_radians()),
//!     Ratio::new::<ratio>(0.001),
//!     Angle::new::<radian>(30.0_f64.to_radians()),
//!     Angle::new::<radian>(40.0_f64.to_radians()),
//!     AngularVelocity::new::<radian_per_second>(15.5 * std::f64::consts::TAU / 86_400.0),
//!     0.0,
//! )?;
//! let propagator = Sgp4Propagator::new(epoch, elements)?;
//! let result = propagator.propagate(
//!     propagator.initial_orbit(),
//!     epoch + Duration::from_seconds(60.0),
//! )?;
//! assert_eq!(result.epoch(), epoch + Duration::from_seconds(60.0));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::f64::consts::PI;

use dynamics_core::Propagator;
use frames::ReferenceFrame;
use hifitime::{Epoch, TimeScale};
use orbits::cartesian::{CartesianState, StateError};
use orskit_core::Orbit;
use sgp4::chrono::NaiveDate;
use thiserror::Error;
use units::uom::si::{
    angle::radian, angular_velocity::radian_per_second, length::kilometer, ratio::ratio,
    velocity::kilometer_per_second,
};
use units::{Angle, AngularVelocity, Length, Position, Ratio, Velocity, VelocityVector};

/// Validated SGP4 mean orbital elements.
#[derive(Debug, Clone, Copy, PartialEq)]
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
    /// Constructs validated mean elements; angles use radians and mean motion
    /// uses radians per SI second. B* is expressed in inverse Earth radii.
    pub fn new(
        inclination: Angle,
        right_ascension_of_ascending_node: Angle,
        eccentricity: Ratio,
        argument_of_perigee: Angle,
        mean_anomaly: Angle,
        mean_motion: AngularVelocity,
        b_star_inverse_earth_radii: f64,
    ) -> Result<Self, Sgp4ElementsError> {
        let inclination_value = inclination.get::<radian>();
        if !inclination_value.is_finite() || !(0.0..=PI).contains(&inclination_value) {
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

/// Invalid SGP4 mean-element input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum Sgp4ElementsError {
    /// Inclination is non-finite or outside 0..=pi.
    #[error("inclination must be finite and within 0..=pi radians")]
    Inclination,
    /// An angular element is non-finite.
    #[error("angular elements must be finite")]
    Angle,
    /// Eccentricity is non-finite or outside 0..1.
    #[error("eccentricity must be finite and within 0..1")]
    Eccentricity,
    /// Mean motion is non-finite or non-positive.
    #[error("mean motion must be finite and positive")]
    MeanMotion,
    /// B* is non-finite.
    #[error("B* must be finite")]
    BStar,
}

/// Failure while constructing or evaluating an SGP4 model.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Sgp4Error {
    /// The epoch is outside the civil calendar range accepted by the model.
    #[error("SGP4 epoch is outside the supported civil-time range")]
    EpochOutOfRange,
    /// Kozai mean-motion conversion failed.
    #[error("invalid mean motion for SGP4")]
    KozaiElements(#[from] sgp4::KozaiElementsError),
    /// Epoch eccentricity is outside the model's valid range.
    #[error("invalid epoch eccentricity for SGP4")]
    EpochEccentricity(#[from] sgp4::OutOfRangeEpochEccentricity),
    /// The model failed at the requested epoch.
    #[error("SGP4 propagation failed")]
    Propagation(#[from] sgp4::Error),
    /// The model produced a Cartesian state rejected by orskit.
    #[error("SGP4 returned an invalid Cartesian state")]
    CartesianState(#[from] StateError),
    /// The supplied seed does not match this TLE-derived model.
    #[error("initial Cartesian state does not match the SGP4 model at its epoch")]
    InitialStateMismatch,
}

/// SGP4/SDP4 propagator using WGS-72 and AFSPC-compatible conventions.
#[derive(Debug)]
pub struct Sgp4Propagator {
    epoch: Epoch,
    constants: sgp4::Constants,
    initial_state: CartesianState,
}

impl Sgp4Propagator {
    /// Initializes the fixed WGS-72 SGP4/SDP4 model from an epoch and mean
    /// elements.
    pub fn new(epoch: Epoch, elements: Sgp4Elements) -> Result<Self, Sgp4Error> {
        let epoch = epoch.to_time_scale(TimeScale::UTC);
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
        )?;
        let constants = sgp4::Constants::new(
            sgp4::WGS72,
            sgp4::afspc_epoch_to_sidereal_time,
            model_epoch,
            elements.b_star_inverse_earth_radii,
            orbit,
        )?;
        let initial_state = evaluate_model(&constants, epoch, epoch)?;
        Ok(Self {
            epoch,
            constants,
            initial_state: *initial_state.as_ref(),
        })
    }

    /// Returns the UTC epoch attached to the mean elements.
    #[must_use]
    pub const fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Returns the model's propagated Cartesian seed at its element epoch.
    #[must_use]
    pub fn initial_orbit(&self) -> Orbit<CartesianState> {
        Orbit::new(self.epoch, self.initial_state)
    }

    /// Evaluates the model at an absolute epoch.
    pub fn state_at(&self, target: Epoch) -> Result<Orbit<CartesianState>, Sgp4Error> {
        evaluate_model(&self.constants, self.epoch, target)
    }
}

fn evaluate_model(
    constants: &sgp4::Constants,
    epoch: Epoch,
    target: Epoch,
) -> Result<Orbit<CartesianState>, Sgp4Error> {
    let target = target.to_time_scale(TimeScale::UTC);
    let minutes = (target - epoch).to_seconds() / 60.0;
    let prediction =
        constants.propagate_afspc_compatibility_mode(sgp4::MinutesSinceEpoch(minutes))?;
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
    )?;
    Ok(Orbit::new(target, state))
}

impl Propagator<CartesianState> for Sgp4Propagator {
    type Error = Sgp4Error;

    fn propagate(
        &self,
        initial: Orbit<CartesianState>,
        target: Epoch,
    ) -> Result<Orbit<CartesianState>, Self::Error> {
        let expected = self.state_at(initial.epoch())?;
        if initial.as_ref() != expected.as_ref() {
            return Err(Sgp4Error::InitialStateMismatch);
        }
        self.state_at(target)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::*;
    use hifitime::Duration;
    use units::uom::si::{angle::degree, angular_velocity::radian_per_second};

    const POSITION_TOLERANCE_METRES: f64 = 1.0;
    const VELOCITY_TOLERANCE_METRES_PER_SECOND: f64 = 0.001;

    #[test]
    fn vallado_near_earth_verification_state_matches() {
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
    fn vallado_deep_space_verification_state_matches() {
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
    fn propagator_requires_a_seed_from_its_own_model() {
        let propagator = elements(
            Epoch::from_gregorian_utc_at_midnight(2026, 1, 1),
            [51.6, 20.0, 0.001, 30.0, 40.0, 15.5, 0.0],
        );
        let other = elements(
            Epoch::from_gregorian_utc_at_midnight(2026, 1, 1),
            [52.0, 20.0, 0.001, 30.0, 40.0, 15.5, 0.0],
        );
        assert!(matches!(
            propagator.propagate(other.initial_orbit(), other.epoch()),
            Err(Sgp4Error::InitialStateMismatch)
        ));
    }

    #[test]
    fn propagation_failures_retain_the_dependency_error_source() {
        let propagator = elements(
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
        let target = propagator.epoch() + Duration::from_seconds(1.0e12);
        let error = propagator
            .state_at(target)
            .expect_err("extreme arc diverges");
        assert!(matches!(error, Sgp4Error::Propagation(_)));
        assert!(error.source().is_some());
    }

    #[test]
    fn invalid_eccentricity_is_a_typed_input_error() {
        assert_eq!(
            Sgp4Elements::new(
                Angle::new::<radian>(0.0),
                Angle::new::<radian>(0.0),
                Ratio::new::<ratio>(1.0),
                Angle::new::<radian>(0.0),
                Angle::new::<radian>(0.0),
                AngularVelocity::new::<radian_per_second>(1.0),
                0.0,
            ),
            Err(Sgp4ElementsError::Eccentricity)
        );
    }

    fn elements(epoch: Epoch, values: [f64; 7]) -> Sgp4Propagator {
        Sgp4Propagator::new(
            epoch,
            Sgp4Elements::new(
                Angle::new::<degree>(values[0]),
                Angle::new::<degree>(values[1]),
                Ratio::new::<ratio>(values[2]),
                Angle::new::<degree>(values[3]),
                Angle::new::<degree>(values[4]),
                AngularVelocity::new::<radian_per_second>(
                    values[5] * std::f64::consts::TAU / 86_400.0,
                ),
                values[6],
            )
            .expect("valid SGP4 elements"),
        )
        .expect("valid SGP4 propagator")
    }

    fn assert_prediction(
        propagator: Sgp4Propagator,
        minutes_since_epoch: f64,
        position_kilometres: [f64; 3],
        velocity_kilometres_per_second: [f64; 3],
    ) {
        let target = propagator.epoch() + Duration::from_seconds(minutes_since_epoch * 60.0);
        let prediction = propagator
            .propagate(propagator.initial_orbit(), target)
            .expect("published prediction");
        assert_eq!(prediction.epoch(), target);
        assert_eq!(prediction.as_ref().frame(), ReferenceFrame::TEME);
        assert_vector_close(
            prediction.as_ref().position().to_metres(),
            position_kilometres.map(|value| value * 1_000.0),
            POSITION_TOLERANCE_METRES,
        );
        assert_vector_close(
            prediction.as_ref().velocity().to_metres_per_second(),
            velocity_kilometres_per_second.map(|value| value * 1_000.0),
            VELOCITY_TOLERANCE_METRES_PER_SECOND,
        );
    }

    fn assert_vector_close(actual: [f64; 3], expected: [f64; 3], tolerance: f64) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!((actual - expected).abs() <= tolerance);
        }
    }
}
