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

Configuration can be split across multiple files.

### Address records

```toml
[address]
a = true
aaaa = true
```

### Subdomains

```toml
[[subdomains]]
name = "www"
```

### Auxiliary records

```toml
[[txt]]
content = "v=spf1 include:example.com ~all"
```

### Provider configuration

FIXME
