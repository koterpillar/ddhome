use std::fmt;
use std::net::IpAddr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaaRecord {
    pub ca: String,
    pub wildcards: bool,
    pub account_uri: Option<String>,
}

impl CaaRecord {
    pub fn new(ca: &str, wildcards: bool, account_uri: Option<&str>) -> Self {
        Self {
            ca: ca.trim().to_ascii_lowercase(),
            wildcards,
            account_uri: account_uri
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_owned),
        }
    }

    pub fn to_dns_value(&self) -> String {
        let tag = if self.wildcards { "issuewild" } else { "issue" };
        let issuer = self.ca.trim().to_ascii_lowercase();

        if let Some(account_uri) = self.account_uri.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            format!("0 {tag} \"{issuer}; accounturi={account_uri}\"")
        } else {
            format!("0 {tag} \"{issuer}\"")
        }
    }

    pub fn parse_dns_value(value: &str) -> Option<Self> {
        let mut parts = value.split_whitespace();
        let flags = parts.next()?;
        let tag = parts.next()?;
        let issuer_and_params = parts.collect::<Vec<_>>().join(" ");

        if flags != "0" || issuer_and_params.is_empty() {
            return None;
        }

        let wildcards = if tag.eq_ignore_ascii_case("issue") {
            false
        } else if tag.eq_ignore_ascii_case("issuewild") {
            true
        } else {
            return None;
        };

        let issuer_and_params = issuer_and_params.trim();
        let issuer_and_params = if issuer_and_params.starts_with('"')
            && issuer_and_params.ends_with('"')
            && issuer_and_params.len() >= 2
        {
            &issuer_and_params[1..issuer_and_params.len() - 1]
        } else {
            issuer_and_params
        };

        let mut segments = issuer_and_params.split(';');
        let issuer = segments.next()?.trim();
        if issuer.is_empty() {
            return None;
        }

        let mut account_uri: Option<String> = None;

        for segment in segments {
            let segment = segment.trim();
            if segment.is_empty() {
                continue;
            }

            let (parameter, value) = segment.split_once('=')?;
            if !parameter.trim().eq_ignore_ascii_case("accounturi") {
                return None;
            }

            if account_uri.is_some() {
                return None;
            }

            let value = value.trim();
            let value = if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
                &value[1..value.len() - 1]
            } else {
                value
            };

            if value.is_empty() {
                return None;
            }

            account_uri = Some(value.to_owned());
        }

        Some(Self::new(issuer, wildcards, account_uri.as_deref()))
    }
}

impl fmt::Display for CaaRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dns_value())
    }
}

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

#[cfg(test)]
mod tests {
    use super::CaaRecord;

    #[test]
    fn caa_record_value_includes_account_uri_parameter() {
        let value = CaaRecord::new(
            "example.com",
            false,
            Some("https://example.com/acme/acct/123456"),
        )
        .to_dns_value();

        assert_eq!(
            value,
            "0 issue \"example.com; accounturi=https://example.com/acme/acct/123456\""
        );
    }
}
