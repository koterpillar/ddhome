pub mod caa;

use std::net::IpAddr;

pub use caa::CaaRecord;

/// A single actionable desired DNS condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Desire {
    /// Ensure a subdomain entry points back to the zone root.
    Subdomain { name: String },
    /// Ensure an A or AAAA record exists with the given IP value.
    Address { value: IpAddr },
    /// Ensure a TXT record exists with the given content.
    Txt { content: String },
    /// Ensure a CAA record exists for the given CA.
    Caa(CaaRecord),
}

/// A list of actionable desired DNS conditions.
pub type Desires = Vec<Desire>;
