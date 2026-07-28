use thiserror::Error;
use units::uom::si::{angle::radian, length::meter, ratio::ratio};
use units::{Angle, Length, Position, Ratio};

use crate::Body;

const HALF_TURN_RADIANS: f64 = std::f64::consts::PI;
const QUARTER_TURN_RADIANS: f64 = std::f64::consts::FRAC_PI_2;

/// Oblate reference ellipsoid attached to one celestial-body identity.
///
/// The semi-major axis is the equatorial radius. Inverse flattening is
/// `a / (a - b)`, where `b` is the semi-minor polar axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceEllipsoid {
    body: Body,
    semi_major_axis: Length,
    inverse_flattening: Ratio,
}

impl ReferenceEllipsoid {
    /// Constructs an oblate ellipsoid from its body, semi-major axis, and
    /// inverse flattening.
    pub fn new(
        body: Body,
        semi_major_axis: Length,
        inverse_flattening: Ratio,
    ) -> Result<Self, ReferenceEllipsoidError> {
        let axis_m = semi_major_axis.get::<meter>();
        if !axis_m.is_finite() || axis_m <= 0.0 {
            return Err(ReferenceEllipsoidError::InvalidSemiMajorAxis);
        }
        let inverse_flattening_value = inverse_flattening.get::<ratio>();
        if !inverse_flattening_value.is_finite() || inverse_flattening_value <= 1.0 {
            return Err(ReferenceEllipsoidError::InvalidInverseFlattening);
        }
        Ok(Self {
            body,
            semi_major_axis,
            inverse_flattening,
        })
    }

    /// World Geodetic System 1984 reference ellipsoid.
    ///
    /// NGA defines `a = 6_378_137.0 m` and `1/f = 298.257_223_563`.
    #[must_use]
    pub fn wgs84() -> Self {
        Self {
            body: Body::EARTH,
            semi_major_axis: Length::new::<meter>(6_378_137.0),
            inverse_flattening: Ratio::new::<ratio>(298.257_223_563),
        }
    }

    /// Body whose conventional body-fixed axes define this ellipsoid.
    #[must_use]
    pub const fn body(self) -> Body {
        self.body
    }

    /// Equatorial semi-major axis.
    #[must_use]
    pub const fn semi_major_axis(self) -> Length {
        self.semi_major_axis
    }

    /// Inverse flattening `1/f`.
    #[must_use]
    pub const fn inverse_flattening(self) -> Ratio {
        self.inverse_flattening
    }

    /// Polar semi-minor axis.
    #[must_use]
    pub fn semi_minor_axis(self) -> Length {
        Length::new::<meter>(self.semi_minor_axis_m())
    }

    /// First eccentricity squared, `e² = 2f - f²`.
    #[must_use]
    pub fn first_eccentricity_squared(self) -> Ratio {
        Ratio::new::<ratio>(self.eccentricity_squared())
    }

    /// Converts east-positive geodetic longitude, ellipsoidal latitude, and
    /// ellipsoidal height to body-centered Cartesian coordinates.
    ///
    /// The Cartesian axes are conventional body-fixed geocentric axes: `+Z`
    /// is the north polar axis, `+X` intersects zero longitude at the equator,
    /// and `+Y` intersects 90° east longitude at the equator.
    #[must_use]
    pub fn to_geocentric(self, position: GeodeticPosition) -> Position {
        let longitude = position.longitude.get::<radian>();
        let latitude = position.latitude.get::<radian>();
        let height = position.ellipsoidal_height.get::<meter>();
        let (sin_latitude, cos_latitude) = latitude.sin_cos();
        let (sin_longitude, cos_longitude) = longitude.sin_cos();
        let eccentricity_squared = self.eccentricity_squared();
        let prime_vertical_radius =
            self.semi_major_axis_m() / (1.0 - eccentricity_squared * sin_latitude.powi(2)).sqrt();

        Position::from_metres(
            (prime_vertical_radius + height) * cos_latitude * cos_longitude,
            (prime_vertical_radius + height) * cos_latitude * sin_longitude,
            ((1.0 - eccentricity_squared) * prime_vertical_radius + height) * sin_latitude,
        )
    }

