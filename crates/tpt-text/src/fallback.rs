//! Caller-configured font fallback.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use unicode_script::Script;

/// Ordered fallback families, configured by the caller.
///
/// When the primary face lacks a character, the resolver tries, in order:
/// the chain for the character's script, then the default families. The
/// provider's own [`FontProvider::fallback_chain`](crate::FontProvider::fallback_chain)
/// is tried last. If nothing covers the character, the primary face is used
/// and the glyph renders as `.notdef`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FallbackResolver {
    chains: Vec<(Script, Vec<String>)>,
    defaults: Vec<String>,
}

impl FallbackResolver {
    /// Creates a resolver with no families configured.
    pub fn new() -> FallbackResolver {
        FallbackResolver::default()
    }

    /// Sets the chain for `script`, replacing any earlier one.
    pub fn with_script_chain(mut self, script: Script, families: &[&str]) -> FallbackResolver {
        let names = families.iter().map(|f| f.to_string()).collect();
        match self.chains.iter_mut().find(|(s, _)| *s == script) {
            Some(entry) => entry.1 = names,
            None => self.chains.push((script, names)),
        }
        self
    }

    /// Sets the families tried for any script after the script chain.
    pub fn with_defaults(mut self, families: &[&str]) -> FallbackResolver {
        self.defaults = families.iter().map(|f| f.to_string()).collect();
        self
    }

    /// Family names to try for `script`, in priority order.
    pub(crate) fn candidates(&self, script: Script) -> impl Iterator<Item = &str> {
        let chain = self
            .chains
            .iter()
            .find(|(s, _)| *s == script)
            .map(|(_, names)| names.as_slice())
            .unwrap_or(&[]);
        chain.iter().chain(self.defaults.iter()).map(String::as_str)
    }
}
