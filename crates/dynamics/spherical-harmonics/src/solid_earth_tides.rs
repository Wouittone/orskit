use std::sync::Arc;

use bodies::Body;
use frames::{ReferenceDataDescriptor, ReferenceFrame};
use hifitime::Epoch;
use orbits::cartesian::{BodyEphemerisError, BodyEphemerisProvider};
use thiserror::Error;
use units::GravitationalParameter;

use super::{
    invalid_provenance_field, CoefficientDeltaCoverage, CoefficientDeltaProviderError,
    CoefficientNormalization, ConstructionError, HarmonicCoefficient,
    HarmonicCoefficientDeltaEvaluator, HarmonicCoefficientDeltaMetadata,
    HarmonicCoefficientDeltaProvider, ProvenanceProvider, SphericalHarmonicField, TideSystem,
};

const MAXIMUM_DEGREE: u32 = 4;
const MAXIMUM_ORDER: u32 = 4;
const PERMANENT_C20: f64 = 4.4228e-8 * -0.31460 * 0.29525;

const DEGREE_2_LOVE_NUMBERS: [f64; 3] = [0.29525, 0.29470, 0.29801];
const DEGREE_3_LOVE_NUMBERS: [f64; 4] = [0.093, 0.093, 0.093, 0.094];
const DEGREE_4_LOVE_NUMBERS: [f64; 3] = [-0.00087, -0.00079, -0.00057];

const ZERO_COEFFICIENT: HarmonicCoefficient = HarmonicCoefficient {
    cosine: 0.0,
    sine: 0.0,
};

/// A caller-sourced gravitational parameter for an IERS solid-Earth tide raiser.
///
/// ```rust
/// use bodies::Body;
/// use dynamics_spherical_harmonics::TideRaiser;
/// use frames::ReferenceDataDescriptor;
/// use units::GravitationalParameter;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let moon = TideRaiser::new(
///     Body::MOON,
///     GravitationalParameter::try_from(4.904_869_5e12)?,
///     ReferenceDataDescriptor {
///         authority: "caller-selected constants source".into(),
///         product: "Moon gravitational parameter".into(),
///         revision: "mission-release-1".into(),
///         checksum: None,
///     },
/// )?;
/// assert_eq!(moon.body(), Body::MOON);
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct TideRaiser {
    body: Body,
    gravitational_parameter: GravitationalParameter,
    source: ReferenceDataDescriptor,
}

impl TideRaiser {
    /// Constructs a sourced Moon or Sun tide-raiser parameter.
    ///
    /// The parameter must be finite and positive. Its provenance identifies
    /// the product and revision from which it was selected; no constants are
    /// bundled by the solid-Earth tide model.
    pub fn new(
        body: Body,
        gravitational_parameter: GravitationalParameter,
        source: ReferenceDataDescriptor,
    ) -> Result<Self, ConstructionError> {
        if body != Body::MOON && body != Body::SUN {
            return Err(ConstructionError::InvalidTideRaiserIdentity);
        }
        let value = gravitational_parameter.as_cubic_metres_per_second_squared();
        if !value.is_finite() || value <= 0.0 {
            return Err(ConstructionError::InvalidTideRaiserGravitationalParameter);
        }
        if let Some(field) = invalid_provenance_field(&source) {
            return Err(ConstructionError::InvalidProvenance {
                provider: ProvenanceProvider::TideRaiser,
                record: 0,
                field,
            });
        }
        Ok(Self {
            body,
            gravitational_parameter,
            source,
        })
    }

    /// Returns the tide-raising body's identity.
    #[must_use]
    pub const fn body(&self) -> Body {
        self.body
    }

    /// Returns the selected typed gravitational parameter.
    #[must_use]
    pub const fn gravitational_parameter(&self) -> GravitationalParameter {
        self.gravitational_parameter
    }

    /// Returns the data record for this gravitational parameter.
    #[must_use]
    pub const fn reference_data(&self) -> &ReferenceDataDescriptor {
        &self.source
    }
}

