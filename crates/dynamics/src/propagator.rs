use std::{error::Error, fmt};

use hifitime::Epoch;
use orskit_core::{Orbit, SpacecraftState};

/// Adapts one caller-selected state representation to the representation a
/// propagator resolves internally for a particular physical problem.
///
/// A propagator first calls [`Self::resolve`] with its explicit problem, then
/// advances the resulting [`Self::Resolved`] state, and finally calls
/// [`Self::restore`] before returning to the caller. This makes the resolution
/// boundary explicit and reusable across analytical, numerical, and future
/// propagation implementations without imposing a closed state enum.
pub trait PropagationState<Problem: ?Sized>: SpacecraftState {
    /// State representation directly advanced by this propagator/problem pair.
    type Resolved: SpacecraftState;
    /// Typed error raised while resolving or restoring a representation.
    type Error: Error + Send + Sync + 'static;

    /// Resolves this caller-selected state into the representation advanced by
    /// the propagator for `problem`.
    fn resolve(self, problem: &Problem) -> Result<Self::Resolved, Self::Error>;

    /// Restores a propagated resolved state to the caller-selected
    /// representation using the explicit problem context.
    ///
    /// This permits restoration into representations that require problem-owned
    /// data, such as a central-gravity provider, without introducing ambient
    /// conversion state.
    fn restore(resolved: Self::Resolved, problem: &Problem) -> Result<Self, Self::Error>;
}

/// Propagates an epoch-qualified orbit.
///
/// `Problem` describes the physical problem independently of the algorithm
/// used to solve it. Multiple propagator implementations may therefore solve
/// the same compatible problem without encoding body topology in their type
/// names or configuration.
///
/// The selected state representation is preserved. This translational contract
/// deliberately excludes mass, inertia, and attitude: callers compose those
/// independently when constructing a complete spacecraft view.
pub trait Propagator<Problem: ?Sized, State>: fmt::Debug + Send + Sync
where
    State: PropagationState<Problem>,
    Self::Error: From<State::Error>,
{
    /// Typed error returned by this problem/algorithm combination.
    type Error: Error + Send + Sync + 'static;

    /// Propagates a resolved state to `target` for `problem`.
    ///
    /// Implementations advance only the state representation declared by
    /// [`PropagationState::Resolved`]. The default [`Self::propagate`] method
    /// resolves and restores every caller-selected representation around this
    /// method.
    fn propagate_resolved(
        &self,
        initial: Orbit<State::Resolved>,
        problem: &Problem,
        target: Epoch,
    ) -> Result<Orbit<State::Resolved>, Self::Error>;

    /// Resolves, propagates, and restores `initial` to `target` for `problem`.
    ///
    /// An absolute epoch makes the time scale and requested output instant
    /// explicit at the public boundary. Implementations may derive a duration
    /// internally from the initial epoch.
    fn propagate(
        &self,
        initial: Orbit<State>,
        problem: &Problem,
        target: Epoch,
    ) -> Result<Orbit<State>, Self::Error> {
        let epoch = initial.epoch();
        let state = initial.into_state();
        let resolved = state.clone().resolve(problem).map_err(Self::Error::from)?;

        if target == epoch {
            return Ok(Orbit::new(target, state));
        }

        let propagated = self.propagate_resolved(Orbit::new(epoch, resolved), problem, target)?;
        let state = State::restore(propagated.into_state(), problem).map_err(Self::Error::from)?;
        Ok(Orbit::new(target, state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frames::ReferenceFrame;
    use thiserror::Error;

    #[derive(Debug, Clone, PartialEq)]
    struct UserState(i32);

    impl SpacecraftState for UserState {
        fn frame(&self) -> ReferenceFrame {
            ReferenceFrame::GCRF
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct ResolvedState(i32);

    impl SpacecraftState for ResolvedState {
        fn frame(&self) -> ReferenceFrame {
            ReferenceFrame::GCRF
        }
    }

    #[derive(Debug)]
    struct TestProblem {
        resolution_offset: i32,
        propagation_offset: i32,
    }

    #[derive(Debug, Error)]
    #[error("test propagation failure")]
    struct TestError;

    impl PropagationState<TestProblem> for UserState {
        type Resolved = ResolvedState;
        type Error = TestError;

        fn resolve(self, problem: &TestProblem) -> Result<Self::Resolved, Self::Error> {
            Ok(ResolvedState(self.0 + problem.resolution_offset))
        }

        fn restore(resolved: Self::Resolved, _problem: &TestProblem) -> Result<Self, Self::Error> {
            Ok(Self(resolved.0))
        }
    }

    #[derive(Debug)]
    struct TestPropagator;

    impl Propagator<TestProblem, UserState> for TestPropagator {
        type Error = TestError;

        fn propagate_resolved(
            &self,
            initial: Orbit<ResolvedState>,
            problem: &TestProblem,
            target: Epoch,
        ) -> Result<Orbit<ResolvedState>, Self::Error> {
            Ok(Orbit::new(
                target,
                ResolvedState(initial.into_state().0 + problem.propagation_offset),
            ))
        }
    }

    #[test]
    fn default_propagation_resolves_and_restores_the_caller_state() {
        let initial_epoch = Epoch::from_tai_seconds(100.0);
        let target_epoch = Epoch::from_tai_seconds(250.0);
        let problem = TestProblem {
            resolution_offset: 10,
            propagation_offset: 5,
        };

        let propagated = TestPropagator
            .propagate(
                Orbit::new(initial_epoch, UserState(2)),
                &problem,
                target_epoch,
            )
            .expect("test propagation succeeds");

        assert_eq!(propagated.epoch(), target_epoch);
        assert_eq!(propagated.state(), UserState(17));
    }
}
