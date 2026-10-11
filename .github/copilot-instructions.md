For pull request reviews, use the configured Rust Analyzer MCP server for
symbol-aware navigation, references, diagnostics, and macro-aware analysis. Use
the docs.rs MCP server when an external crate API or Cargo behavior is relevant.
Use the GitHub MCP server for pull request and workflow context.

Treat numerical and astrodynamics correctness as a review priority: preserve
physical units, reference-frame assumptions, orbital invariants, integration
tolerance behavior, estimation covariance semantics, parsing compatibility, and
floating-point stability. Check Rust 1.96.1 compatibility, all applicable
feature sets, public API documentation, and workspace-wide effects for changes
to shared crates.

When the user authorizes bot publication, use `scripts\publish_bot.ps1` as
documented in `docs\guides\bot-publishing.md`. Keep the app's human login and
review credentials unchanged. Never persist installation tokens, change global
Git identity, or fall back to personal credentials after a bot-auth failure.
Native app PR creation does not use this publisher's bot credentials; explain
that distinction before publishing. Do not commit or push without user approval.