/// Failure while resolving or evaluating a solid-Earth tide epoch.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SolidEarthTideEvaluationError {
    /// The caller-selected ephemeris failed for an explicitly requested body.
    #[error("ephemeris failed for tide-raising body {body} at {epoch}")]
    Ephemeris {
        /// Body requested from the ephemeris.
        body: Body,
        /// Epoch requested from the ephemeris.
        epoch: Epoch,
        /// Original caller-provider error.
        #[source]
        source: Box<BodyEphemerisError>,
    },
    /// The ephemeris response did not match the requested body, epoch, or frame.
    #[error("ephemeris returned a mismatched body, epoch, or terrestrial frame for {body}")]
    InconsistentEphemerisState {
        /// Body requested from the ephemeris.
        body: Body,
    },
    /// The tide-raiser position is not a finite exterior geocentric position.
    #[error("tide-raising body {body} has an invalid geocentric distance")]
    InvalidTideRaiserDistance {
        /// Body whose position is invalid.
        body: Body,
    },
    /// The computed correction is not finite.
    #[error("solid-Earth tide coefficient delta C/S({degree},{order}) is not finite")]
    NonFiniteCoefficient {
        /// Invalid spherical-harmonic degree.
        degree: u32,
        /// Invalid spherical-harmonic order.
        order: u32,
    },
}

#[derive(Debug, Clone, Copy)]
struct TideRaiserPosition {
    gravitational_parameter: f64,
    position_m: [f64; 3],
    distance_m: f64,
}

#[derive(Debug)]
struct SolidEarthTideEpochEvaluator {
    covered: bool,
    coefficients: [[HarmonicCoefficient; 5]; 5],
}

impl HarmonicCoefficientDeltaEvaluator for SolidEarthTideEpochEvaluator {
    fn coefficient_delta(
        &self,
        degree: u32,
        order: u32,
    ) -> Result<Option<HarmonicCoefficient>, CoefficientDeltaProviderError> {
        if !self.covered || degree > MAXIMUM_DEGREE || order > degree || order > MAXIMUM_ORDER {
            return Ok(None);
        }
        let degree_index = usize::try_from(degree).expect("degree is at most four");
        let order_index = usize::try_from(order).expect("order is at most four");
        Ok(Some(self.coefficients[degree_index][order_index]))
    }
}

#[derive(Debug)]
pub(super) struct Iers2010SolidEarthTideProvider {
    metadata: HarmonicCoefficientDeltaMetadata,
    earth_gravitational_parameter: f64,
    earth_equatorial_radius_m: f64,
    ephemeris: Arc<dyn BodyEphemerisProvider>,
    reference_data: Vec<ReferenceDataDescriptor>,
    moon: TideRaiser,
    sun: TideRaiser,
}

impl Iers2010SolidEarthTideProvider {
    pub(super) fn new(
        field: SphericalHarmonicField,
        tide_system: TideSystem,
        coverage: CoefficientDeltaCoverage,
        ephemeris: Arc<dyn BodyEphemerisProvider>,
        mut ephemeris_reference_data: Vec<ReferenceDataDescriptor>,
        moon: TideRaiser,
        sun: TideRaiser,
    ) -> Result<Self, ConstructionError> {
        if field.origin != ReferenceFrame::ITRF2020.origin() {
            return Err(ConstructionError::SolidEarthTideRequiresEarthOrigin);
        }
        if field.body_fixed_frame != ReferenceFrame::ITRF2020 {
            return Err(ConstructionError::SolidEarthTideRequiresItrf2020);
        }
        if !matches!(tide_system, TideSystem::TideFree | TideSystem::ZeroTide) {
            return Err(ConstructionError::UnsupportedSolidEarthTideSystem);
        }
        if coverage.end < coverage.start {
            return Err(ConstructionError::InvalidDeltaCoverage);
        }
        if ephemeris_reference_data.is_empty() {
            return Err(ConstructionError::MissingSolidEarthTideEphemerisProvenance);
        }
        for (record, source) in ephemeris_reference_data.iter().enumerate() {
            if let Some(field) = invalid_provenance_field(source) {
                return Err(ConstructionError::InvalidProvenance {
                    provider: ProvenanceProvider::TideEphemeris,
                    record,
                    field,
                });
            }
        }
        if moon.body != Body::MOON || sun.body != Body::SUN {
            return Err(ConstructionError::InvalidTideRaiserIdentity);
        }

        let earth_gravitational_parameter = field
            .gravitational_parameter
            .as_cubic_metres_per_second_squared();
        let earth_equatorial_radius_m = field
            .reference_radius
            .get::<units::uom::si::length::meter>();
        if !earth_gravitational_parameter.is_finite() || earth_gravitational_parameter <= 0.0 {
            return Err(ConstructionError::InvalidGravitationalParameter);
        }
        if !earth_equatorial_radius_m.is_finite() || earth_equatorial_radius_m <= 0.0 {
            return Err(ConstructionError::InvalidReferenceRadius);
        }

        let model_source = ReferenceDataDescriptor {
            authority: "International Earth Rotation and Reference Systems Service".into(),
            product: "IERS Conventions (2010), Chapter 6, §6.2, Eqs. 6.6–6.7 and 6.13–6.14".into(),
            revision: "Technical Note 36; Chapter 6 revised 2018-02-01".into(),
            checksum: None,
        };
        let mut reference_data = vec![clone_reference_data(&model_source)];
        reference_data.append(&mut ephemeris_reference_data);
        reference_data.push(clone_reference_data(&moon.source));
        reference_data.push(clone_reference_data(&sun.source));
        Ok(Self {
            metadata: HarmonicCoefficientDeltaMetadata {
                source: model_source,
                normalization: CoefficientNormalization::FullyNormalized4Pi,
                tide_system,
                coefficient_frame: field.body_fixed_frame,
                maximum_degree: MAXIMUM_DEGREE,
                maximum_order: MAXIMUM_ORDER,
                coverage,
            },
            earth_gravitational_parameter,
            earth_equatorial_radius_m,
            ephemeris,
            reference_data,
            moon,
            sun,
        })
    }