    /// Converts conventional body-centered Cartesian coordinates to geodetic
    /// longitude, latitude, and ellipsoidal height.
    ///
    /// This implements EPSG method 9602. The body center is undefined for all
    /// geodetic coordinates. On the exact polar axis, latitude and height are
    /// defined but longitude is not, so the conversion returns
    /// [`GeodeticConversionError::IndeterminateLongitudeAtPole`] rather than
    /// silently choosing a meridian.
    pub fn to_geodetic(
        self,
        position: Position,
    ) -> Result<GeodeticPosition, GeodeticConversionError> {
        if !position.is_finite() {
            return Err(GeodeticConversionError::NonFinitePosition);
        }
        let [x, y, z] = position.to_metres();
        let distance_from_axis = x.hypot(y);
        if distance_from_axis == 0.0 {
            return if z == 0.0 {
                Err(GeodeticConversionError::BodyCenter)
            } else {
                Err(GeodeticConversionError::IndeterminateLongitudeAtPole)
            };
        }

        let a = self.semi_major_axis_m();
        let b = self.semi_minor_axis_m();
        let e2 = self.eccentricity_squared();
        let second_eccentricity_squared = (a * a - b * b) / (b * b);
        let auxiliary = (z * a).atan2(distance_from_axis * b);
        let (sin_auxiliary, cos_auxiliary) = auxiliary.sin_cos();
        let latitude = (z + second_eccentricity_squared * b * sin_auxiliary.powi(3))
            .atan2(distance_from_axis - e2 * a * cos_auxiliary.powi(3));
        let longitude = y.atan2(x);
        let sin_latitude = latitude.sin();
        let prime_vertical_radius = a / (1.0 - e2 * sin_latitude.powi(2)).sqrt();
        let height = distance_from_axis / latitude.cos() - prime_vertical_radius;

        GeodeticPosition::new(
            Angle::new::<radian>(longitude),
            Angle::new::<radian>(latitude),
            Length::new::<meter>(height),
        )
    }

    fn semi_major_axis_m(self) -> f64 {
        self.semi_major_axis.get::<meter>()
    }

    fn flattening(self) -> f64 {
        self.inverse_flattening.get::<ratio>().recip()
    }

    fn semi_minor_axis_m(self) -> f64 {
        self.semi_major_axis_m() * (1.0 - self.flattening())
    }

    fn eccentricity_squared(self) -> f64 {
        let flattening = self.flattening();
        flattening * (2.0 - flattening)
    }
}

/// East-positive longitude, ellipsoidal geodetic latitude, and ellipsoidal
/// height above a reference ellipsoid.
///
/// Longitude is in the closed interval `[-π, π]`; latitude is in
/// `[-π/2, π/2]`. Longitude remains explicit at a pole because it selects the
/// east/north directions of a local topocentric frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeodeticPosition {
    longitude: Angle,
    latitude: Angle,
    ellipsoidal_height: Length,
}

impl GeodeticPosition {
    /// Constructs validated geodetic coordinates.
    pub fn new(
        longitude: Angle,
        latitude: Angle,
        ellipsoidal_height: Length,
    ) -> Result<Self, GeodeticConversionError> {
        let longitude_rad = longitude.get::<radian>();
        if !longitude_rad.is_finite()
            || !(-HALF_TURN_RADIANS..=HALF_TURN_RADIANS).contains(&longitude_rad)
        {
            return Err(GeodeticConversionError::InvalidLongitude);
        }
        let latitude_rad = latitude.get::<radian>();
        if !latitude_rad.is_finite()
            || !(-QUARTER_TURN_RADIANS..=QUARTER_TURN_RADIANS).contains(&latitude_rad)
        {
            return Err(GeodeticConversionError::InvalidLatitude);
        }
        if !ellipsoidal_height.get::<meter>().is_finite() {
            return Err(GeodeticConversionError::NonFiniteHeight);
        }
        Ok(Self {
            longitude,
            latitude,
            ellipsoidal_height,
        })
    }

