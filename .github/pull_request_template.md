## Outcome

<!-- What user-visible or maintainer-visible outcome does this change deliver? -->

## Evidence

<!-- Link tests, task briefs, ADRs, benchmarks, or rendered documentation as applicable. -->

## Checklist

- [ ] This change has one clear semantic outcome and avoids unrelated cleanup.
- [ ] I ran the relevant checks, or listed skipped checks and why above.
- [ ] I added tests or explained why this change cannot alter behavior.
- [ ] New public APIs and non-obvious workflows are documented.
- [ ] The contribution is original work or material already approved for reuse.
- [ ] If this changes a scientific model, format, dataset, or validation claim, I
      updated [provenance](../.agent/PROVENANCE.md) and cited the evidence.
- [ ] If this advances a capability, I updated the
      [parity ledger](../.agent/PARITY.md) without overstating its status.
- [ ] If this changes units, frames, epochs, time scales, tolerances, valid
      regimes, or errors, those contracts are explicit in code and docs.
- [ ] Binding impact is handled or explicitly deferred while the Rust core is
      stabilizing.

Documentation-only changes may mark scientific, parity, test, and binding
items not applicable; explain only when the reason is not evident from the
diff.
