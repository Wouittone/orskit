use dynamics::Propagator;
use nalgebra::Cholesky;
use orbits::cartesian::CartesianState;

use crate::{
    covariance::{
        cartesian_covariance_from_raw, cartesian_covariance_raw, symmetrize,
        validate_positive_definite,
    },
    numerical::{
        orbit_from_raw, position_from_raw, raw_position, raw_state, RawCovariance, RawPosition,
        RawState,
    },
    CartesianCovariance, CartesianObservation, CartesianStateEstimate, CorrectionEvent,
    EstimationObserver, KalmanFilter, OrbitDetermination, OrbitDeterminationError, PredictionEvent,
};

/// Scaled unscented-transform parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnscentedConfiguration {
    alpha: f64,
    beta: f64,
    kappa: f64,
}

impl Default for UnscentedConfiguration {
    fn default() -> Self {
        Self {
            alpha: 1.0e-3,
            beta: 2.0,
            kappa: 0.0,
        }
    }
}

impl UnscentedConfiguration {
    /// Creates a scaled unscented-transform configuration.
    pub fn new(alpha: f64, beta: f64, kappa: f64) -> Result<Self, OrbitDeterminationError> {
        let configuration = Self { alpha, beta, kappa };
        configuration.weights()?;
        Ok(configuration)
    }

    fn weights(self) -> Result<(f64, f64, f64), OrbitDeterminationError> {
        const DIMENSION: f64 = 6.0;
        let scaling = self.alpha * self.alpha * (DIMENSION + self.kappa);
        let denominator = DIMENSION + scaling - DIMENSION;
        if !self.alpha.is_finite()
            || !self.beta.is_finite()
            || !self.kappa.is_finite()
            || scaling <= 0.0
            || !denominator.is_finite()
            || denominator <= 0.0
        {
            return Err(OrbitDeterminationError::InvalidUnscentedConfiguration);
        }
        let lambda = scaling - DIMENSION;
        Ok((
            lambda,
            lambda / scaling,
            lambda / scaling + (1.0 - self.alpha * self.alpha + self.beta),
        ))
    }
}

/// Unscented Kalman filter using an application-selected orskit propagator.
#[derive(Debug, Clone)]
pub struct UnscentedKalmanFilter<P> {
    propagator: P,
    estimate: CartesianStateEstimate,
    process_noise: CartesianCovariance,
    configuration: UnscentedConfiguration,
}

impl<P> UnscentedKalmanFilter<P> {
    /// Creates a UKF with default scaled-transform parameters.
    pub fn new(
        propagator: P,
        estimate: CartesianStateEstimate,
        process_noise: CartesianCovariance,
    ) -> Result<Self, OrbitDeterminationError> {
        Self::with_configuration(
            propagator,
            estimate,
            process_noise,
            UnscentedConfiguration::default(),
        )
    }

    /// Creates a UKF with explicit scaled-transform parameters.
    pub fn with_configuration(
        propagator: P,
        estimate: CartesianStateEstimate,
        process_noise: CartesianCovariance,
        configuration: UnscentedConfiguration,
    ) -> Result<Self, OrbitDeterminationError> {
        configuration.weights()?;
        if estimate.orbit().as_ref().frame() != process_noise.frame() {
            return Err(OrbitDeterminationError::FrameMismatch);
        }
        Ok(Self {
            propagator,
            estimate,
            process_noise,
            configuration,
        })
    }

    /// Returns the most recent posterior estimate.
    #[must_use]
    pub const fn current_estimate(&self) -> &CartesianStateEstimate {
        &self.estimate
    }
}

impl<P> UnscentedKalmanFilter<P> {
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
        let (lambda, central_mean_weight, central_covariance_weight) =
            self.configuration.weights()?;
        let scale = (6.0 + lambda).sqrt();
        let sigma_points = sigma_points(
            raw_state(self.estimate.orbit().as_ref()),
            cartesian_covariance_raw(self.estimate.covariance()),
            scale,
        )?;
        let mut propagated = Vec::with_capacity(sigma_points.len());
        for sigma in sigma_points {
            let orbit = orbit_from_raw(self.estimate.orbit().epoch(), observation.frame(), sigma)?;
            propagated.push(
                self.propagator
                    .propagate(orbit, observation.epoch())
                    .map_err(OrbitDeterminationError::propagation)?,
            );
        }
        let state_weight = 1.0 / (2.0 * (6.0 + lambda));
        let predicted_raw = weighted_state_mean(&propagated, central_mean_weight, state_weight);
        let predicted_covariance = predicted_state_covariance(
            &propagated,
            predicted_raw,
            central_covariance_weight,
            state_weight,
            cartesian_covariance_raw(&self.process_noise),
        );
        validate_positive_definite(&predicted_covariance, "predicted Cartesian covariance")?;
        let predicted = CartesianStateEstimate::new(
            orbit_from_raw(observation.epoch(), observation.frame(), predicted_raw)?,
            cartesian_covariance_from_raw(observation.frame(), predicted_covariance)?,
        )?;
        if let Some(observer) = &mut observer {
            observer.on_prediction(PredictionEvent {
                estimate: &predicted,
            });
        }