    /// East-positive longitude from the `+X` axis toward `+Y`.
    #[must_use]
    pub const fn longitude(self) -> Angle {
        self.longitude
    }

    /// Ellipsoidal latitude measured from the equatorial plane toward `+Z`.
    #[must_use]
    pub const fn latitude(self) -> Angle {
        self.latitude
    }

    /// Height along the ellipsoid normal; negative values are below it.
    #[must_use]
    pub const fn ellipsoidal_height(self) -> Length {
        self.ellipsoidal_height
    }
}

/// Invalid reference-ellipsoid definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ReferenceEllipsoidError {
    /// The semi-major axis was non-finite or non-positive.
    #[error("reference-ellipsoid semi-major axis must be finite and positive")]
    InvalidSemiMajorAxis,
    /// Inverse flattening was non-finite or not greater than one.
    #[error("reference-ellipsoid inverse flattening must be finite and greater than one")]
    InvalidInverseFlattening,
}

/// Failure while constructing or converting geodetic coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum GeodeticConversionError {
    /// Longitude was non-finite or outside `[-π, π]`.
    #[error("east-positive geodetic longitude must be finite and within [-pi, pi]")]
    InvalidLongitude,
    /// Latitude was non-finite or outside `[-π/2, π/2]`.
    #[error("geodetic latitude must be finite and within [-pi/2, pi/2]")]
    InvalidLatitude,
    /// Ellipsoidal height was non-finite.
    #[error("ellipsoidal height must be finite")]
    NonFiniteHeight,
    /// A Cartesian input component was non-finite.
    #[error("geocentric position must be finite")]
    NonFinitePosition,
    /// The body center has no unique geodetic latitude, longitude, or height.
    #[error("the body center has no unique geodetic coordinates")]
    BodyCenter,
    /// Longitude is undefined on the exact polar axis.
    #[error("geodetic longitude is indeterminate on the exact polar axis")]
    IndeterminateLongitudeAtPole,
}

#[cfg(test)]
mod tests {
    use super::*;
    use units::uom::si::angle::degree;

    fn epsg_position() -> GeodeticPosition {
        GeodeticPosition::new(
            Angle::new::<degree>(2.0 + 7.0 / 60.0 + 46.38 / 3_600.0),
            Angle::new::<degree>(53.0 + 48.0 / 60.0 + 33.82 / 3_600.0),
            Length::new::<meter>(73.0),
        )
        .expect("EPSG coordinates are valid")
    }

    #[test]
    fn wgs84_matches_nga_parameters() {
        let ellipsoid = ReferenceEllipsoid::wgs84();
        assert_eq!(ellipsoid.body(), Body::EARTH);
        assert_eq!(ellipsoid.semi_major_axis().get::<meter>(), 6_378_137.0);
        assert_eq!(
            ellipsoid.inverse_flattening().get::<ratio>(),
            298.257_223_563
        );
    }

    #[test]
    fn invalid_ellipsoids_and_geodetic_coordinates_are_rejected() {
        assert_eq!(
            ReferenceEllipsoid::new(
                Body::EARTH,
                Length::new::<meter>(0.0),
                Ratio::new::<ratio>(298.0)
            ),
            Err(ReferenceEllipsoidError::InvalidSemiMajorAxis)
        );
        assert_eq!(
            ReferenceEllipsoid::new(
                Body::EARTH,
                Length::new::<meter>(1.0),
                Ratio::new::<ratio>(1.0)
            ),
            Err(ReferenceEllipsoidError::InvalidInverseFlattening)
        );
        assert_eq!(
            GeodeticPosition::new(
                Angle::new::<radian>(f64::INFINITY),
                Angle::new::<radian>(0.0),
                Length::new::<meter>(0.0)
            ),
            Err(GeodeticConversionError::InvalidLongitude)
        );
        assert_eq!(
            GeodeticPosition::new(
                Angle::new::<radian>(0.0),
                Angle::new::<radian>(std::f64::consts::PI),
                Length::new::<meter>(0.0)
            ),
            Err(GeodeticConversionError::InvalidLatitude)
        );
        assert_eq!(
            GeodeticPosition::new(
                Angle::new::<radian>(0.0),
                Angle::new::<radian>(0.0),
                Length::new::<meter>(f64::NAN)
            ),
            Err(GeodeticConversionError::NonFiniteHeight)
        );
    }

