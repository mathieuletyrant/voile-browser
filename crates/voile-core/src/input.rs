//! Interprétation du texte tapé dans la barre de commande.

use url::Url;

/// Transforme une saisie en URL navigable : URL complète, domaine nu,
/// `localhost:3000`, IP… sinon recherche via `search_url` (`%s`).
pub fn resolve_input(input: &str, search_url: &str) -> String {
    let input = input.trim();
    if input.is_empty() {
        return "about:blank".into();
    }

    if let Ok(url) = Url::parse(input) {
        let ok_scheme = matches!(
            url.scheme(),
            "http" | "https" | "file" | "about" | "chrome" | "chrome-extension" | "voile" | "data"
        );
        if ok_scheme && (url.has_host() || url.scheme() != "http" && url.scheme() != "https") {
            return url.into();
        }
    }

    if !input.contains(char::is_whitespace) && looks_like_host(input) {
        let scheme = if is_local(input) { "http" } else { "https" };
        if let Ok(url) = Url::parse(&format!("{scheme}://{input}")) {
            return url.into();
        }
    }

    search_url.replace("%s", &encode_query(input))
}

fn looks_like_host(input: &str) -> bool {
    let host = input.split(['/', '?', '#']).next().unwrap_or_default();
    let host_no_port = host.rsplit_once(':').map_or(host, |(h, port)| {
        if port.chars().all(|c| c.is_ascii_digit()) {
            h
        } else {
            host
        }
    });
    if host_no_port.eq_ignore_ascii_case("localhost") {
        return true;
    }
    if host_no_port.parse::<std::net::Ipv4Addr>().is_ok() {
        return true;
    }
    let labels: Vec<&str> = host_no_port.split('.').collect();
    labels.len() >= 2
        && labels
            .iter()
            .all(|l| !l.is_empty() && l.chars().all(|c| c.is_alphanumeric() || c == '-'))
        && labels
            .last()
            .is_some_and(|tld| tld.len() >= 2 && tld.chars().all(char::is_alphabetic))
}

fn is_local(input: &str) -> bool {
    let host = input.split(['/', ':']).next().unwrap_or_default();
    host.eq_ignore_ascii_case("localhost")
        || host.ends_with(".local")
        || host
            .parse::<std::net::Ipv4Addr>()
            .is_ok_and(|ip| ip.is_private() || ip.is_loopback())
}

fn encode_query(q: &str) -> String {
    url::form_urlencoded::byte_serialize(q.as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    const S: &str = "https://s.test/?q=%s";

    #[test]
    fn full_urls_pass_through() {
        assert_eq!(
            resolve_input("https://github.com/pulls", S),
            "https://github.com/pulls"
        );
        assert_eq!(resolve_input("about:blank", S), "about:blank");
        assert_eq!(
            resolve_input("voile://app/settings", S),
            "voile://app/settings"
        );
    }

    #[test]
    fn bare_domains_get_https() {
        assert_eq!(resolve_input("github.com", S), "https://github.com/");
        assert_eq!(
            resolve_input("cheztomy.cc/docs", S),
            "https://cheztomy.cc/docs"
        );
    }

    #[test]
    fn local_hosts_get_http() {
        assert_eq!(resolve_input("localhost:3000", S), "http://localhost:3000/");
        assert_eq!(resolve_input("192.168.68.58", S), "http://192.168.68.58/");
        assert_eq!(
            resolve_input("homeassistant.local:8123", S),
            "http://homeassistant.local:8123/"
        );
    }

    #[test]
    fn everything_else_is_a_search() {
        assert_eq!(
            resolve_input("rust cef tabs", S),
            "https://s.test/?q=rust+cef+tabs"
        );
        assert_eq!(resolve_input("arc", S), "https://s.test/?q=arc");
        assert_eq!(resolve_input("v1.2", S), "https://s.test/?q=v1.2");
    }
}
