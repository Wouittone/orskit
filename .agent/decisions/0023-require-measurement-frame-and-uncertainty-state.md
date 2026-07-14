# ADR-0023: require measurement frame and uncertainty state

- Status: Accepted
- Date: 2026-07-13
- Affected parity rows: range observations

## Context

Qualified range observations still omitted the frame in which their modeling
geometry is interpreted and required a numerical uncertainty even when a
producer did not know one. Callers could therefore omit critical context or
invent a placeholder uncertainty.

## Decision

1. Every `RangeMeasurement` explicitly carries a `ReferenceFrame`.
2. Range and supplied uncertainty remain typed `Length` quantities.
3. Uncertainty is an explicit `Option<Length>`: `Some` must be positive and
   finite; `None` means unknown/not supplied and is distinct value identity.
4. No default frame or uncertainty is inferred.

## Consequences

- Correct, explicit construction is shorter than fabricating hidden defaults.
- Future correction/model APIs can validate their frame needs against the
  observation rather than relying on ambient convention.

## Validation

Tests prove frame and uncertainty-state identity, retain explicit `None`, and
reject zero, negative, and non-finite supplied uncertainty.

## Provenance

This is original orskit measurement-boundary design. No external implementation
or tests were consulted.
