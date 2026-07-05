use std::{error::Error, fmt};

use hifitime::{Duration, Epoch};
use orskit_core::SpacecraftView;

use crate::ForceModel;

/// Propagates an epoch-specific view of a time-independent spacecraft.
///
/// The spacecraft identity and geometry remain borrowed and unchanged. The
/// orbital-state enum preserves its native variant; translational propagators
/// advance orbit and epoch while preserving mass, inertia, and attitude.
pub trait Propagator<M>: fmt::Debug + Send + Sync
where
    M: ForceModel + ?Sized,
{
    /// Typed error returned by this model/algorithm combination.
    type Error: Error + Send + Sync + 'static;

    /// Propagates `initial` by a signed duration using `model`.
    fn propagate<'a>(
        &self,
        initial: &SpacecraftView<'a>,
        model: &M,
        duration: Duration,
    ) -> Result<SpacecraftView<'a>, Self::Error>;

    /// Propagates `initial` to an explicit target epoch using `model`.
    fn propagate_to<'a>(
        &self,
        initial: &SpacecraftView<'a>,
        model: &M,
        target: Epoch,
    ) -> Result<SpacecraftView<'a>, Self::Error> {
        self.propagate(initial, model, target - initial.epoch())
    }
}
