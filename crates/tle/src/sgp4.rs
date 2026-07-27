use std::f64::consts::TAU;

use dynamics::{Sgp4Elements, Sgp4ElementsError};
use hifitime::Epoch;
use orskit_core::Orbit;
use thiserror::Error;
use units::uom::si::{angle::radian, angular_velocity::radian_per_second, ratio::ratio};
use units::{Angle, AngularVelocity, Ratio};

use crate::TwoLineElement;

const NANOSECONDS_PER_EPOCH_FRACTION: u64 = 864_000;

/// Failure while converting a parsed TLE into model-specific SGP4 elements.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Sgp4ConversionError {
    /// The record selects an explicit legacy propagator rather than the
    /// distributed-data default SGP4/SDP4 model.
    #[error("TLE ephemeris type {found} is not supported; expected distributed-data type 0")]
    UnsupportedEphemerisType {
        /// Unsupported fixed-column ephemeris type.
        found: u8,
    },
    /// Parsed fields do not form valid model elements.
    #[error("TLE fields do not form valid SGP4 elements")]
    Elements(#[source] Sgp4ElementsError),
}

impl TwoLineElement {
    /// Converts this format record into an epoch-qualified SGP4 domain state.
    ///
    /// Parsing and fixed-column policy remain in this crate. Propagation and
    /// its fixed force model live behind `dynamics`' `sgp4` feature.
    pub fn to_sgp4_orbit(&self) -> Result<Orbit<Sgp4Elements>, Sgp4ConversionError> {
        if self.ephemeris_type() != 0 {
            return Err(Sgp4ConversionError::UnsupportedEphemerisType {
                found: self.ephemeris_type(),
            });
        }
        let epoch = tle_epoch(self)?;
        let elements = Sgp4Elements::new(
            Angle::new::<radian>(self.inclination_deg().to_radians()),
            Angle::new::<radian>(self.right_ascension_of_ascending_node_deg().to_radians()),
            Ratio::new::<ratio>(self.eccentricity()),
            Angle::new::<radian>(self.argument_of_perigee_deg().to_radians()),
            Angle::new::<radian>(self.mean_anomaly_deg().to_radians()),
            AngularVelocity::new::<radian_per_second>(
                self.mean_motion_rev_per_day() * TAU / 86_400.0,
            ),
            self.b_star_inverse_earth_radii(),
        )
        .map_err(Sgp4ConversionError::Elements)?;
        Ok(Orbit::new(epoch, elements))
    }
}

fn tle_epoch(tle: &TwoLineElement) -> Result<Epoch, Sgp4ConversionError> {
    let scaled = tle.epoch_day_scaled;
    let whole_day = (scaled / super::SCALE_8) as u16;
    let fraction = scaled % super::SCALE_8;
    let nanoseconds = fraction * NANOSECONDS_PER_EPOCH_FRACTION;
    let seconds = nanoseconds / 1_000_000_000;
    let subsecond = (nanoseconds % 1_000_000_000) as u32;
    let (month, day) = month_day(tle.epoch_year(), whole_day);
    Ok(Epoch::from_gregorian_utc(
        i32::from(tle.epoch_year()),
        month,
        day,
        (seconds / 3_600) as u8,
        ((seconds % 3_600) / 60) as u8,
        (seconds % 60) as u8,
        subsecond,
    ))
}

fn month_day(year: u16, ordinal: u16) -> (u8, u8) {
    let february = if super::days_in_year(year) == 366 {
        29
    } else {
        28
    };
    let month_lengths = [31_u16, february, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut remaining = ordinal;
    for (index, length) in month_lengths.into_iter().enumerate() {
        if remaining <= length {
            return ((index + 1) as u8, remaining as u8);
        }
        remaining -= length;
    }
    unreachable!("the strict parser validates the ordinal day")
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE_1: &str = "1 00005U 58002B   00179.78495062  .00000023  00000-0  28098-4 0  4753";
    const LINE_2: &str = "2 00005  34.2682 348.7242 1859667 331.7664  19.3264 10.82419157413667";

    #[test]
    fn conversion_preserves_epoch() {
        let tle = TwoLineElement::parse(LINE_1, LINE_2).expect("published TLE");
        let orbit = tle.to_sgp4_orbit().expect("valid model elements");
        assert_eq!(
            orbit.epoch(),
            Epoch::from_gregorian_utc(2000, 6, 27, 18, 50, 19, 733_568_000)
        );
    }

    #[test]
    fn conversion_rejects_legacy_model_selectors() {
        for ephemeris_type in 1..=9 {
            let line_one = replace_field_and_checksum(LINE_1, 62..63, &ephemeris_type.to_string());
            let tle = TwoLineElement::parse(&line_one, LINE_2).expect("valid format");
            assert!(matches!(
                tle.to_sgp4_orbit(),
                Err(Sgp4ConversionError::UnsupportedEphemerisType { found })
                    if found == ephemeris_type
            ));
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
