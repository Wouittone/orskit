# ADR-0003: own streaming CCSDS ingestion at the I/O edge

- Status: Accepted
- Date: 2026-07-04
- Owners: orskit maintainers
- Affected parity rows: I/O / CCSDS messages; orbits / Cartesian states;
  geometry / frames

## Context

orskit must ingest navigation messages that can exceed 100 MiB while keeping
scientific time, unit, state, and frame contracts explicit. The Rust ecosystem
has broad message-model crates, notably `ccsds-ndm` 0.0.9, and an ODM crate in
the Lox ecosystem, `lox-odm` 0.1.0-alpha.3. Their public parsing entry points
consume complete strings/files and their models own time, unit, and frame types.
Adapting a fully materialized message would retain both representations at the
peak and would not provide event-level backpressure. `lox-odm` is additionally
alpha, requires Rust 1.90, and intentionally uses the wider Lox type ecosystem.

The hot OEM workload is regular line-oriented KVN data. I/O is naturally
asynchronous, while decimal conversion across already-delimited state records
is data-parallel. Tokio and Rayon solve different parts of that workload and
must not be hidden behind one ambient runtime.

## Decision

1. Own CCSDS wire parsing in a dedicated `orskit-ccsds` edge crate; scientific
   domain crates never depend on message formats.
2. Implement OEM KVN as the first vertical slice. A state-machine decoder feeds
   both blocking `BufRead` and Tokio `AsyncBufRead` event readers with bounded
   working memory.
3. Provide a separate Rayon document parser for callers that already hold a
   complete message and want ordered parallel state conversion. Parallel
   collection preserves source order and reports source line numbers.
4. Convert immediately into Hifitime epochs, `orskit-units` quantities,
   `orskit-frames` identities, and orskit-owned `CartesianCoordinates`. Do not
   fabricate spacecraft mass, orientation, or inertia absent from OEM.
5. Keep Tokio and Rayon behind explicit `async` and `parallel` crate features.
6. Treat XML, covariance, OPM/OMM/OCM, attitude, and tracking messages as
   follow-on vertical slices sharing the same edge/domain boundary. Unsupported
   content returns an explicit typed error in this slice.

## Alternatives considered

- Depend directly on `ccsds-ndm`: its breadth and MPL-2.0 license are suitable,
  but its whole-input API and separate physical model do not meet bounded-memory
  streaming or peak-allocation goals. It remains useful as a future black-box
  differential oracle or optional compatibility adapter.
- Depend directly on `lox-odm`: its Rust-native ODM model is promising, but its
  alpha API, Rust 1.90 floor, and Lox frame/time types would make the I/O edge
  choose foundational types for orskit.
- Wrap either crate with Tokio `spawn_blocking`: this prevents executor stalls
  but does not make parsing streaming, bound peak memory, or expose backpressure.
- Parallelize file I/O itself: line scanning and structural validation are
  sequential and cheap. Rayon is applied only after record boundaries and
  segment context are known.

## Consequences

- The first slice has a small, auditable parser and maps directly to the domain
  model with no duplicate message tree on the streaming path.
- orskit owns conformance, fuzzing, and maintenance for supported wire formats.
- The initial feature status is honestly Partial: OEM KVN state records are
  supported; covariance and other CCSDS syntaxes/messages are not yet supported.
- Async streaming and parallel collection are complementary APIs rather than a
  claim that every source benefits from CPU parallelism.

## Validation

Mode-equivalence tests compare blocking, Tokio, and Rayon results in source
order. Malformed-input tests exercise allocation and line-context boundaries.
A reproducible Criterion benchmark generates approximately 100 MiB of OEM KVN
and records byte throughput for streaming and parallel collection separately.
The initial Windows/16-logical-processor baseline measured about 43 MiB/s for
streaming, 39 MiB/s for sequential collection, and 100 MiB/s for ordered Rayon
collection; see `.agent/benchmarks/2026-07-04-oem-kvn-100-mib.md` for method and
intervals.

## Provenance

- CCSDS 502.0-B-3, *Orbit Data Messages*, Issue 3, May 2023: public standard;
  syntax and semantic facts only.
- SANA Orbit Centers, Time Systems, and Celestial Body Reference Frames:
  public normative registries; identifiers only.
- `ccsds-ndm` 0.0.9 and `lox-odm` 0.1.0-alpha.3: public API/package metadata
  inspected solely for dependency evaluation; no source reused.
