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
```

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
