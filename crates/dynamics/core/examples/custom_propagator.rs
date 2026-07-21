//! Minimal custom `Propagator` and `PropagationState` pair.

use dynamics_core::{PropagationState, Propagator};
use frames::ReferenceFrame;
use hifitime::Epoch;
use orskit_core::{Orbit, OrbitParts, SpacecraftState};
use thiserror::Error;
use units::{Position, VelocityVector};

#[derive(Debug, Clone)]
struct ApplicationState {
    frame: ReferenceFrame,
    position: Position,
}

impl SpacecraftState for ApplicationState {
    fn frame(&self) -> ReferenceFrame {
        self.frame
    }
}

#[derive(Debug)]
struct ResolvedState {
    frame: ReferenceFrame,
    position_metres: [f64; 3],
}

impl SpacecraftState for ResolvedState {
    fn frame(&self) -> ReferenceFrame {
        self.frame
    }
}

#[derive(Debug)]
struct LinearMotion {
    velocity: VelocityVector,
}

#[derive(Debug, Error)]
enum LinearPropagationError {
    #[error("state and model values must be finite")]
    NonFinite,
}

impl PropagationState<LinearMotion> for ApplicationState {
    type Resolved = ResolvedState;
    type Error = LinearPropagationError;

    fn validate(&self, problem: &LinearMotion) -> Result<(), Self::Error> {
        if self.position.is_finite() && problem.velocity.is_finite() {
            Ok(())
        } else {
            Err(LinearPropagationError::NonFinite)
        }
    }

    fn resolve(self, problem: &LinearMotion) -> Result<Self::Resolved, Self::Error> {
        self.validate(problem)?;
        Ok(ResolvedState {
            frame: self.frame,
            position_metres: self.position.to_metres(),
        })
    }

    fn restore(resolved: Self::Resolved, _problem: &LinearMotion) -> Result<Self, Self::Error> {
        Ok(Self {
            frame: resolved.frame,
            position: Position::from_metres(
                resolved.position_metres[0],
                resolved.position_metres[1],
                resolved.position_metres[2],
            ),
        })
    }
}

#[derive(Debug)]
struct LinearPropagator {
    problem: LinearMotion,
}

impl Propagator<ApplicationState> for LinearPropagator {
    type Error = LinearPropagationError;

    fn propagate(
        &self,
        initial: Orbit<ApplicationState>,
        target: Epoch,
    ) -> Result<Orbit<ApplicationState>, Self::Error> {
        let OrbitParts { epoch, state } = initial.into();
        let mut resolved = state.resolve(&self.problem)?;
        let elapsed_seconds = (target - epoch).to_seconds();
        let velocity = self.problem.velocity.to_metres_per_second();
        for (position, speed) in resolved.position_metres.iter_mut().zip(velocity) {
            *position += speed * elapsed_seconds;
        }
        let restored = ApplicationState::restore(resolved, &self.problem)?;
        Ok(Orbit::new(target, restored))
    }
}

fn main() -> Result<(), LinearPropagationError> {
    let propagator = LinearPropagator {
        problem: LinearMotion {
            velocity: VelocityVector::from_metres_per_second(1.0, -2.0, 0.5),
        },
    };
    let initial = Orbit::new(
        Epoch::from_tai_seconds(10.0),
        ApplicationState {
            frame: ReferenceFrame::GCRF,
            position: Position::from_metres(100.0, 200.0, 300.0),
        },
    );
    let propagated = propagator.propagate(initial, Epoch::from_tai_seconds(20.0))?;
    assert_eq!(
        propagated.as_ref().position.to_metres(),
        [110.0, 180.0, 305.0]
    );
    Ok(())
}
