use std::{error::Error, fmt};

use hifitime::Epoch;
use orskit_core::{Orbit, SpacecraftState};

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

    /// Propagates `initial` to `target` for `problem`.
    ///
    /// An absolute epoch makes the time scale and requested output instant
    /// explicit at the public boundary. Implementations may derive a duration
    /// internally from the initial epoch.
    fn propagate(
        &self,
        initial: Orbit<State>,
        problem: &Problem,
        target: Epoch,
    ) -> Result<Orbit<State>, Self::Error>;
}
