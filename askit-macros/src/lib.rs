//! Procedural macros for agent-stream-kit.
//!
//! Currently a placeholder that simply returns the decorated item unchanged.

use proc_macro::TokenStream;

/// Identity attribute macro placeholder for future ASKit macros.
#[proc_macro_attribute]
pub fn askit(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
