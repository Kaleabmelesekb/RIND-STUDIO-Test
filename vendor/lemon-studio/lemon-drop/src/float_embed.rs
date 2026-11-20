// Honeydew (vendored addition): Lemon Drop built into Honeydew has no floating display window; its display is the rack
// card (`os_plugkit::embed`). `float::close` is all the plugin's `Drop` asks of the module.

use crate::Shared;

/// Nothing floats: there is nothing to close.
pub fn close(_: &Shared) {}
