//! Original summed, backward-difference Gauss--Jackson implementation.
//!
//! Berry and Healy, *Implementation of Gauss-Jackson Integration for Orbit
//! Propagation*, J. Astronautical Sciences 52(3), 2004, pp. 331--357,
//! <https://hdl.handle.net/1903/2202>, derive summed Adams/Gauss--Jackson
//! operators (equations 38, 52). Only equations were used, not supplemental
//! code or another library's implementation.

use super::*;

const POINTS: usize = 9;

/// Fixed-step eighth-order settings, including a separately controlled startup.
///
/// The eight startup intervals use native Bogacki--Shampine. Its local error
/// settings must be substantially tighter than the desired long-arc error.
/// Corrector tolerances bound iteration changes, not truncation/global error.
#[derive(Debug, Clone, Copy)]
pub struct GaussJacksonConfiguration {
    step: Duration,
    startup: IntegrationConfiguration,
    position_tolerance: Length,
    velocity_tolerance: Velocity,
    max_iterations: usize,
    max_steps: usize,
}

impl GaussJacksonConfiguration {
    /// Configures the only supported order (eight).
    ///
    /// `step` is a positive magnitude in either propagation direction. Targets
    /// must be integer multiples of it in exact duration nanoseconds. Limits
    /// include startup steps; no step resizing or terminal partial step occurs.
    ///
    /// ```
    /// use dynamics_numerical::{GaussJacksonConfiguration, IntegrationConfiguration};
    /// use hifitime::Duration;
    /// use units::{Length, Ratio, Velocity};
    /// use units::uom::si::{length::meter, ratio::ratio, velocity::meter_per_second};
    ///
    /// let startup = IntegrationConfiguration::new(
    ///     Length::new::<meter>(1e-7), Velocity::new::<meter_per_second>(1e-10),
    ///     Ratio::new::<ratio>(1e-15), Duration::from_seconds(1e-8),
    ///     Duration::from_seconds(1.0), Duration::from_seconds(0.1), 100_000, 10_000,
    /// )?;
    /// let configuration = GaussJacksonConfiguration::new(
    ///     Duration::from_seconds(30.0), startup,
    ///     Length::new::<meter>(1e-8), Velocity::new::<meter_per_second>(1e-11),
    ///     20, 1_000_000,
    /// )?;
    /// assert_eq!(configuration.step(), Duration::from_seconds(30.0));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new(
        step: Duration,
        startup: IntegrationConfiguration,
        position_tolerance: Length,
        velocity_tolerance: Velocity,
        max_iterations: usize,
        max_steps: usize,
    ) -> Result<Self, GaussJacksonConfigurationError> {
        if step.total_nanoseconds() <= 0 || !step.to_seconds().is_finite() {
            return Err(GaussJacksonConfigurationError::InvalidStep);
        }
        if !position_tolerance.get::<meter>().is_finite()
            || position_tolerance.get::<meter>() <= 0.0
        {
            return Err(GaussJacksonConfigurationError::InvalidPositionTolerance);
        }
        if !velocity_tolerance.get::<meter_per_second>().is_finite()
            || velocity_tolerance.get::<meter_per_second>() <= 0.0
        {
            return Err(GaussJacksonConfigurationError::InvalidVelocityTolerance);
        }
        if max_iterations == 0 || max_steps == 0 {
            return Err(GaussJacksonConfigurationError::ZeroLimit);
        }
        Ok(Self {
            step,
            startup,
            position_tolerance,
            velocity_tolerance,
            max_iterations,
            max_steps,
        })
    }

    /// Returns the fixed positive step magnitude.
    #[must_use]
    pub const fn step(self) -> Duration {
        self.step
    }

    /// Returns the independently controlled native startup settings.
    #[must_use]
    pub const fn startup(self) -> IntegrationConfiguration {
        self.startup
    }

    /// Returns the per-component position correction threshold.
    #[must_use]
    pub const fn position_tolerance(self) -> Length {
        self.position_tolerance
    }

    /// Returns the per-component velocity correction threshold.
    #[must_use]
    pub const fn velocity_tolerance(self) -> Velocity {
        self.velocity_tolerance
    }

    /// Returns the maximum fixed-point corrections per mature step.
    #[must_use]
    pub const fn max_iterations(self) -> usize {
        self.max_iterations
    }

    /// Returns the arc step limit, including startup intervals.
    #[must_use]
    pub const fn max_steps(self) -> usize {
        self.max_steps
    }
}