    #[test]
    fn epsg_geographic_geocentric_vector_matches_to_centimetres() {
        let ellipsoid = ReferenceEllipsoid::wgs84();
        let [x, y, z] = ellipsoid.to_geocentric(epsg_position()).to_metres();

        assert!((x - 3_771_793.97).abs() <= 0.01);
        assert!((y - 140_253.34).abs() <= 0.01);
        assert!((z - 5_124_304.35).abs() <= 0.01);
    }

    #[test]
    fn epsg_vector_round_trips_with_sub_millimetre_height_error() {
        let ellipsoid = ReferenceEllipsoid::wgs84();
        let expected = epsg_position();
        let actual = ellipsoid
            .to_geodetic(ellipsoid.to_geocentric(expected))
            .expect("non-polar EPSG position");

        assert!(
            (actual.longitude().get::<radian>() - expected.longitude().get::<radian>()).abs()
                < 1.0e-12
        );
        assert!(
            (actual.latitude().get::<radian>() - expected.latitude().get::<radian>()).abs()
                < 1.0e-12
        );
        assert!((actual.ellipsoidal_height().get::<meter>() - 73.0).abs() < 1.0e-3);
    }

    #[test]
    fn geodetic_round_trips_cover_hemispheres_and_altitudes() {
        let ellipsoid = ReferenceEllipsoid::wgs84();
        for (longitude_deg, latitude_deg, height_m) in [
            (-170.0, -80.0, -500.0),
            (-90.0, 0.0, 1_000_000.0),
            (0.0, 89.0, 100.0),
            (45.0, -25.0, 0.0),
            (179.0, 60.0, 40_000.0),
        ] {
            let expected = GeodeticPosition::new(
                Angle::new::<degree>(longitude_deg),
                Angle::new::<degree>(latitude_deg),
                Length::new::<meter>(height_m),
            )
            .expect("finite geodetic position");
            let actual = ellipsoid
                .to_geodetic(ellipsoid.to_geocentric(expected))
                .expect("non-singular geocentric position");

            assert!(
                (actual.longitude().get::<radian>() - expected.longitude().get::<radian>()).abs()
                    < 1.0e-12
            );
            assert!(
                (actual.latitude().get::<radian>() - expected.latitude().get::<radian>()).abs()
                    < 1.0e-11
            );
            assert!((actual.ellipsoidal_height().get::<meter>() - height_m).abs() < 1.0e-3);
        }
    }

    #[test]
    fn pole_and_center_inverse_singularities_are_explicit() {
        let ellipsoid = ReferenceEllipsoid::wgs84();
        assert_eq!(
            ellipsoid.to_geodetic(Position::from_metres(0.0, 0.0, 0.0)),
            Err(GeodeticConversionError::BodyCenter)
        );
        assert_eq!(
            ellipsoid.to_geodetic(Position::from_metres(f64::NAN, 0.0, 0.0)),
            Err(GeodeticConversionError::NonFinitePosition)
        );
        assert_eq!(
            ellipsoid.to_geodetic(Position::from_metres(
                0.0,
                0.0,
                ellipsoid.semi_minor_axis().get::<meter>()
            )),
            Err(GeodeticConversionError::IndeterminateLongitudeAtPole)
        );
    }

    #[test]
    fn forward_conversion_accepts_an_explicit_polar_meridian() {
        let ellipsoid = ReferenceEllipsoid::wgs84();
        let north_pole = GeodeticPosition::new(
            Angle::new::<degree>(45.0),
            Angle::new::<degree>(90.0),
            Length::new::<meter>(10.0),
        )
        .expect("explicit polar meridian");
        let [x, y, z] = ellipsoid.to_geocentric(north_pole).to_metres();

        assert!(x.abs() < 1.0e-9);
        assert!(y.abs() < 1.0e-9);
        assert!((z - (ellipsoid.semi_minor_axis().get::<meter>() + 10.0)).abs() < 1.0e-9);
    }
}