    fn tide_raiser_position(
        &self,
        body: Body,
        gravitational_parameter: f64,
        epoch: Epoch,
    ) -> Result<TideRaiserPosition, SolidEarthTideEvaluationError> {
        let frame = self.metadata.coefficient_frame;
        let state = self
            .ephemeris
            .state_at(body, epoch, frame)
            .map_err(|source| SolidEarthTideEvaluationError::Ephemeris {
                body,
                epoch,
                source: Box::new(source),
            })?;
        if state.body() != body || state.epoch() != epoch || state.kinematics().frame() != frame {
            return Err(SolidEarthTideEvaluationError::InconsistentEphemerisState { body });
        }
        let position_m = state.kinematics().position().to_metres();
        let distance_m = position_m[0].hypot(position_m[1]).hypot(position_m[2]);
        if !position_m.iter().all(|component| component.is_finite())
            || !distance_m.is_finite()
            || distance_m <= self.earth_equatorial_radius_m
        {
            return Err(SolidEarthTideEvaluationError::InvalidTideRaiserDistance { body });
        }
        Ok(TideRaiserPosition {
            gravitational_parameter,
            position_m,
            distance_m,
        })
    }

    fn evaluate_epoch(
        &self,
        epoch: Epoch,
    ) -> Result<SolidEarthTideEpochEvaluator, SolidEarthTideEvaluationError> {
        let request_epoch = epoch.to_time_scale(self.metadata.coverage.time_scale);
        let start = self
            .metadata
            .coverage
            .start
            .to_time_scale(self.metadata.coverage.time_scale);
        let end = self
            .metadata
            .coverage
            .end
            .to_time_scale(self.metadata.coverage.time_scale);
        if request_epoch < start || request_epoch > end {
            return Ok(SolidEarthTideEpochEvaluator {
                covered: false,
                coefficients: [[ZERO_COEFFICIENT; 5]; 5],
            });
        }

        let moon = self.tide_raiser_position(
            self.moon.body,
            self.moon
                .gravitational_parameter
                .as_cubic_metres_per_second_squared(),
            request_epoch,
        )?;
        let sun = self.tide_raiser_position(
            self.sun.body,
            self.sun
                .gravitational_parameter
                .as_cubic_metres_per_second_squared(),
            request_epoch,
        )?;
        let raisers = [moon, sun];
        let mut coefficients = [[ZERO_COEFFICIENT; 5]; 5];
        for order in 0..=2 {
            coefficients[2][order] = tide_coefficient(
                &raisers,
                self.earth_gravitational_parameter,
                self.earth_equatorial_radius_m,
                TideCoefficientSpec {
                    output_degree: 2,
                    order,
                    legendre_degree: 2,
                    radial_exponent: 3,
                    love_number: DEGREE_2_LOVE_NUMBERS[order],
                },
            )?;
            coefficients[4][order] = tide_coefficient(
                &raisers,
                self.earth_gravitational_parameter,
                self.earth_equatorial_radius_m,
                TideCoefficientSpec {
                    output_degree: 4,
                    order,
                    legendre_degree: 2,
                    radial_exponent: 3,
                    love_number: DEGREE_4_LOVE_NUMBERS[order],
                },
            )?;
        }
        for order in 0..=3 {
            coefficients[3][order] = tide_coefficient(
                &raisers,
                self.earth_gravitational_parameter,
                self.earth_equatorial_radius_m,
                TideCoefficientSpec {
                    output_degree: 3,
                    order,
                    legendre_degree: 3,
                    radial_exponent: 4,
                    love_number: DEGREE_3_LOVE_NUMBERS[order],
                },
            )?;
        }

        if self.metadata.tide_system == TideSystem::ZeroTide {
            coefficients[2][0].cosine -= PERMANENT_C20;
        }
        Ok(SolidEarthTideEpochEvaluator {
            covered: true,
            coefficients,
        })
    }
}

