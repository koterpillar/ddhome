#![allow(dead_code)]

use std::future::Future;

use crate::model::Desire;

pub trait Provider {
    /// Returns Ok when provider state already satisfies the desire,
    /// otherwise returns a human-readable mismatch explanation.
    fn evaluate<'a>(
        &'a self,
        desire: &'a Desire,
    ) -> impl Future<Output = Result<(), String>> + Send + 'a;

    /// Applies the desired state to the provider.
    fn apply<'a>(
        &'a self,
        desire: &'a Desire,
    ) -> impl Future<Output = Result<(), String>> + Send + 'a;
}
