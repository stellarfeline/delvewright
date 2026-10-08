"""The ONE reading of a Rust item's visibility, for every gate that reads Rust
source as text.

An item header opens with `pub`, `pub(crate)`, `pub(super)`, `pub(in …)` or no
visibility at all. A gate that names an item by matching its header owes all
five: a split that moves a private item into a submodule makes it `pub(super)`
or `pub(in …)`, and a pattern that lists the visibilities it has seen loses the
item without a word. `VISIBILITY` is that rule as a regex fragment, optional and
followed by its whitespace, to be placed directly before the item keyword.
"""

#: An optional visibility and the whitespace after it: `pub `, `pub(crate) `,
#: `pub(super) `, `pub(in crate::a) `, or nothing.
VISIBILITY = r"(?:pub(?:\([^)]*\))?\s+)?"
