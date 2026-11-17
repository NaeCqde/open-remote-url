//! Decides, per http/https URL, whether the Sender forwards it to the Receiver or opens it in the
//! local (real default) browser.
//!
//! `DOMAINS` is a comma separated list of patterns:
//! - `www.example.com`   exactly this host (a subdomain is written in full)
//! - `*.example.com`     any subdomain, at any depth; NOT `example.com` itself (list both to cover both)
//! - `192.168.1.10`      an IP address (IPv6 as `::1` or `[::1]`)
//! - `10.0.0.0/8`        an IP range (CIDR)
//!
//! `DOMAIN_MODE`:
//! - `exclude` (default): a listed host is opened locally; everything else is sent. An empty list sends everything.
//! - `include`: only a listed host is sent; everything else is opened locally.
//!
//! Only http/https URLs are judged. Any other scheme is always sent.

use std::net::IpAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainMode {
    /// Listed hosts are sent; the rest is opened locally.
    Include,
    /// Listed hosts are opened locally; the rest is sent.
    Exclude,
}

impl DomainMode {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "include" | "send" | "allow" | "whitelist" => DomainMode::Include,
            _ => DomainMode::Exclude,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Pattern {
    Exact(String),
    /// `*.example.com` → stores `.example.com`
    Subdomain(String),
    Ip(IpAddr),
    Cidr(IpAddr, u8),
}

#[derive(Debug, Clone)]
pub struct DomainRules {
    mode: DomainMode,
    patterns: Vec<Pattern>,
}

impl DomainRules {
    pub fn new(mode: DomainMode, domains: &[String]) -> Self {
        let patterns = domains.iter().filter_map(|d| parse_pattern(d)).collect();
        Self { mode, patterns }
    }

