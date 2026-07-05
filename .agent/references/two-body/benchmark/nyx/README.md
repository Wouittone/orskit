# Nyx black-box benchmark harness

This isolated harness is licensed under AGPL-3.0-or-later because it links the
unmodified `nyx-space` 2.3.1 crate. It is not a member of the orskit workspace,
not an orskit dependency, and not part of the MIT/Apache library distribution.
Default features are disabled so the premium feature is not enabled.
The direct `hyperdual` dependency is pinned to 1.4.0 to keep Nyx's public type
graph on its compatible `nalgebra` release.

The harness uses only Nyx's documented public `Orbit` construction and
two-body epoch-shift behavior. Nyx source, tests, examples, and internal design
must not be consulted or copied into this harness or orskit.

Nyx's license text is available from the
[GNU Affero General Public License](https://www.gnu.org/licenses/agpl-3.0.html).
