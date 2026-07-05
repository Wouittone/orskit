use std::{error::Error, fmt};

use hifitime::{Duration, Epoch};
use orskit_core::Orbit;

use crate::ForceModel;

/// Propagates an epoch-qualified orbit.
///
/// The orbital-state enum preserves its native variant. This translational
/// contract deliberately excludes mass, inertia, and attitude: callers must
/// compose those independently when constructing a complete spacecraft view.
pub trait Propagator<M>: fmt::Debug + Send + Sync
where
    M: ForceModel + ?Sized,
{
    /// Typed error returned by this model/algorithm combination.
    type Error: Error + Send + Sync + 'static;

    /// Propagates `initial` by a signed duration using `model`.
    fn propagate(
        &self,
        initial: Orbit,
        model: &M,
        duration: Duration,
    ) -> Result<Orbit, Self::Error>;

    /// Propagates `initial` to an explicit target epoch using `model`.
    fn propagate_to(&self, initial: Orbit, model: &M, target: Epoch) -> Result<Orbit, Self::Error> {
        self.propagate(initial, model, target - initial.epoch())
    }
}
