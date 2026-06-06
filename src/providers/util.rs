use std::future::Future;

pub enum RecordResolution<T> {
    /// An existing record already matches the desired value.
    Match,
    /// No matching record exists and one should be created.
    Missing,
    /// A matching record exists but its content differs from the desired value.
    Mismatch(T),
}

impl<T> RecordResolution<T> {
    /// Resolves a desired value against an optional existing record.
    ///
    /// Returns:
    /// - `Missing` when no record exists
    /// - `Match` when the record exists and satisfies `matches_expected`
    /// - `Mismatch` when the record exists but differs from what is expected
    pub fn from_expected<F>(actual: Option<T>, matches_expected: F) -> Self
    where
        T: Clone,
        F: FnOnce(&T) -> bool,
    {
        match actual {
            None => Self::Missing,
            Some(actual) if matches_expected(&actual) => Self::Match,
            Some(actual) => Self::Mismatch(actual),
        }
    }

    pub fn is_match(&self) -> bool {
        matches!(self, Self::Match)
    }

    pub async fn correct<F, Fut, R>(self, default: R, action: F) -> R
    where
        F: FnOnce(Option<T>) -> Fut,
        Fut: Future<Output = R>,
    {
        match self {
            Self::Match => default,
            Self::Missing => action(None).await,
            Self::Mismatch(actual) => action(Some(actual)).await,
        }
    }
}
