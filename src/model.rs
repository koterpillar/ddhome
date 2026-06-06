use std::fmt;
use std::net::IpAddr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaaRecord {
    pub ca: String,
    pub wildcards: bool,
    pub account_uri: Option<String>,
    pub validation_methods: Option<Vec<String>>,
}

impl CaaRecord {
    pub fn new(
        ca: &str,
        wildcards: bool,
        account_uri: Option<&str>,
        validation_methods: Option<&[String]>,
    ) -> Self {
        Self {
            ca: ca.trim().to_ascii_lowercase(),
            wildcards,
            account_uri: account_uri
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_owned),
            validation_methods: validation_methods
                .map(|methods| {
                    methods
                        .iter()
                        .map(|method| method.trim().to_ascii_lowercase())
                        .filter(|method| !method.is_empty())
                        .collect::<Vec<_>>()
                })
                .filter(|methods| !methods.is_empty()),
        }
    }

    pub fn to_dns_value(&self) -> (String, String) {
        let tag = if self.wildcards { "issuewild" } else { "issue" };
        let issuer = self.ca.trim().to_ascii_lowercase();

        let mut value = issuer;

        if let Some(account_uri) = self
            .account_uri
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            value.push_str("; accounturi=");
            value.push_str(account_uri);
        }

        if let Some(validation_methods) = self.validation_methods.as_ref().filter(|m| !m.is_empty())
        {
            value.push_str("; validationmethods=");
            value.push_str(&validation_methods.join(","));
        }

        (tag.to_owned(), value)
    }

    pub fn parse_dns_value(tag: &str, value: &str) -> Option<Self> {
        let wildcards = if tag.eq_ignore_ascii_case("issue") {
            false
        } else if tag.eq_ignore_ascii_case("issuewild") {
            true
        } else {
            return None;
        };

        let value = value.trim();
        if value.is_empty() {
            return None;
        }

        let mut segments = value.split(';');
        let issuer = segments.next()?.trim();
        if issuer.is_empty() {
            return None;
        }

        let mut account_uri: Option<String> = None;
        let mut validation_methods: Option<Vec<String>> = None;

        for segment in segments {
            let segment = segment.trim();
            if segment.is_empty() {
                continue;
            }

            let (parameter, param_value) = segment.split_once('=')?;
            let parameter = parameter.trim();

            let param_value = param_value.trim();
            let param_value = if param_value.starts_with('"') && param_value.ends_with('"') && param_value.len() >= 2 {
                &param_value[1..param_value.len() - 1]
            } else {
                param_value
            };

            if param_value.is_empty() {
                return None;
            }

            if parameter.eq_ignore_ascii_case("accounturi") {
                if account_uri.is_some() {
                    return None;
                }

                account_uri = Some(param_value.to_owned());
            } else if parameter.eq_ignore_ascii_case("validationmethods") {
                if validation_methods.is_some() {
                    return None;
                }

                let methods = param_value
                    .split(',')
                    .map(|method| method.trim().to_ascii_lowercase())
                    .collect::<Vec<_>>();

                if methods.is_empty() || methods.iter().any(|method| method.is_empty()) {
                    return None;
                }

                validation_methods = Some(methods);
            } else {
                return None;
            }
        }

        Some(Self::new(
            issuer,
            wildcards,
            account_uri.as_deref(),
            validation_methods.as_deref(),
        ))
    }
}

impl fmt::Display for CaaRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (tag, value) = self.to_dns_value();
        write!(f, "0 {} \"{}\"", tag, value)
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
        let (tag, value) = CaaRecord::new(
            "example.com",
            false,
            Some("https://example.com/acme/acct/123456"),
            None,
        )
        .to_dns_value();

        assert_eq!(tag, "issue");
        assert_eq!(
            value,
            "example.com; accounturi=https://example.com/acme/acct/123456"
        );
    }

    #[test]
    fn caa_record_value_includes_validation_methods_parameter() {
        let methods = vec!["dns-01".to_owned(), "http-01".to_owned()];
        let (tag, value) = CaaRecord::new("example.com", false, None, Some(&methods)).to_dns_value();

        assert_eq!(tag, "issue");
        assert_eq!(
            value,
            "example.com; validationmethods=dns-01,http-01"
        );
    }

    #[test]
    fn parse_caa_value_with_validation_methods() {
        let parsed =
            CaaRecord::parse_dns_value("issue", "example.com; validationmethods=dns-01,http-01")
                .expect("expected CAA value to parse");

        assert_eq!(parsed.ca, "example.com");
        assert!(!parsed.wildcards);
        assert!(parsed.account_uri.is_none());
        assert_eq!(
            parsed.validation_methods,
            Some(vec!["dns-01".to_owned(), "http-01".to_owned()])
        );
    }

    #[test]
    fn parse_caa_value_with_account_uri_and_validation_methods() {
        let parsed = CaaRecord::parse_dns_value(
            "issue",
            "example.com; accounturi=https://example.com/acme/acct/123456; validationmethods=dns-01",
        )
        .expect("expected CAA value to parse");

        let (tag, value) = parsed.to_dns_value();
        assert_eq!(tag, "issue");
        assert_eq!(
            value,
            "example.com; accounturi=https://example.com/acme/acct/123456; validationmethods=dns-01"
        );
    }

    #[test]
    fn parse_caa_value_rejects_empty_validation_method() {
        let parsed = CaaRecord::parse_dns_value(
            "issue",
            "example.com; validationmethods=dns-01,,http-01",
        );

        assert!(parsed.is_none());
    }
}
