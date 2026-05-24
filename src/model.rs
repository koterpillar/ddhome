#![allow(dead_code)]

use std::net::IpAddr;

/// A single actionable desired DNS condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Desire {
    /// Ensure an A or AAAA record exists with the given IP value.
    Address {
        value: IpAddr,
    },
    /// Ensure a TXT record exists with the given content.
    Txt {
        content: String,
    },
}

/// A list of actionable desired DNS conditions.
pub type Desires = Vec<Desire>;
