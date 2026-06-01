# ddhome - maintain a home server DNS configuration

This tool maintains a home server DNS records written in a declarative way. It
points a domain name to a dynamic IP, adds subdomain CNAME records and handles
auxiliary records like TXT and CAA.

It is similar to `ddclient`, but supports more than just a dynamic A record.

## Status

Work in progress.

## Usage

The desired configuration is written in TOML files. When `ddhome` is run, it
compares the desired configuration with the actual DNS records and makes updates
as needed.

By default, `ddhome` runs in check mode: it reports mismatches but does not
change DNS records unless `--apply` is provided.

```shell
# Check desired state against DNS (default config path: /etc/ddhome)
ddhome

# Check with an explicit config path (file or directory)
ddhome ./test-config/all.toml

# Apply required DNS changes
ddhome --apply ./test-config/all.toml
```

Configuration can be split across multiple files.

### Address records

Set these to point the domain to the current IP address.

```toml
[address]
a = true
aaaa = true
```

### Subdomains

Specify subdomains to be created as CNAME records pointing to the main domain.

```toml
[[subdomain]]
name = "www"
```

### Auxiliary records

```toml
[[txt]]
content = "v=spf1 include:example.com ~all"

[[caa]]
ca = "example.com"
wildcards = false

[[caa]]
ca = "example.com"
wildcards = true

[[caa]]
ca = "example.com"
wildcards = false
account_uri = "https://example.com/acme/acct/123456"
```

`[[caa]]` adds root CAA records. Use `ca` to name the certificate authority and `wildcards` to choose between `issue` and `issuewild`. Set optional `account_uri` to include the RFC 8657 `accounturi` parameter for ACME account binding.

### Provider configuration

```toml
[bunny]
zone_id = 123456
```

`zone_id` is the Bunny DNS zone ID.

The Bunny API key is read from the environment:

```bash
read -s BUNNY_API_KEY
export BUNNY_API_KEY
```
