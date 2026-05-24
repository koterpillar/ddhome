#![allow(dead_code)]

use crate::model::{Desire, Desires};

pub type Evaluation<'a> = (&'a Desire, Result<(), String>);

pub trait Provider {
    /// Returns Ok when provider state already satisfies the desire,
    /// otherwise returns a human-readable mismatch explanation.
    async fn evaluate(&self, desire: &Desire) -> Result<(), String>;

    async fn evaluate_desires<'a>(&'a self, desires: &'a Desires) -> Vec<Evaluation<'a>>
    where
        Self: Sync,
    {
        let mut evaluations = Vec::with_capacity(desires.len());
        for desire in desires {
            evaluations.push((desire, self.evaluate(desire).await));
        }

        evaluations
    }

    /// Applies the desired state to the provider.
    async fn apply(&self, desire: &Desire) -> Result<(), String>;
}
