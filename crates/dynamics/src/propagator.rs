use std::{error::Error, fmt};

use core_crate::{Orbit, SpacecraftState};
use hifitime::{Duration, Epoch};

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
pub trait Propagator<Problem: ?Sized, State: SpacecraftState>: fmt::Debug + Send + Sync {
    /// Typed error returned by this problem/algorithm combination.
    type Error: Error + Send + Sync + 'static;

    /// Propagates `initial` by a signed duration for `problem`.
    fn propagate(
        &self,
        initial: Orbit<State>,
        problem: &Problem,
        duration: Duration,
    ) -> Result<Orbit<State>, Self::Error>;

    /// Propagates `initial` to an explicit target epoch for `problem`.
    fn propagate_to(
        &self,
        initial: Orbit<State>,
        problem: &Problem,
        target: Epoch,
    ) -> Result<Orbit<State>, Self::Error> {
        let duration = target - initial.epoch();
        self.propagate(initial, problem, duration)
    }
}