impl HarmonicCoefficientDeltaProvider for Iers2010SolidEarthTideProvider {
    fn metadata(&self) -> &HarmonicCoefficientDeltaMetadata {
        &self.metadata
    }

    fn coefficient_delta(
        &self,
        epoch: Epoch,
        degree: u32,
        order: u32,
    ) -> Option<HarmonicCoefficient> {
        self.evaluator_at(epoch)
            .ok()?
            .coefficient_delta(degree, order)
            .ok()?
    }

    fn evaluator_at(
        &self,
        epoch: Epoch,
    ) -> Result<Box<dyn HarmonicCoefficientDeltaEvaluator + '_>, CoefficientDeltaProviderError>
    {
        self.evaluate_epoch(epoch)
            .map(|evaluator| Box::new(evaluator) as Box<dyn HarmonicCoefficientDeltaEvaluator>)
            .map_err(|error| Box::new(error) as CoefficientDeltaProviderError)
    }

    fn reference_data(&self) -> &[ReferenceDataDescriptor] {
        &self.reference_data
    }
}

struct TideCoefficientSpec {
    output_degree: usize,
    order: usize,
    legendre_degree: usize,
    radial_exponent: i32,
    love_number: f64,
}

fn tide_coefficient(
    raisers: &[TideRaiserPosition; 2],
    earth_gravitational_parameter: f64,
    earth_equatorial_radius_m: f64,
    spec: TideCoefficientSpec,
) -> Result<HarmonicCoefficient, SolidEarthTideEvaluationError> {
    let mut cosine = 0.0;
    let mut sine = 0.0;
    for raiser in raisers {
        let [x, y, z] = raiser.position_m;
        let radial = x.hypot(y);
        let sin_latitude = z / raiser.distance_m;
        let cos_latitude = radial / raiser.distance_m;
        let legendre =
            fully_normalized_legendre(spec.legendre_degree, spec.order, sin_latitude, cos_latitude);
        let (cos_m_longitude, sin_m_longitude) = longitude_multiples(x, y, radial, spec.order);
        let radial_ratio = earth_equatorial_radius_m / raiser.distance_m;
        let factor = (3.0 * raiser.gravitational_parameter / earth_gravitational_parameter)
            * radial_ratio.powi(spec.radial_exponent)
            * legendre
            * (spec.love_number / (2 * spec.legendre_degree + 1) as f64);
        cosine = factor.mul_add(cos_m_longitude, cosine);
        sine = factor.mul_add(sin_m_longitude, sine);
    }
    if !cosine.is_finite() || !sine.is_finite() {
        return Err(SolidEarthTideEvaluationError::NonFiniteCoefficient {
            degree: u32::try_from(spec.output_degree).expect("degree is at most four"),
            order: u32::try_from(spec.order).expect("order is at most four"),
        });
    }
    Ok(HarmonicCoefficient { cosine, sine })
}

fn fully_normalized_legendre(
    degree: usize,
    order: usize,
    sin_latitude: f64,
    cos_latitude: f64,
) -> f64 {
    if order > degree {
        return 0.0;
    }
    let mut diagonal = 1.0;
    for factor in 1..=order {
        diagonal *= f64::from(u32::try_from(2 * factor - 1).expect("small degree")) * cos_latitude;
    }
    let unnormalized = if degree == order {
        diagonal
    } else {
        let mut previous_previous = diagonal;
        let mut previous = f64::from(u32::try_from(2 * order + 1).expect("small degree"))
            * sin_latitude
            * diagonal;
        if degree == order + 1 {
            previous
        } else {
            for current_degree in (order + 2)..=degree {
                let current =
                    (f64::from(u32::try_from(2 * current_degree - 1).expect("small degree"))
                        * sin_latitude
                        * previous
                        - f64::from(
                            u32::try_from(current_degree + order - 1).expect("small degree"),
                        ) * previous_previous)
                        / f64::from(
                            u32::try_from(current_degree - order)
                                .expect("positive degree difference"),
                        );
                previous_previous = previous;
                previous = current;
            }
            previous
        }
    };
    let factorial_ratio = factorial(degree - order) as f64 / factorial(degree + order) as f64;
    let order_factor = if order == 0 { 1.0 } else { 2.0 };
    (order_factor * (2 * degree + 1) as f64 * factorial_ratio).sqrt() * unnormalized
}