    pub fn mode(&self) -> DomainMode {
        self.mode
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// `true`: forward to the Receiver. `false`: open in the local browser.
    pub fn should_send(&self, url: &str) -> bool {
        let Some(host) = http_host(url) else {
            // Not http/https (or unparsable): never filtered.
            return true;
        };
        if self.patterns.is_empty() {
            // exclude + nothing listed = send everything. include + nothing listed = nothing is sent.
            return self.mode == DomainMode::Exclude;
        }
        let listed = self.patterns.iter().any(|p| matches(p, &host));
        match self.mode {
            DomainMode::Include => listed,
            DomainMode::Exclude => !listed,
        }
    }
}

fn parse_pattern(raw: &str) -> Option<Pattern> {
    let s = raw.trim().trim_end_matches('.').to_lowercase();
    if s.is_empty() {
        return None;
    }
    if let Some((ip, bits)) = s.split_once('/') {
        let ip: IpAddr = ip.trim_matches(|c| c == '[' || c == ']').parse().ok()?;
        let bits: u8 = bits.parse().ok()?;
        let max = if ip.is_ipv4() { 32 } else { 128 };
        return (bits <= max).then_some(Pattern::Cidr(ip, bits));
    }
    if let Ok(ip) = s.trim_matches(|c| c == '[' || c == ']').parse::<IpAddr>() {
        return Some(Pattern::Ip(ip));
    }
    if let Some(rest) = s.strip_prefix("*.") {
        return (!rest.is_empty() && !rest.contains('*')).then(|| Pattern::Subdomain(format!(".{rest}")));
    }
    if s.contains('*') {
        // Only a leading `*.` is supported; anything else is a typo, not a silent match-all.
        log::warn!("DOMAINS: ignoring unsupported pattern '{}' (use '*.example.com')", raw.trim());
        return None;
    }
    Some(Pattern::Exact(s))
}

fn matches(p: &Pattern, host: &Host) -> bool {
    match (p, host) {
        (Pattern::Exact(e), Host::Name(h)) => e == h,
        (Pattern::Subdomain(suffix), Host::Name(h)) => h.ends_with(suffix.as_str()),
        (Pattern::Ip(a), Host::Ip(b)) => a == b,
        (Pattern::Cidr(net, bits), Host::Ip(ip)) => in_cidr(*net, *bits, *ip),
        _ => false,
    }
}

fn in_cidr(net: IpAddr, bits: u8, ip: IpAddr) -> bool {
    match (net, ip) {
        (IpAddr::V4(n), IpAddr::V4(i)) => {
            let mask = if bits == 0 { 0 } else { u32::MAX << (32 - bits as u32) };
            u32::from(n) & mask == u32::from(i) & mask
        }
        (IpAddr::V6(n), IpAddr::V6(i)) => {
            let mask = if bits == 0 { 0 } else { u128::MAX << (128 - bits as u32) };
            u128::from(n) & mask == u128::from(i) & mask
        }
        _ => false,
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Host {
    Name(String),
    Ip(IpAddr),
}

/// The host of an http/https URL, or `None` for any other scheme or an unparsable URL.
fn http_host(url: &str) -> Option<Host> {
    let url = url.trim();
    let (scheme, rest) = url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let authority = rest.split(|c| matches!(c, '/' | '?' | '#' | '\\')).next()?;
    // user:pass@host:port — the host is after the LAST '@'
    let hostport = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = if let Some(inner) = hostport.strip_prefix('[') {
        inner.split(']').next()?.to_string()
    } else {
        hostport.split(':').next()?.to_string()
    };
    let host = host.trim_end_matches('.').to_lowercase();
    if host.is_empty() {
        return None;
    }
    // IPv6 may carry a zone id (fe80::1%en0)
    let ip_candidate = host.split('%').next().unwrap_or(&host);
    Some(match ip_candidate.parse::<IpAddr>() {
        Ok(ip) => Host::Ip(ip),
        Err(_) => Host::Name(host),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(mode: DomainMode, list: &[&str]) -> DomainRules {
        let v: Vec<String> = list.iter().map(|s| s.to_string()).collect();
        DomainRules::new(mode, &v)
    }

    #[test]
    fn empty_exclude_sends_everything() {
        let r = rules(DomainMode::Exclude, &[]);
        assert!(r.should_send("https://example.com/"));
        assert!(r.should_send("http://127.0.0.1:3000/"));
    }

    #[test]
    fn empty_include_sends_nothing_but_non_http() {
        let r = rules(DomainMode::Include, &[]);
        assert!(!r.should_send("https://example.com/"));
        assert!(r.should_send("steam://run/1"));
    }

    #[test]
    fn exclude_exact_and_wildcard() {
        let r = rules(DomainMode::Exclude, &["intranet.example.com", "*.corp.test"]);
        assert!(!r.should_send("https://intranet.example.com/a?b=c"));
        assert!(r.should_send("https://example.com/"));
        assert!(r.should_send("https://x.intranet.example.com/")); // exact does not cover subdomains
        assert!(!r.should_send("https://a.corp.test/"));
        assert!(!r.should_send("https://a.b.corp.test/"));
        assert!(r.should_send("https://corp.test/")); // wildcard does not cover the apex
        assert!(r.should_send("https://evilcorp.test/")); // not a suffix match without the dot
    }

    #[test]
    fn include_mode_is_the_inverse() {
        let r = rules(DomainMode::Include, &["github.com", "*.github.com"]);
        assert!(r.should_send("https://github.com/x"));
        assert!(r.should_send("https://gist.github.com/x"));
        assert!(!r.should_send("https://example.com/"));
    }

    #[test]
    fn ip_addresses_and_ranges() {
        let r = rules(DomainMode::Exclude, &["192.168.1.10", "10.0.0.0/8", "::1", "fd00::/8"]);
        assert!(!r.should_send("http://192.168.1.10:8080/"));
        assert!(r.should_send("http://192.168.1.11/"));
        assert!(!r.should_send("http://10.20.30.40/"));
        assert!(!r.should_send("http://[::1]:3000/"));
        assert!(!r.should_send("http://[fd12::1]/"));
        assert!(r.should_send("http://[2001:db8::1]/"));
        // a name is never matched by an IP pattern, and the reverse
        assert!(r.should_send("http://10.example.com/"));
    }

    #[test]
    fn url_parsing_edge_cases() {
        let r = rules(DomainMode::Exclude, &["example.com"]);
        assert!(!r.should_send("HTTPS://EXAMPLE.COM./path"));
        assert!(!r.should_send("https://user:pw@example.com:8443/x"));
        // '@' in the path/query must not change the host
        assert!(r.should_send("https://evil.test/?u=a@example.com"));
        assert!(r.should_send("https://evil.test/@example.com"));
        assert!(r.should_send("https://example.com.evil.test/"));
        assert!(!r.should_send("https://example.com?x=1"));
    }

    #[test]
    fn other_schemes_are_never_filtered() {
        let r = rules(DomainMode::Include, &["example.com"]);
        assert!(r.should_send("vcc://vpm/addRepo?url=https://nope.test/"));
    }

    #[test]
    fn bad_patterns_are_ignored() {
        let r = rules(DomainMode::Exclude, &["*", "a*b.com", "10.0.0.0/99", " "]);
        assert!(r.is_empty());
    }

    #[test]
    fn mode_parse() {
        assert_eq!(DomainMode::parse("include"), DomainMode::Include);
        assert_eq!(DomainMode::parse(" INCLUDE "), DomainMode::Include);
        assert_eq!(DomainMode::parse("exclude"), DomainMode::Exclude);
        assert_eq!(DomainMode::parse(""), DomainMode::Exclude);
    }
}
