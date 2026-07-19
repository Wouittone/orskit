use dynamics::Propagator;
use nalgebra::Cholesky;
use orbits::cartesian::CartesianState;

use crate::{
    covariance::{symmetrize, validate_positive_definite},
    numerical::{position_jacobian, propagate_with_transition, raw_position, RawCovariance},
    CartesianCovariance, CartesianObservation, CartesianStateEstimate, CorrectionEvent,
    EstimationObserver, KalmanFilter, OrbitDetermination, OrbitDeterminationError, PredictionEvent,
};

/// Extended Kalman filter using an application-selected orskit propagator.
#[derive(Debug, Clone)]
pub struct ExtendedKalmanFilter<P> {
    propagator: P,
    estimate: CartesianStateEstimate,
    process_noise: CartesianCovariance,
}

impl<P> ExtendedKalmanFilter<P> {
    /// Creates a filter with a selected propagator, prior, and process noise.
    pub fn new(
        propagator: P,
        estimate: CartesianStateEstimate,
        process_noise: CartesianCovariance,
    ) -> Result<Self, OrbitDeterminationError> {
        if estimate.orbit().as_ref().frame() != process_noise.frame() {
            return Err(OrbitDeterminationError::FrameMismatch);
        }
        Ok(Self {
            propagator,
            estimate,
            process_noise,
        })
    }

    /// Returns the selected propagation algorithm.
    #[must_use]
    pub const fn propagator(&self) -> &P {
        &self.propagator
    }

    /// Returns the most recent posterior estimate.
    #[must_use]
    pub const fn current_estimate(&self) -> &CartesianStateEstimate {
        &self.estimate
    }
}

impl<P> ExtendedKalmanFilter<P> {
    fn estimation_step<Observation>(
        &mut self,
        observation: &Observation,
        mut observer: Option<&mut dyn EstimationObserver>,
    ) -> Result<CartesianStateEstimate, OrbitDeterminationError>
    where
        P: Propagator<CartesianState>,
        Observation: CartesianObservation,
    {
        if observation.frame() != self.estimate.orbit().as_ref().frame() {
            return Err(OrbitDeterminationError::FrameMismatch);
        }
        let (predicted_orbit, transition) = propagate_with_transition(
            &self.propagator,
            self.estimate.orbit().clone(),
            observation.epoch(),
        )?;
        if predicted_orbit.as_ref().frame() != observation.frame() {
            return Err(OrbitDeterminationError::FrameMismatch);
        }
        let predicted_covariance = symmetrize(
            transition * self.estimate.covariance().raw() * transition.transpose()
                + self.process_noise.raw(),
        );
        validate_positive_definite(&predicted_covariance, "predicted Cartesian covariance")?;
        let predicted = CartesianStateEstimate::new(
            predicted_orbit,
            CartesianCovariance::from_raw(observation.frame(), predicted_covariance)?,
        )?;
        if let Some(observer) = &mut observer {
            observer.on_prediction(PredictionEvent {
                estimate: &predicted,
            });
        }

        let innovation = observation.position() - predicted.orbit().as_ref().position();
        let jacobian = position_jacobian();
        let innovation_covariance = symmetrize(
            jacobian * predicted.covariance().raw() * jacobian.transpose()
                + observation.covariance().raw(),
        );
        let factor = Cholesky::new(innovation_covariance)
            .ok_or(OrbitDeterminationError::SingularInnovationCovariance)?;
        let gain = factor
            .solve(&(jacobian * predicted.covariance().raw()))
            .transpose();
        let corrected_raw = crate::numerical::raw_state(predicted.orbit().as_ref())
            + gain * raw_position(innovation);
        let projection = RawCovariance::identity() - gain * jacobian;
        let corrected_covariance = symmetrize(
            projection * predicted.covariance().raw() * projection.transpose()
                + gain * observation.covariance().raw() * gain.transpose(),
        );
        let corrected_orbit = crate::numerical::orbit_from_raw(
            observation.epoch(),
            observation.frame(),
            corrected_raw,
        )?;
        let posterior = CartesianStateEstimate::new(
            corrected_orbit,
            CartesianCovariance::from_raw(observation.frame(), corrected_covariance)?,
        )?;
        if let Some(observer) = &mut observer {
            observer.on_correction(CorrectionEvent {
                innovation,
                residual: observation.position() - posterior.orbit().as_ref().position(),
                innovation_covariance: crate::PositionCovariance::from_raw(
                    observation.frame(),
                    innovation_covariance,
                )?,
                estimate: &posterior,
            });
        }
        self.estimate = posterior.clone();
        Ok(posterior)
    }
}

impl<P, Observation> OrbitDetermination<Observation> for ExtendedKalmanFilter<P>
where
    P: Propagator<CartesianState> + Send + Sync + std::fmt::Debug,
    Observation: CartesianObservation,
{
    type Estimate = CartesianStateEstimate;
    type Error = OrbitDeterminationError;

    fn estimate(&mut self, observation: &Observation) -> Result<Self::Estimate, Self::Error> {
        self.estimation_step(observation, None)
    }
}

impl<P, Observation> KalmanFilter<Observation> for ExtendedKalmanFilter<P>
where
    P: Propagator<CartesianState> + Send + Sync + std::fmt::Debug,
    Observation: CartesianObservation,
{
    fn estimate_with_observer(
        &mut self,
        observation: &Observation,
        observer: &mut dyn EstimationObserver,
    ) -> Result<Self::Estimate, Self::Error> {
        self.estimation_step(observation, Some(observer))
    }
}