fn factorial(value: usize) -> usize {
    (1..=value).product()
}

fn longitude_multiples(x: f64, y: f64, horizontal_radius: f64, order: usize) -> (f64, f64) {
    if order == 0 || horizontal_radius == 0.0 {
        return (1.0, 0.0);
    }
    let cosine = x / horizontal_radius;
    let sine = y / horizontal_radius;
    let mut cosine_multiple: f64 = 1.0;
    let mut sine_multiple: f64 = 0.0;
    for _ in 0..order {
        let next_cosine = cosine_multiple.mul_add(cosine, -(sine_multiple * sine));
        sine_multiple = sine_multiple.mul_add(cosine, cosine_multiple * sine);
        cosine_multiple = next_cosine;
    }
    (cosine_multiple, sine_multiple)
}

fn clone_reference_data(source: &ReferenceDataDescriptor) -> ReferenceDataDescriptor {
    ReferenceDataDescriptor {
        authority: source.authority.clone(),
        product: source.product.clone(),
        revision: source.revision.clone(),
        checksum: source.checksum.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frames::{
        CustomFrameId, FrameKinematics, FrameMotion, FrameOrientation, FrameOrigin, ReferenceFrame,
    };
    use hifitime::TimeScale;
    use orbits::cartesian::BodyState;
    use units::uom::si::length::meter;
    use units::{Length, Position, VelocityVector};

    #[derive(Debug)]
    struct FixedEphemeris {
        moon: [f64; 3],
        sun: [f64; 3],
        requests: std::sync::atomic::AtomicUsize,
        fail: bool,
        wrong_epoch: bool,
        wrong_frame: bool,
    }

    impl BodyEphemerisProvider for FixedEphemeris {
        fn state_at(
            &self,
            body: Body,
            epoch: Epoch,
            frame: ReferenceFrame,
        ) -> Result<BodyState, BodyEphemerisError> {
            use std::sync::atomic::Ordering;

            self.requests.fetch_add(1, Ordering::Relaxed);
            if self.fail {
                return Err(BodyEphemerisError::OutsideCoverage {
                    body,
                    epoch: Box::new(epoch),
                });
            }
            let position = match body {
                Body::MOON => self.moon,
                Body::SUN => self.sun,
                _ => return Err(BodyEphemerisError::UnsupportedBody { body }),
            };
            let response_epoch = if self.wrong_epoch {
                epoch + hifitime::Duration::from_seconds(1.0)
            } else {
                epoch
            };
            let response_frame = if self.wrong_frame {
                ReferenceFrame::GCRF
            } else {
                frame
            };
            Ok(BodyState::new(
                body,
                response_epoch,
                FrameKinematics::new(
                    Position::from_metres(position[0], position[1], position[2]),
                    VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
                    response_frame,
                )
                .expect("finite position and velocity"),
            ))
        }
    }

    fn source(authority: &str, product: &str) -> ReferenceDataDescriptor {
        ReferenceDataDescriptor {
            authority: authority.into(),
            product: product.into(),
            revision: "test-1".into(),
            checksum: None,
        }
    }

    fn raiser(body: Body, mu: f64) -> TideRaiser {
        TideRaiser::new(
            body,
            GravitationalParameter::try_from(mu).expect("positive GM"),
            source("test source", "tide-raiser GM"),
        )
        .expect("valid tide raiser")
    }

    fn provider(
        tide_system: TideSystem,
        ephemeris: Arc<FixedEphemeris>,
    ) -> Iers2010SolidEarthTideProvider {
        let field = SphericalHarmonicField {
            gravitational_parameter: GravitationalParameter::try_from(3.986_004_418e14)
                .expect("positive Earth GM"),
            reference_radius: Length::new::<meter>(6_378_137.0),
            origin: FrameOrigin::Body(Body::EARTH),
            inertial_frame: frames::InertialFrame::GCRF,
            body_fixed_frame: ReferenceFrame::ITRF2020,
            time_scale: TimeScale::TAI,
            maximum_degree: 4,
            maximum_order: 4,
        };
        Iers2010SolidEarthTideProvider::new(
            field,
            tide_system,
            CoefficientDeltaCoverage {
                start: Epoch::from_tai_seconds(0.0),
                end: Epoch::from_tai_seconds(10_000.0),
                time_scale: TimeScale::TAI,
            },
            ephemeris,
            vec![source("test ephemeris", "fixed geocentric vectors")],
            raiser(Body::MOON, 4.904_869_5e12),
            raiser(Body::SUN, 1.327_124_4e20),
        )
        .expect("valid tide provider")
    }

    fn fixture() -> Arc<FixedEphemeris> {
        Arc::new(FixedEphemeris {
            moon: [384_400_000.0, 0.0, 0.0],
            sun: [149_597_870_700.0, 0.0, 0.0],
            requests: std::sync::atomic::AtomicUsize::new(0),
            fail: false,
            wrong_epoch: false,
            wrong_frame: false,
        })
    }

    #[test]
    fn equatorial_raisiers_match_the_closed_form_degree_two_zonal_equation() {
        let ephemeris = fixture();
        let provider = provider(TideSystem::TideFree, Arc::clone(&ephemeris));
        let epoch = Epoch::from_tai_seconds(123.0);
        let actual = provider
            .evaluator_at(epoch)
            .expect("resolved tide epoch")
            .coefficient_delta(2, 0)
            .expect("degree-two zonal coefficient")
            .expect("covered coefficient");

        let earth_mu = 3.986_004_418e14;
        let earth_radius = 6_378_137.0;
        let moon_term = (4.904_869_5e12 / earth_mu) * (earth_radius / 384_400_000.0_f64).powi(3);
        let sun_term = (1.327_124_4e20 / earth_mu) * (earth_radius / 149_597_870_700.0_f64).powi(3);
        let independently_evaluated =
            (0.29525 / 5.0) * 3.0 * (moon_term + sun_term) * (-5.0_f64.sqrt() / 2.0);
        assert!(
            (actual.cosine - independently_evaluated).abs() < 2.0e-23,
            "actual={}, independent closed form={}",
            actual.cosine,
            independently_evaluated
        );
        assert_eq!(actual.sine, 0.0);
        assert_eq!(
            ephemeris
                .requests
                .load(std::sync::atomic::Ordering::Relaxed),
            2
        );
    }

    #[test]
    fn normalized_tesseral_and_degree_three_terms_match_closed_forms() {
        let latitude_sine: f64 = 0.5;
        let latitude_cosine = (1.0 - latitude_sine * latitude_sine).sqrt();
        let longitude_cosine = std::f64::consts::FRAC_1_SQRT_2;
        let make_position = |radius: f64| {
            [
                radius * latitude_cosine * longitude_cosine,
                radius * latitude_cosine * longitude_cosine,
                radius * latitude_sine,
            ]
        };
        let moon_distance = 384_400_000.0;
        let sun_distance = 149_597_870_700.0;
        let ephemeris = Arc::new(FixedEphemeris {
            moon: make_position(moon_distance),
            sun: make_position(sun_distance),
            requests: std::sync::atomic::AtomicUsize::new(0),
            fail: false,
            wrong_epoch: false,
            wrong_frame: false,
        });
        let provider = provider(TideSystem::TideFree, ephemeris);
        let evaluator = provider
            .evaluator_at(Epoch::from_tai_seconds(321.0))
            .expect("resolved epoch");
        let earth_mu = 3.986_004_418e14;
        let earth_radius = 6_378_137.0;
        let weighted_degree_two = (4.904_869_5e12 / earth_mu)
            * (earth_radius / moon_distance).powi(3)
            + (1.327_124_4e20 / earth_mu) * (earth_radius / sun_distance).powi(3);
        let p22 = 15.0_f64.sqrt() / 2.0 * latitude_cosine.powi(2);
        let expected_s22 = (0.29801 / 5.0) * 3.0 * weighted_degree_two * p22;
        let s22 = evaluator
            .coefficient_delta(2, 2)
            .expect("degree-two tesseral")
            .expect("covered coefficient");
        assert!((s22.sine - expected_s22).abs() < 2.0e-22);
        assert!(s22.cosine.abs() < 2.0e-22);

        let s42 = evaluator
            .coefficient_delta(4, 2)
            .expect("degree-four response")
            .expect("covered coefficient");
        assert!(
            (s42.sine / s22.sine - DEGREE_4_LOVE_NUMBERS[2] / DEGREE_2_LOVE_NUMBERS[2]).abs()
                < 2.0e-14,
            "degree-four/degree-two ratio={}, expected={}",
            s42.sine / s22.sine,
            DEGREE_4_LOVE_NUMBERS[2] / DEGREE_2_LOVE_NUMBERS[2]
        );

        let weighted_degree_three = (4.904_869_5e12 / earth_mu)
            * (earth_radius / moon_distance).powi(4)
            + (1.327_124_4e20 / earth_mu) * (earth_radius / sun_distance).powi(4);
        let p30 = 7.0_f64.sqrt() / 2.0 * (5.0 * latitude_sine.powi(3) - 3.0 * latitude_sine);
        let expected_c30 = (0.093 / 7.0) * 3.0 * weighted_degree_three * p30;
        let c30 = evaluator
            .coefficient_delta(3, 0)
            .expect("degree-three zonal")
            .expect("covered coefficient");
        assert!((c30.cosine - expected_c30).abs() < 2.0e-22);
        assert_eq!(c30.sine, 0.0);
    }

    #[test]
    fn zero_tide_removes_only_the_iers_permanent_zonal_part() {
        let tide_free = provider(TideSystem::TideFree, fixture());
        let zero_tide = provider(TideSystem::ZeroTide, fixture());
        let epoch = Epoch::from_tai_seconds(123.0);
        let tide_free_evaluator = tide_free.evaluator_at(epoch).expect("tide-free epoch");
        let zero_tide_evaluator = zero_tide.evaluator_at(epoch).expect("zero-tide epoch");
        for degree in 2..=4 {
            for order in 0..=degree.min(4) {
                let free = tide_free_evaluator
                    .coefficient_delta(degree, order)
                    .expect("tide-free delta")
                    .expect("tide-free coverage");
                let zero = zero_tide_evaluator
                    .coefficient_delta(degree, order)
                    .expect("zero-tide delta")
                    .expect("zero-tide coverage");
                let expected_shift = if degree == 2 && order == 0 {
                    -PERMANENT_C20
                } else {
                    0.0
                };
                assert!((zero.cosine - free.cosine - expected_shift).abs() < 5.0e-22);
                assert_eq!(zero.sine, free.sine);
            }
        }
    }

    #[test]
    fn all_coefficients_share_one_pair_of_explicit_ephemeris_requests() {
        let ephemeris = fixture();
        let provider = provider(TideSystem::TideFree, Arc::clone(&ephemeris));
        let evaluator = provider
            .evaluator_at(Epoch::from_tai_seconds(456.0))
            .expect("resolved epoch");
        for degree in 0..=4 {
            for order in 0..=degree {
                evaluator
                    .coefficient_delta(degree, order)
                    .expect("coefficient lookup");
            }
        }
        assert_eq!(
            ephemeris
                .requests
                .load(std::sync::atomic::Ordering::Relaxed),
            2
        );
    }

    #[test]
    fn ephemeris_failures_are_preserved_as_provider_errors() {
        let ephemeris = Arc::new(FixedEphemeris {
            moon: [384_400_000.0, 0.0, 0.0],
            sun: [149_597_870_700.0, 0.0, 0.0],
            requests: std::sync::atomic::AtomicUsize::new(0),
            fail: true,
            wrong_epoch: false,
            wrong_frame: false,
        });
        let provider = provider(TideSystem::TideFree, ephemeris);
        let error = provider
            .evaluator_at(Epoch::from_tai_seconds(456.0))
            .expect_err("failed ephemeris must not become a zero tide");
        let error = error
            .downcast_ref::<SolidEarthTideEvaluationError>()
            .expect("typed solid-Earth tide source error");
        assert!(matches!(
            error,
            SolidEarthTideEvaluationError::Ephemeris {
                body: Body::MOON,
                source,
                ..
            } if matches!(source.as_ref(), BodyEphemerisError::OutsideCoverage { .. })
        ));
    }

    #[test]
    fn inconsistent_or_interior_ephemeris_states_are_rejected() {
        for (wrong_epoch, wrong_frame) in [(true, false), (false, true)] {
            let ephemeris = Arc::new(FixedEphemeris {
                moon: [384_400_000.0, 0.0, 0.0],
                sun: [149_597_870_700.0, 0.0, 0.0],
                requests: std::sync::atomic::AtomicUsize::new(0),
                fail: false,
                wrong_epoch,
                wrong_frame,
            });
            let provider = provider(TideSystem::TideFree, ephemeris);
            let error = provider
                .evaluator_at(Epoch::from_tai_seconds(456.0))
                .expect_err("ephemeris metadata mismatch");
            assert!(matches!(
                error.downcast_ref::<SolidEarthTideEvaluationError>(),
                Some(SolidEarthTideEvaluationError::InconsistentEphemerisState {
                    body: Body::MOON
                })
            ));
        }

        let interior = Arc::new(FixedEphemeris {
            moon: [0.0, 0.0, 0.0],
            sun: [149_597_870_700.0, 0.0, 0.0],
            requests: std::sync::atomic::AtomicUsize::new(0),
            fail: false,
            wrong_epoch: false,
            wrong_frame: false,
        });
        let provider = provider(TideSystem::TideFree, interior);
        let error = provider
            .evaluator_at(Epoch::from_tai_seconds(456.0))
            .expect_err("non-exterior tide raiser position");
        assert!(matches!(
            error.downcast_ref::<SolidEarthTideEvaluationError>(),
            Some(SolidEarthTideEvaluationError::InvalidTideRaiserDistance { body: Body::MOON })
        ));
    }

    #[test]
    fn tide_raiser_identity_and_provenance_are_required() {
        assert!(matches!(
            TideRaiser::new(
                Body::EARTH,
                GravitationalParameter::try_from(1.0).expect("positive GM"),
                source("test source", "Earth GM"),
            ),
            Err(ConstructionError::InvalidTideRaiserIdentity)
        ));
        assert!(matches!(
            TideRaiser::new(
                Body::MOON,
                GravitationalParameter::try_from(1.0).expect("positive GM"),
                source("", "Moon GM"),
            ),
            Err(ConstructionError::InvalidProvenance {
                provider: ProvenanceProvider::TideRaiser,
                field: super::super::ProvenanceField::Authority,
                ..
            })
        ));
    }

    #[test]
    fn epoch_coverage_and_non_earth_inputs_are_not_silently_accepted() {
        let provider = provider(TideSystem::TideFree, fixture());
        let outside = provider
            .evaluator_at(Epoch::from_tai_seconds(10_001.0))
            .expect("out-of-coverage evaluator");
        assert_eq!(outside.coefficient_delta(2, 0).expect("query"), None);

        let mut field = SphericalHarmonicField {
            gravitational_parameter: GravitationalParameter::try_from(3.986_004_418e14)
                .expect("positive Earth GM"),
            reference_radius: Length::new::<meter>(6_378_137.0),
            origin: FrameOrigin::Body(Body::EARTH),
            inertial_frame: frames::InertialFrame::GCRF,
            body_fixed_frame: ReferenceFrame::ITRF2020,
            time_scale: TimeScale::TAI,
            maximum_degree: 4,
            maximum_order: 4,
        };
        field.body_fixed_frame = ReferenceFrame::new(
            FrameOrigin::Body(Body::EARTH),
            FrameOrientation::custom(CustomFrameId::new(1), FrameMotion::NonInertial),
        );
        assert!(matches!(
            Iers2010SolidEarthTideProvider::new(
                field,
                TideSystem::TideFree,
                provider.metadata.coverage,
                fixture(),
                vec![source("test ephemeris", "vectors")],
                raiser(Body::MOON, 4.904_869_5e12),
                raiser(Body::SUN, 1.327_124_4e20),
            ),
            Err(ConstructionError::SolidEarthTideRequiresItrf2020)
        ));
    }

    #[test]
    fn mean_tide_is_explicitly_rejected() {
        let field = SphericalHarmonicField {
            gravitational_parameter: GravitationalParameter::try_from(3.986_004_418e14)
                .expect("positive Earth GM"),
            reference_radius: Length::new::<meter>(6_378_137.0),
            origin: FrameOrigin::Body(Body::EARTH),
            inertial_frame: frames::InertialFrame::GCRF,
            body_fixed_frame: ReferenceFrame::ITRF2020,
            time_scale: TimeScale::TAI,
            maximum_degree: 4,
            maximum_order: 4,
        };
        assert!(matches!(
            Iers2010SolidEarthTideProvider::new(
                field,
                TideSystem::MeanTide,
                CoefficientDeltaCoverage {
                    start: Epoch::from_tai_seconds(0.0),
                    end: Epoch::from_tai_seconds(1.0),
                    time_scale: TimeScale::TAI,
                },
                fixture(),
                vec![source("test ephemeris", "vectors")],
                raiser(Body::MOON, 4.904_869_5e12),
                raiser(Body::SUN, 1.327_124_4e20),
            ),
            Err(ConstructionError::UnsupportedSolidEarthTideSystem)
        ));
    }
}
