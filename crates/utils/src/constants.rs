//! Typed physical and astronomical constants.

use units::uom::si::{length::meter, velocity::meter_per_second};
use units::{GravitationalConstant, GravitationalParameter, Length, Velocity};

/// WGS 84 geocentric gravitational parameter in `m^3/s^2`.
///
/// Source: [NGA World Geodetic System 1984](https://earth-info.nga.mil/?action=wgs84&dir=wgs84).
#[must_use]
pub fn wgs84_earth_gravitational_parameter() -> GravitationalParameter {
    GravitationalParameter::try_from(3.986_004_418e14)
        .expect("the conventional Earth gravitational parameter is positive and finite")
}

/// WGS 84 reference-ellipsoid semi-major axis.
///
/// Source: [NGA World Geodetic System 1984](https://earth-info.nga.mil/?action=wgs84&dir=wgs84).
#[must_use]
pub fn wgs84_semi_major_axis() -> Length {
    Length::new::<meter>(6_378_137.0)
}

/// Exact vacuum speed of light.
///
/// Source: [BIPM definition of the metre](https://www.bipm.org/en/si-base-units/metre).
#[must_use]
pub fn speed_of_light() -> Velocity {
    Velocity::new::<meter_per_second>(299_792_458.0)
}

/// Exact IAU 2012 astronomical unit.
///
/// Source: [IAU 2012 Resolution B2](https://www.iau.org/static/resolutions/IAU2012_English.pdf).
#[must_use]
pub fn astronomical_unit() -> Length {
    Length::new::<meter>(149_597_870_700.0)
}

/// CODATA 2018 Newtonian constant of gravitation in `m^3/(kg*s^2)`.
///
/// Source: [NIST 2018 CODATA values](https://physics.nist.gov/cuu/Constants/archive2018.html).
#[must_use]
pub fn gravitational_constant() -> GravitationalConstant {
    GravitationalConstant::try_from(6.674_30e-11)
        .expect("the CODATA gravitational constant is positive and finite")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_constants_have_typed_dimensions() {
        assert_eq!(
            speed_of_light(),
            Velocity::new::<meter_per_second>(299_792_458.0)
        );
        assert_eq!(astronomical_unit(), Length::new::<meter>(149_597_870_700.0));
    }
}