/// Invalid fixed-step configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum GaussJacksonConfigurationError {
    /// Step is not positive and finite.
    #[error("Gauss-Jackson step must be positive and finite")]
    InvalidStep,
    /// Iteration position threshold is not positive and finite.
    #[error("corrector position tolerance must be positive and finite")]
    InvalidPositionTolerance,
    /// Iteration velocity threshold is not positive and finite.
    #[error("corrector velocity tolerance must be positive and finite")]
    InvalidVelocityTolerance,
    /// One of the resource limits is zero.
    #[error("corrector iteration and propagation step limits must be nonzero")]
    ZeroLimit,
}

/// Fixed-step propagation failure; no failed candidate is returned as a result.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum GaussJacksonPropagationError<E: Error + Send + Sync + 'static> {
    /// Startup, dynamics, frame, or finite-value validation failed.
    #[error("Gauss-Jackson dynamics or startup failed")]
    Numerical(#[from] NumericalPropagationError<E>),
    /// The frame does not affirmatively declare inertial axes.
    #[error("Gauss-Jackson requires an inertial frame")]
    NonInertialFrame,
    /// A partial step would invalidate the uniform history.
    #[error("target is not on the fixed-step grid from the initial epoch")]
    TargetOffGrid,
    /// The requested arc exceeds the configured work limit.
    #[error("requested arc exceeds the Gauss-Jackson step limit")]
    StepLimitExceeded,
    /// Fixed-point correction failed at this epoch.
    #[error("Gauss-Jackson corrector did not converge at {epoch} after {iterations} iterations")]
    NonConvergence {
        /// Epoch of the unsuccessful candidate.
        epoch: Epoch,
        /// Number of attempted corrections.
        iterations: usize,
    },
}

/// Explicitly opt-in, fixed-step eighth-order summed Gauss--Jackson propagator.
///
/// Advances SI Cartesian position/velocity in the initial inertial frame and
/// preserves the complete frame (including origin) and exact target epoch.
/// Dynamics can depend on time, position, and velocity, but must be smooth,
/// non-stiff, and valid throughout the history. Each call creates fresh history:
/// eight forward/backward startup steps followed by iterated corrections.
/// Arcs of at most eight steps use only the configured native startup method.
///
/// There is **no dense output**, off-grid endpoint interpolation, event,
/// maneuver, STM, mass, or covariance extension. Restart after discontinuities,
/// frame changes, or step changes. This is not an adaptive/error-controlled
/// long-arc method: demonstrate step refinement for the selected force model.
/// Corrector convergence does not certify stability or scientific accuracy;
/// stiff dynamics, high eccentricity/periapsis, or too large a step can fail.
/// History and the mature loop are fixed-size stack storage; the problem may
/// allocate independently. No external ODE solver is linked.
#[derive(Debug, Clone)]
pub struct GaussJackson8<P> {
    startup: BogackiShampine32<P>,
    configuration: GaussJacksonConfiguration,
    position_weights: [f64; POINTS],
    velocity_weights: [f64; POINTS],
}

impl<P> GaussJackson8<P> {
    /// Selects one owned physical problem and explicit fixed-step settings.
    #[must_use]
    pub fn new(problem: P, configuration: GaussJacksonConfiguration) -> Self {
        let (position_weights, velocity_weights) = summed_weights();
        Self {
            startup: BogackiShampine32::new(problem, configuration.startup),
            configuration,
            position_weights,
            velocity_weights,
        }
    }

    /// Returns the owned physical problem.
    #[must_use]
    pub const fn problem(&self) -> &P {
        self.startup.problem()
    }

    /// Returns the fixed-step and startup configuration.
    #[must_use]
    pub const fn configuration(&self) -> GaussJacksonConfiguration {
        self.configuration
    }
}

impl<P: CartesianDynamics> Propagator<CartesianState> for GaussJackson8<P> {
    type Error = GaussJacksonPropagationError<P::Error>;

