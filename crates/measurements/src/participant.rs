//! Stable measurement-participant identities and ordered signal paths.

use std::{fmt, str::FromStr, sync::Arc};

use thiserror::Error;

/// Stable application-defined identity for a measurement participant.
///
/// The same identity can name a ground station, spacecraft, relay, or another
/// participant supplied by the application. Identities are preserved exactly;
/// leading or trailing whitespace and control characters are rejected rather
/// than normalized into a potentially different participant.
///
/// ```
/// use measurements::ParticipantId;
///
/// let station = ParticipantId::new("DSS-14")?;
/// assert_eq!(station.as_str(), "DSS-14");
/// # Ok::<(), measurements::ParticipantIdError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ParticipantId(Arc<str>);

impl ParticipantId {
    /// Constructs a non-empty participant identity.
    pub fn new(value: impl Into<String>) -> Result<Self, ParticipantIdError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ParticipantIdError::Empty);
        }
        if value.trim() != value {
            return Err(ParticipantIdError::SurroundingWhitespace);
        }
        if value.chars().any(char::is_control) {
            return Err(ParticipantIdError::ControlCharacter);
        }
        Ok(Self(Arc::from(value)))
    }

    /// Returns the application-defined identity text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ParticipantId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ParticipantId {
    type Err = ParticipantIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

/// An ordered sequence of participants visited by a measurement signal.
///
/// Entries represent signal events in order, not a set. For example, a
/// two-leg ground-space-ground observation is represented by
/// `[station, spacecraft, station]`. Adjacent duplicate participants are
/// rejected because they do not define a signal leg; a participant may recur
/// later in the path.
///
/// ```
/// use measurements::{ParticipantId, SignalPath};
///
/// let station = ParticipantId::new("DSS-14")?;
/// let spacecraft = ParticipantId::new("SC-01")?;
/// let path = SignalPath::new(vec![station.clone(), spacecraft, station])?;
/// assert_eq!(path.participant_count(), 3);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SignalPath(Arc<[ParticipantId]>);

impl SignalPath {
    /// Constructs a path containing at least one signal leg.
    pub fn new(participants: Vec<ParticipantId>) -> Result<Self, SignalPathError> {
        if participants.len() < 2 {
            return Err(SignalPathError::TooFewParticipants {
                actual: participants.len(),
            });
        }
        if let Some(index) = participants.windows(2).position(|pair| pair[0] == pair[1]) {
            return Err(SignalPathError::AdjacentDuplicate { index: index + 1 });
        }
        Ok(Self(Arc::from(participants)))
    }

    /// Returns the participants in signal-event order.
    #[must_use]
    pub fn participants(&self) -> &[ParticipantId] {
        &self.0
    }

    /// Returns the number of signal events in the path.
    #[must_use]
    pub fn participant_count(&self) -> usize {
        self.0.len()
    }

    /// Returns the participant at one signal-event index.
    #[must_use]
    pub fn participant(&self, index: usize) -> Option<&ParticipantId> {
        self.0.get(index)
    }
}

/// Invalid participant identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ParticipantIdError {
    /// The identity contains no non-whitespace characters.
    #[error("participant identifier must not be empty")]
    Empty,
    /// Trimming would change the identity.
    #[error("participant identifier must not have leading or trailing whitespace")]
    SurroundingWhitespace,
    /// The identity contains a control character.
    #[error("participant identifier must not contain control characters")]
    ControlCharacter,
}

/// Invalid ordered signal path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SignalPathError {
    /// Fewer than two signal events were supplied.
    #[error("a signal path needs at least two participants, but received {actual}")]
    TooFewParticipants {
        /// Number of supplied participants.
        actual: usize,
    },
    /// Two neighboring signal events name the same participant.
    #[error("signal-path participant at index {index} duplicates its predecessor")]
    AdjacentDuplicate {
        /// Index of the second duplicate participant.
        index: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: &str) -> ParticipantId {
        ParticipantId::new(value).expect("test participant identity is valid")
    }

    #[test]
    fn participant_identity_rejects_ambiguous_text() {
        assert_eq!(ParticipantId::new("   "), Err(ParticipantIdError::Empty));
        assert_eq!(
            ParticipantId::new(" DSS-14"),
            Err(ParticipantIdError::SurroundingWhitespace)
        );
        assert_eq!(
            ParticipantId::new("DSS-14\nSC-01"),
            Err(ParticipantIdError::ControlCharacter)
        );
    }

    #[test]
    fn path_requires_a_real_signal_leg() {
        assert_eq!(
            SignalPath::new(vec![id("DSS-14")]),
            Err(SignalPathError::TooFewParticipants { actual: 1 })
        );
        assert_eq!(
            SignalPath::new(vec![id("DSS-14"), id("DSS-14")]),
            Err(SignalPathError::AdjacentDuplicate { index: 1 })
        );
    }

    #[test]
    fn returning_path_may_revisit_a_non_adjacent_participant() {
        let station = id("DSS-14");
        let path = SignalPath::new(vec![station.clone(), id("SC-01"), station.clone()])
            .expect("two-leg returning path");

        assert_eq!(path.participant_count(), 3);
        assert_eq!(path.participant(0), Some(&station));
        assert_eq!(path.participant(2), Some(&station));
    }
}