        let predicted_measurements: Vec<RawPosition> = propagated
            .iter()
            .map(|orbit| raw_position(orbit.as_ref().position()))
            .collect();
        let measurement_mean =
            weighted_position_mean(&predicted_measurements, central_mean_weight, state_weight);
        let innovation_covariance = predicted_measurement_covariance(
            &predicted_measurements,
            measurement_mean,
            central_covariance_weight,
            state_weight,
            observation.covariance().raw(),
        );
        let cross_covariance = cross_covariance(
            &propagated,
            predicted_raw,
            &predicted_measurements,
            measurement_mean,
            central_covariance_weight,
            state_weight,
        );
        let factor = Cholesky::new(innovation_covariance)
            .ok_or(OrbitDeterminationError::SingularInnovationCovariance)?;
        let gain = factor.solve(&cross_covariance.transpose()).transpose();
        let innovation = observation.position() - position_from_raw(measurement_mean);
        let corrected_raw = predicted_raw + gain * raw_position(innovation);
        let corrected_covariance =
            symmetrize(predicted_covariance - gain * innovation_covariance * gain.transpose());
        let posterior = CartesianStateEstimate::new(
            orbit_from_raw(observation.epoch(), observation.frame(), corrected_raw)?,
            cartesian_covariance_from_raw(observation.frame(), corrected_covariance)?,
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

impl<P, Observation> OrbitDetermination<Observation> for UnscentedKalmanFilter<P>
where
    P: Propagator<CartesianState>,
    Observation: CartesianObservation,
{
    type Estimate = CartesianStateEstimate;
    type Error = OrbitDeterminationError;
    fn estimate(&mut self, observation: &Observation) -> Result<Self::Estimate, Self::Error> {
        self.estimation_step(observation, None)
    }
}

impl<P, Observation> KalmanFilter<Observation> for UnscentedKalmanFilter<P>
where
    P: Propagator<CartesianState>,
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

fn sigma_points(
    mean: RawState,
    covariance: RawCovariance,
    scale: f64,
) -> Result<Vec<RawState>, OrbitDeterminationError> {
    let root = Cholesky::new(covariance)
        .ok_or(OrbitDeterminationError::NotPositiveDefinite {
            what: "Cartesian covariance",
        })?
        .l()
        * scale;
    let mut points = Vec::with_capacity(13);
    points.push(mean);
    for column in 0..6 {
        points.push(mean + root.column(column));
    }
    for column in 0..6 {
        points.push(mean - root.column(column));
    }
    Ok(points)
}

fn weighted_state_mean(
    points: &[orskit_core::Orbit<CartesianState>],
    central_weight: f64,
    weight: f64,
) -> RawState {
    points
        .iter()
        .enumerate()
        .fold(RawState::zeros(), |sum, (index, point)| {
            sum + raw_state(point.as_ref()) * if index == 0 { central_weight } else { weight }
        })
}

fn weighted_position_mean(points: &[RawPosition], central_weight: f64, weight: f64) -> RawPosition {
    points
        .iter()
        .enumerate()
        .fold(RawPosition::zeros(), |sum, (index, point)| {
            sum + point * if index == 0 { central_weight } else { weight }
        })
}

fn predicted_state_covariance(
    points: &[orskit_core::Orbit<CartesianState>],
    mean: RawState,
    central_weight: f64,
    weight: f64,
    process_noise: RawCovariance,
) -> RawCovariance {
    symmetrize(
        points
            .iter()
            .enumerate()
            .fold(process_noise, |sum, (index, point)| {
                let delta = raw_state(point.as_ref()) - mean;
                sum + delta * delta.transpose() * if index == 0 { central_weight } else { weight }
            }),
    )
}

fn predicted_measurement_covariance(
    points: &[RawPosition],
    mean: RawPosition,
    central_weight: f64,
    weight: f64,
    measurement_noise: nalgebra::Matrix3<f64>,
) -> nalgebra::Matrix3<f64> {
    symmetrize(
        points
            .iter()
            .enumerate()
            .fold(measurement_noise, |sum, (index, point)| {
                let delta = point - mean;
                sum + delta * delta.transpose() * if index == 0 { central_weight } else { weight }
            }),
    )
}

fn cross_covariance(
    states: &[orskit_core::Orbit<CartesianState>],
    state_mean: RawState,
    measurements: &[RawPosition],
    measurement_mean: RawPosition,
    central_weight: f64,
    weight: f64,
) -> nalgebra::SMatrix<f64, 6, 3> {
    states.iter().zip(measurements).enumerate().fold(
        nalgebra::SMatrix::<f64, 6, 3>::zeros(),
        |sum, (index, (state, measurement))| {
            let state_delta = raw_state(state.as_ref()) - state_mean;
            let measurement_delta = measurement - measurement_mean;
            sum + state_delta
                * measurement_delta.transpose()
                * if index == 0 { central_weight } else { weight }
        },
    )
}