    fn propagate(
        &self,
        initial: Orbit<CartesianState>,
        target: Epoch,
    ) -> Result<Orbit<CartesianState>, Self::Error> {
        let OrbitParts { epoch, state } = initial.into();
        if !state.frame().is_inertial() {
            return Err(GaussJacksonPropagationError::NonInertialFrame);
        }
        self.problem()
            .validate(&state)
            .map_err(NumericalPropagationError::Dynamics)?;
        let total = (target - epoch).total_nanoseconds();
        let step = self.configuration.step.total_nanoseconds();
        if total % step != 0 {
            return Err(GaussJacksonPropagationError::TargetOffGrid);
        }
        let count = total.unsigned_abs() / step as u128;
        if count > self.configuration.max_steps as u128 {
            return Err(GaussJacksonPropagationError::StepLimitExceeded);
        }
        if count == 0 {
            return Ok(Orbit::new(target, state));
        }
        let signed_step = if total < 0 { -step } else { step };
        let h = Duration::from_total_nanoseconds(signed_step).to_seconds();
        let frame = state.frame();
        let mut values = state_to_array(state);
        let mut history = [[0.0; 3]; POINTS];
        history[0] = self.acceleration(epoch, frame, values)?;
        for index in 1..=count.min(8) as usize {
            let next_epoch = epoch + Duration::from_total_nanoseconds(signed_step * index as i128);
            let previous_epoch =
                epoch + Duration::from_total_nanoseconds(signed_step * (index - 1) as i128);
            let next = self.startup.propagate(
                Orbit::new(previous_epoch, array_to_state(frame, values)?),
                next_epoch,
            )?;
            values = state_to_array(*next.as_ref());
            history.rotate_right(1);
            history[0] = self.acceleration(next_epoch, frame, values)?;
        }
        if count <= 8 {
            return Ok(Orbit::new(target, array_to_state(frame, values)?));
        }
        let mut correction = weighted(self.position_weights, history);
        let velocity_correction = weighted(self.velocity_weights, history);
        let mut first_sum: [f64; 3] =
            std::array::from_fn(|axis| values[axis + 3] / h - velocity_correction[axis]);
        let mut sum_roundoff = [0.0; 3];
        let mut position_roundoff = [0.0; 3];
        for index in 9..=count as usize {
            let next_epoch = epoch + Duration::from_total_nanoseconds(signed_step * index as i128);
            // Degree-eight extrapolation of the acceleration, not a copied
            // coefficient table: a(n+1) = sum (-1)^j C(9,j+1) a(n-j).
            let mut next_history = history;
            next_history.rotate_right(1);
            next_history[0] = std::array::from_fn(|axis| {
                (0..POINTS)
                    .map(|j| alternating(j) * binomial(9, j + 1) * history[j][axis])
                    .sum()
            });
            let mut candidate = corrected(
                values,
                first_sum,
                correction,
                next_history,
                self.position_weights,
                self.velocity_weights,
                h,
            );
            let mut converged = false;
            for _ in 0..self.configuration.max_iterations {
                next_history[0] = self.acceleration(next_epoch, frame, candidate)?;
                let next = corrected(
                    values,
                    first_sum,
                    correction,
                    next_history,
                    self.position_weights,
                    self.velocity_weights,
                    h,
                );
                let position_ok = (0..3).all(|axis| {
                    (next[axis] - candidate[axis]).abs()
                        <= self.configuration.position_tolerance.get::<meter>()
                });
                let velocity_ok = (3..6).all(|axis| {
                    (next[axis] - candidate[axis]).abs()
                        <= self
                            .configuration
                            .velocity_tolerance
                            .get::<meter_per_second>()
                });
                candidate = next;
                if position_ok && velocity_ok {
                    converged = true;
                    break;
                }
            }
            if !converged {
                return Err(GaussJacksonPropagationError::NonConvergence {
                    epoch: next_epoch,
                    iterations: self.configuration.max_iterations,
                });
            }
            next_history[0] = self.acceleration(next_epoch, frame, candidate)?;
            let next_correction = weighted(self.position_weights, next_history);
            for axis in 0..3 {
                // Accumulate the second sum as position increments; do not
                // subtract large positions to recover an integration constant.
                let increment =
                    h * h * (first_sum[axis] + next_correction[axis] - correction[axis]);
                compensated_add(&mut values[axis], &mut position_roundoff[axis], increment);
                compensated_add(
                    &mut first_sum[axis],
                    &mut sum_roundoff[axis],
                    next_history[0][axis],
                );
            }
            let velocity_correction = weighted(self.velocity_weights, next_history);
            for axis in 0..3 {
                values[axis + 3] = h * (first_sum[axis] + velocity_correction[axis]);
            }
            array_to_state::<P::Error>(frame, values)?;
            history = next_history;
            correction = next_correction;
        }
        Ok(Orbit::new(target, array_to_state(frame, values)?))
    }
}

impl<P: CartesianDynamics> GaussJackson8<P> {
    fn acceleration(
        &self,
        epoch: Epoch,
        frame: ReferenceFrame,
        values: [f64; 6],
    ) -> Result<[f64; 3], NumericalPropagationError<P::Error>> {
        let derivative = self.startup.derivative(epoch, 0.0, frame, values)?;
        Ok([derivative[3], derivative[4], derivative[5]])
    }
}

#[allow(clippy::too_many_arguments)]
fn corrected(
    current: [f64; 6],
    first_sum: [f64; 3],
    previous_correction: [f64; 3],
    history: [[f64; 3]; POINTS],
    position_weights: [f64; POINTS],
    velocity_weights: [f64; POINTS],
    h: f64,
) -> [f64; 6] {
    let position = weighted(position_weights, history);
    let velocity = weighted(velocity_weights, history);
    std::array::from_fn(|component| {
        if component < 3 {
            current[component]
                + h * h
                    * (first_sum[component] + position[component] - previous_correction[component])
        } else {
            let axis = component - 3;
            h * (first_sum[axis] + history[0][axis] + velocity[axis])
        }
    })
}

