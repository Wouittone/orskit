use std::{error::Error, fmt};

use hifitime::Epoch;
use orskit_core::{Orbit, SpacecraftState};

/// Adapts a caller-selected state representation to the representation a
/// concrete propagator advances for its owned physical problem.
///
/// The problem is deliberately not a parameter of [`Propagator`]. A concrete
/// propagator owns it at construction; state implementations can use this
/// trait when they need that same problem to resolve or restore their domain
/// representation.
pub trait PropagationState<Problem: ?Sized>: SpacecraftState + Sized {
    /// State representation directly advanced by this propagator/problem pair.
    type Resolved: SpacecraftState;
    /// Typed error raised while resolving or restoring a representation.
    type Error: Error + Send + Sync + 'static;

    /// Validates this state against the propagator-owned problem.
    fn validate(&self, problem: &Problem) -> Result<(), Self::Error>;

    /// Resolves this state into the representation advanced by the propagator.
    fn resolve(self, problem: &Problem) -> Result<Self::Resolved, Self::Error>;

    /// Restores a propagated resolved state to the requested representation.
    fn restore(resolved: Self::Resolved, problem: &Problem) -> Result<Self, Self::Error>;
}

/// Propagates an epoch-qualified orbit using the physical problem owned by
/// this value.
///
/// The concrete propagator selects both the physical problem and the numerical
/// or analytical method. Consequently an estimator cannot accidentally supply
/// a different problem on a later call. The selected state representation is
/// preserved; callers compose mass, inertia, and attitude separately when
/// constructing a complete spacecraft view.
///
/// ```
/// use std::convert::Infallible;
///
/// use dynamics_core::Propagator;
/// use frames::ReferenceFrame;
/// use hifitime::Epoch;
/// use orskit_core::{Orbit, SpacecraftState};
///
/// #[derive(Debug)]
/// struct State(ReferenceFrame);
///
/// impl SpacecraftState for State {
///     fn frame(&self) -> ReferenceFrame {
///         self.0
///     }
/// }
///
/// #[derive(Debug)]
/// struct HoldStatePropagator;
///
/// impl Propagator<State> for HoldStatePropagator {
///     type Error = Infallible;
///
///     fn propagate(
///         &self,
///         initial: Orbit<State>,
///         target: Epoch,
///     ) -> Result<Orbit<State>, Self::Error> {
///         Ok(Orbit::new(target, State(initial.as_ref().frame())))
///     }
/// }
///
/// let target = Epoch::from_gregorian_tai_at_midnight(2026, 1, 2);
/// let result = HoldStatePropagator.propagate(
///     Orbit::new(
///         Epoch::from_gregorian_tai_at_midnight(2026, 1, 1),
///         State(ReferenceFrame::GCRF),
///     ),
///     target,
/// )?;
/// assert_eq!(result.epoch(), target);
/// # Ok::<(), Infallible>(())
/// ```
pub trait Propagator<State: SpacecraftState>: fmt::Debug + Send + Sync {
    /// Typed error returned by this propagator.
    type Error: Error + Send + Sync + 'static;

    /// Advances `initial` to the absolute `target` epoch.
    fn propagate(&self, initial: Orbit<State>, target: Epoch) -> Result<Orbit<State>, Self::Error>;
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

    #[derive(Debug)]
    struct TestProblem {
        resolution_offset: i32,
        propagation_offset: i32,
    }

    #[derive(Debug, Error)]
    #[error("test propagation failure")]
    struct TestError;

    #[derive(Debug)]
    struct TestPropagator(TestProblem);

    impl Propagator<UserState> for TestPropagator {
        type Error = TestError;

        fn propagate(
            &self,
            initial: Orbit<UserState>,
            target: Epoch,
        ) -> Result<Orbit<UserState>, Self::Error> {
            Ok(Orbit::new(
                target,
                UserState(
                    initial.as_ref().0 + self.0.resolution_offset + self.0.propagation_offset,
                ),
            ))
        }
    }

    #[test]
    fn propagator_owns_its_problem() {
        let propagator = TestPropagator(TestProblem {
            resolution_offset: 10,
            propagation_offset: 5,
        });

        let propagated = propagator
            .propagate(
                Orbit::new(Epoch::from_tai_seconds(100.0), UserState(2)),
                Epoch::from_tai_seconds(250.0),
            )
            .expect("test propagation succeeds");

        assert_eq!(propagated.epoch(), Epoch::from_tai_seconds(250.0));
        assert_eq!(propagated.as_ref(), &UserState(17));
    }
}
