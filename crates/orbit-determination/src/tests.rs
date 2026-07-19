use std::sync::Arc;

use bodies::Body;
use dynamics::{EllipticKeplerPropagator, PointMassGravityModel, TwoBodyDynamics};
use frames::{FrameOrigin, ReferenceFrame};
use gravity::{CentralGravityProvider, SharedCentralGravity};
use hifitime::Epoch;
use orbits::cartesian::CartesianState;
use units::{GravitationalParameter, Position, VelocityVector};

use crate::{
    CartesianCovariance, CartesianPositionObservation, CartesianStateEstimate, CorrectionEvent,
    EstimationObserver, ExtendedKalmanFilter, KalmanFilter, Orbit, OrbitDetermination,
    PositionCovariance, PredictionEvent, StateEstimate, UnscentedKalmanFilter,
};

const EARTH_MU: f64 = 3.986_004_415e14;

#[derive(Debug)]
struct EarthGravity;

impl CentralGravityProvider for EarthGravity {
    fn origin(&self) -> FrameOrigin {
        FrameOrigin::Body(Body::EARTH)
    }
    fn parameter(&self) -> GravitationalParameter {
        GravitationalParameter::try_from(EARTH_MU).expect("positive parameter")
    }
}

fn problem() -> TwoBodyDynamics {
    let gravity: SharedCentralGravity = Arc::new(EarthGravity);
    TwoBodyDynamics::new(PointMassGravityModel::new(gravity))
}

fn propagator() -> EllipticKeplerPropagator {
    EllipticKeplerPropagator::new(problem())
}

fn estimate(position: [f64; 3], velocity: [f64; 3]) -> CartesianStateEstimate {
    let epoch = Epoch::from_tai_seconds(0.0);
    let state = CartesianState::new(
        ReferenceFrame::GCRF,
        Position::from_metres(position[0], position[1], position[2]),
        VelocityVector::from_metres_per_second(velocity[0], velocity[1], velocity[2]),
    )
    .expect("finite state");
    StateEstimate::new(Orbit::new(epoch, state), covariance(1.0e6)).expect("matching frames")
}

fn covariance(value: f64) -> CartesianCovariance {
    let standard_deviation = value.sqrt();
    CartesianCovariance::from_standard_deviations(
        ReferenceFrame::GCRF,
        Position::from_metres(standard_deviation, standard_deviation, standard_deviation),
        VelocityVector::from_metres_per_second(
            standard_deviation,
            standard_deviation,
            standard_deviation,
        ),
    )
    .expect("positive covariance")
}

fn position_covariance(value: f64) -> PositionCovariance {
    let standard_deviation = value.sqrt();
    PositionCovariance::from_standard_deviations(
        ReferenceFrame::GCRF,
        Position::from_metres(standard_deviation, standard_deviation, standard_deviation),
    )
    .expect("positive covariance")
}

fn observation() -> CartesianPositionObservation {
    CartesianPositionObservation::new(
        Epoch::from_tai_seconds(0.0),
        Position::from_metres(7_000_010.0, -10.0, 5.0),
        position_covariance(25.0),
    )
    .expect("valid observation")
}

fn before_after(
    estimate: &CartesianStateEstimate,
    posterior: &CartesianStateEstimate,
) -> (f64, f64) {
    let truth = Position::from_metres(7_000_000.0, 0.0, 0.0);
    (
        (estimate.orbit().as_ref().position() - truth)
            .norm()
            .get::<units::uom::si::length::meter>(),
        (posterior.orbit().as_ref().position() - truth)
            .norm()
            .get::<units::uom::si::length::meter>(),
    )
}

#[test]
fn extended_filter_uses_dedicated_propagator_contract() {
    let prior = estimate([6_999_600.0, 350.0, -250.0], [0.0, 7_546.0, 0.0]);
    let mut filter = ExtendedKalmanFilter::new(propagator(), prior.clone(), covariance(1.0e-9))
        .expect("valid filter");
    let posterior = filter.estimate(&observation()).expect("correction");
    let (before, after) = before_after(&prior, &posterior);
    assert!(after < before);
}

#[test]
fn unscented_filter_implements_the_same_kalman_contract() {
    let prior = estimate([6_999_600.0, 350.0, -250.0], [0.0, 7_546.0, 0.0]);
    let mut filter = UnscentedKalmanFilter::new(propagator(), prior.clone(), covariance(1.0e-9))
        .expect("valid filter");
    let posterior = filter.estimate(&observation()).expect("correction");
    let (before, after) = before_after(&prior, &posterior);
    assert!(after < before);
}

#[test]
fn observer_is_opt_in_for_a_single_estimation_call() {
    #[derive(Default)]
    struct Observer {
        predictions: usize,
        corrections: usize,
        residual_norm: f64,
    }
    impl EstimationObserver for Observer {
        fn on_prediction(&mut self, _: PredictionEvent<'_>) {
            self.predictions += 1;
        }
        fn on_correction(&mut self, event: CorrectionEvent<'_>) {
            self.corrections += 1;
            self.residual_norm = event.residual.norm().get::<units::uom::si::length::meter>();
        }
    }
    let prior = estimate([6_999_600.0, 350.0, -250.0], [0.0, 7_546.0, 0.0]);
    let mut filter =
        ExtendedKalmanFilter::new(propagator(), prior, covariance(1.0e-9)).expect("valid filter");
    let mut observer = Observer::default();
    filter
        .estimate_with_observer(&observation(), &mut observer)
        .expect("correction");
    assert_eq!((observer.predictions, observer.corrections), (1, 1));
    assert!(observer.residual_norm.is_finite());
}

#[test]
fn observation_series_reuses_the_same_problem_and_filter() {
    let prior = estimate([6_999_600.0, 350.0, -250.0], [0.0, 7_546.0, 0.0]);
    let mut filter =
        ExtendedKalmanFilter::new(propagator(), prior, covariance(1.0e-9)).expect("valid filter");
    let observation = observation();
    assert_eq!(
        filter.estimate_all([&observation]).expect("series").len(),
        1
    );
}
