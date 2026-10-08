//! Public suggestions, separate from the device's successfully checked history.
//! Never pin API URLs here: Matrix discovery resolves each selected server.
pub struct PublicServer {
    pub name: &'static str,
    pub description: &'static str,
    /// Only for communities whose documented onboarding is on their website.
    pub signup_website: Option<&'static str>,
}

// Checked 2026-10-07 against the operators' pages and live discovery:
// https://matrix.org/homeserver/about/
// https://tchncs.de/matrix
// https://wiki.mozilla.org/Matrix:Join
// Availability and registration requirements are still checked at runtime.
pub const PUBLIC_SERVERS: &[PublicServer] = &[
    PublicServer {
        name: "matrix.org",
        description: "Matrix.org Foundation",
        signup_website: None,
    },
    PublicServer {
        name: "tchncs.de",
        description: "Independent community server",
        signup_website: None,
    },
    PublicServer {
        name: "mozilla.org",
        description: "Mozilla community · single sign-on",
        signup_website: Some("https://chat.mozilla.org"),
    },
];

pub fn find(server: &str) -> Option<&'static PublicServer> {
    let server = super::homeserver::login_server("", Some(server)).ok()?;
    let url = url::Url::parse(&if server.contains("://") {
        server
    } else {
        format!("https://{server}")
    })
    .ok()?;
    // Do not treat an alternate port, path or cleartext endpoint as the preset.
    if url.scheme() != "https" || url.port().is_some() || url.path() != "/" {
        return None;
    }
    PUBLIC_SERVERS
        .iter()
        .find(|entry| Some(entry.name) == url.host_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_distinct_valid_servers_and_only_exact_https_aliases() {
        let mut names = std::collections::HashSet::new();
        for entry in PUBLIC_SERVERS {
            assert!(names.insert(entry.name));
            assert_eq!(find(entry.name).unwrap().name, entry.name);
            assert_eq!(
                find(&format!("https://{}/", entry.name)).unwrap().name,
                entry.name
            );
        }
        for custom in [
            "http://mozilla.org",
            "mozilla.org:19443",
            "https://mozilla.org/custom",
            "https://mozilla.org.attacker.example",
            "https://mozilla.org@attacker.example",
        ] {
            assert!(find(custom).is_none(), "{custom}");
        }
    }
}