fn weighted(weights: [f64; POINTS], history: [[f64; 3]; POINTS]) -> [f64; 3] {
    std::array::from_fn(|axis| {
        weights
            .iter()
            .zip(history)
            .map(|(weight, acceleration)| weight * acceleration[axis])
            .sum()
    })
}

fn compensated_add(value: &mut f64, roundoff: &mut f64, increment: f64) {
    let adjusted = increment - *roundoff;
    let next = *value + adjusted;
    *roundoff = (next - *value) - adjusted;
    *value = next;
}

fn alternating(power: usize) -> f64 {
    if power.is_multiple_of(2) {
        1.0
    } else {
        -1.0
    }
}

fn binomial(n: usize, k: usize) -> f64 {
    (0..k).fold(1.0, |value, j| value * (n - j) as f64 / (j + 1) as f64)
}

fn summed_weights() -> ([f64; POINTS], [f64; POINTS]) {
    // x = backward difference, q(x) = x / -log(1-x).
    // v/h = U + ((q-1)/x)a; r/h^2 = W + ((q^2-1+x)/x^2)a.
    // U(n+1)=U(n)+a(n+1), W(n+1)=W(n)+U(n).
    // Truncate both regular parts after x^8, then convert differences to
    // ordinates via x^k a(n) = sum (-1)^j C(k,j) a(n-j).
    let mut q = [0.0; 11];
    q[0] = 1.0;
    for degree in 1..q.len() {
        q[degree] = -(1..=degree)
            .map(|j| q[degree - j] / (j + 1) as f64)
            .sum::<f64>();
    }
    let mut position = [0.0; POINTS];
    let mut velocity = [0.0; POINTS];
    for degree in 0..POINTS {
        let c = (0..=degree + 2)
            .map(|j| q[j] * q[degree + 2 - j])
            .sum::<f64>();
        let b = q[degree + 1];
        for j in 0..=degree {
            let weight = alternating(j) * binomial(degree, j);
            position[j] += c * weight;
            velocity[j] += b * weight;
        }
    }
    (position, velocity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operator_coefficients_and_configuration_boundaries() {
        let (position, velocity) = summed_weights();
        assert!((position.iter().sum::<f64>() - 1.0 / 12.0).abs() < 1.0e-14);
        assert!((velocity.iter().sum::<f64>() + 0.5).abs() < 1.0e-14);
        // Applied to a(n-j)=j: nabla a=-1, higher differences vanish.
        let p_linear: f64 = position
            .iter()
            .enumerate()
            .map(|(j, w)| *w * j as f64)
            .sum();
        let v_linear: f64 = velocity
            .iter()
            .enumerate()
            .map(|(j, w)| *w * j as f64)
            .sum();
        assert!(p_linear.abs() < 1.0e-13);
        assert!((v_linear - 1.0 / 12.0).abs() < 1.0e-13);
        let startup = IntegrationConfiguration::new(
            Length::new::<meter>(1.0e-7),
            Velocity::new::<meter_per_second>(1.0e-10),
            Ratio::new::<ratio>(1.0e-15),
            Duration::from_seconds(1.0e-8),
            Duration::from_seconds(1.0),
            Duration::from_seconds(0.1),
            100_000,
            10_000,
        )
        .unwrap();
        for (step, p, v, iterations, steps, expected) in [
            (
                0.0,
                1.0,
                1.0,
                10,
                100,
                GaussJacksonConfigurationError::InvalidStep,
            ),
            (
                -1.0,
                1.0,
                1.0,
                10,
                100,
                GaussJacksonConfigurationError::InvalidStep,
            ),
            (
                1.0,
                f64::NAN,
                1.0,
                10,
                100,
                GaussJacksonConfigurationError::InvalidPositionTolerance,
            ),
            (
                1.0,
                1.0,
                f64::INFINITY,
                10,
                100,
                GaussJacksonConfigurationError::InvalidVelocityTolerance,
            ),
            (
                1.0,
                1.0,
                1.0,
                0,
                100,
                GaussJacksonConfigurationError::ZeroLimit,
            ),
            (
                1.0,
                1.0,
                1.0,
                10,
                0,
                GaussJacksonConfigurationError::ZeroLimit,
            ),
        ] {
            assert_eq!(
                GaussJacksonConfiguration::new(
                    Duration::from_seconds(step),
                    startup,
                    Length::new::<meter>(p),
                    Velocity::new::<meter_per_second>(v),
                    iterations,
                    steps,
                )
                .unwrap_err(),
                expected
            );
        }
    }
}
