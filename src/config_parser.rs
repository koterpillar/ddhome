use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;

fn parse_config(input: &str) -> Result<Config, String> {
    toml::from_str(input).map_err(|e| format!("invalid TOML: {e}"))
}

pub fn parse_config_file(path: &str) -> Result<Config, String> {
    let input =
        fs::read_to_string(path).map_err(|e| format!("failed to read config file {path}: {e}"))?;
    parse_config(&input).map_err(|e| format!("failed to parse config file {path}: {e}"))
}

fn merge_config(into: &mut Config, next: Config, source: &Path) -> Result<(), String> {
    if into.bunny.is_some() && next.bunny.is_some() {
        return Err(format!(
            "duplicate [bunny] section found while merging {}",
            source.display()
        ));
    }

    if into.address.is_some() && next.address.is_some() {
        return Err(format!(
            "duplicate [address] section found while merging {}",
            source.display()
        ));
    }

    if into.bunny.is_none() {
        into.bunny = next.bunny;
    }

    if into.address.is_none() {
        into.address = next.address;
    }

    into.subdomain.extend(next.subdomain);
    into.txt.extend(next.txt);
    into.caa.extend(next.caa);
    Ok(())
}

pub fn parse_config_dir(path: &str) -> Result<Config, String> {
    let dir = Path::new(path);
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("failed to read config directory {}: {e}", dir.display()))?;

    let mut files: Vec<PathBuf> = entries
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("failed to read entry in {}: {e}", dir.display()))?
        .into_iter()
        .filter(|p| p.is_file())
        .filter(|p| {
            p.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("toml"))
                .unwrap_or(false)
        })
        .collect();

    files.sort();

    if files.is_empty() {
        return Err(format!("no .toml files found in {}", dir.display()));
    }

    let mut merged = Config::default();
    for file in files {
        let file_path = file.to_string_lossy();
        let cfg = parse_config_file(&file_path)?;
        merge_config(&mut merged, cfg, &file)?;
    }

    Ok(merged)
}

pub fn parse_config_path(path: &str) -> Result<Config, String> {
    let meta = fs::metadata(path).map_err(|e| format!("failed to stat config path {path}: {e}"))?;
    if meta.is_dir() {
        parse_config_dir(path)
    } else {
        parse_config_file(path)
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::parse_config_dir;

    fn make_temp_dir() -> PathBuf {
        let unique = format!(
            "ddhome-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time is before unix epoch")
                .as_nanos()
        );
        let dir = env::temp_dir().join(unique);
        fs::create_dir_all(&dir).expect("failed to create temporary test directory");
        dir
    }

    #[test]
    fn parse_config_dir_merges_multiple_toml_files_happy_path() {
        let dir = make_temp_dir();

        let first = dir.join("10-address.toml");
        let second = dir.join("20-subdomain.toml");
        let third = dir.join("30-records.toml");
        let fourth = dir.join("40-caa.toml");

        fs::write(
            &first,
            r#"[address]
a = true
aaaa = false
"#,
        )
        .expect("failed to write first config file");

        fs::write(
            &second,
            r#"[[subdomain]]
name = "www"
    "#,
        )
        .expect("failed to write second config file");

        fs::write(
            &third,
            r#"[[subdomain]]
    name = "api"

[[txt]]
content = "v=spf1 include:example.com ~all"
"#,
        )
        .expect("failed to write third config file");

        fs::write(
            &fourth,
            r#"[[caa]]
    ca = "example.com"
    wildcards = true

[[caa]]
    ca = "example.com"
    wildcards = false
    account_uri = "https://example.com/acme/acct/123456"
    "#,
        )
        .expect("failed to write fourth config file");

        let cfg = parse_config_dir(dir.to_str().expect("invalid temp dir path"))
            .expect("expected directory config parsing to succeed");

        let address = cfg.address.expect("expected merged address section");
        assert!(address.a);
        assert!(!address.aaaa);
        assert_eq!(cfg.subdomain.len(), 2);
        assert_eq!(cfg.subdomain[0].name, "www");
        assert_eq!(cfg.subdomain[1].name, "api");
        assert_eq!(cfg.txt.len(), 1);
        assert_eq!(cfg.txt[0].content, "v=spf1 include:example.com ~all");
        assert_eq!(cfg.caa.len(), 2);
        assert_eq!(cfg.caa[0].ca, "example.com");
        assert!(cfg.caa[0].wildcards);
        assert!(cfg.caa[0].account_uri.is_none());
        assert_eq!(cfg.caa[1].ca, "example.com");
        assert!(!cfg.caa[1].wildcards);
        assert_eq!(
            cfg.caa[1].account_uri.as_deref(),
            Some("https://example.com/acme/acct/123456")
        );

        fs::remove_dir_all(&dir).expect("failed to remove temporary test directory");
    }
}
