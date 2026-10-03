use std::f64::consts::TAU;

use dynamics::sgp4::{Sgp4Elements, Sgp4ElementsError, Sgp4Error, Sgp4Propagator};
use hifitime::Epoch;
use thiserror::Error;
use units::uom::si::{angle::radian, angular_velocity::radian_per_second, ratio::ratio};
use units::{Angle, AngularVelocity, Ratio};

use crate::{days_in_year, TwoLineElement, SCALE_8};

/// Failure converting a validated TLE into an SGP4 propagator.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Sgp4ConversionError {
    /// TLE selects a legacy ephemeris type rather than distributed-data SGP4.
    #[error("TLE ephemeris type {found} is unsupported; expected type 0")]
    UnsupportedEphemerisType {
        /// The unsupported ephemeris type.
        found: u8,
    },
    /// TLE values did not form valid mean elements.
    #[error("TLE fields do not form valid SGP4 mean elements")]
    Elements(#[from] Sgp4ElementsError),
    /// The configured SGP4 model rejected the record.
    #[error("could not initialize SGP4 from the TLE")]
    Model(#[from] Sgp4Error),
}

impl TryFrom<&TwoLineElement> for Sgp4Propagator {
    type Error = Sgp4ConversionError;

    fn try_from(tle: &TwoLineElement) -> Result<Self, Self::Error> {
        if tle.ephemeris_type() != 0 {
            return Err(Sgp4ConversionError::UnsupportedEphemerisType {
                found: tle.ephemeris_type(),
            });
        }
        let elements = Sgp4Elements::new(
            Angle::new::<radian>(tle.inclination_deg().to_radians()),
            Angle::new::<radian>(tle.right_ascension_of_ascending_node_deg().to_radians()),
            Ratio::new::<ratio>(tle.eccentricity()),
            Angle::new::<radian>(tle.argument_of_perigee_deg().to_radians()),
            Angle::new::<radian>(tle.mean_anomaly_deg().to_radians()),
            AngularVelocity::new::<radian_per_second>(
                tle.mean_motion_rev_per_day() * TAU / 86_400.0,
            ),
            tle.b_star_inverse_earth_radii(),
        )?;
        Ok(Sgp4Propagator::new(tle_epoch(tle), elements)?)
    }
}

fn tle_epoch(tle: &TwoLineElement) -> Epoch {
    let year = tle.epoch_year();
    let scaled = tle.epoch_day_scaled;
    let ordinal = (scaled / SCALE_8) as u16;
    let day_fraction = scaled % SCALE_8;
    let nanoseconds = day_fraction * 864_000;
    let seconds = nanoseconds / 1_000_000_000;
    let subsecond = (nanoseconds % 1_000_000_000) as u32;
    let (month, day) = month_day(year, ordinal);
    Epoch::from_gregorian_utc(
        i32::from(year),
        month,
        day,
        (seconds / 3_600) as u8,
        ((seconds % 3_600) / 60) as u8,
        (seconds % 60) as u8,
        subsecond,
    )
}

fn month_day(year: u16, ordinal: u16) -> (u8, u8) {
    let february = if days_in_year(year) == 366 { 29 } else { 28 };
    let lengths = [31_u16, february, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut remaining = ordinal;
    for (index, length) in lengths.into_iter().enumerate() {
        if remaining <= length {
            return ((index + 1) as u8, remaining as u8);
        }
        remaining -= length;
    }
    (12, 31)
}
