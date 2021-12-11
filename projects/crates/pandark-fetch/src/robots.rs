use url::Url;

use pandark_types::PolitenessProfile;

/// Robots policy evaluation for a crawl profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobotsPolicy {
    profile: PolitenessProfile,
    disallowed_paths: Vec<String>,
}

impl RobotsPolicy {
    /// Build policy from a politeness profile.
    pub fn from_profile(profile: PolitenessProfile) -> Self {
        let disallowed_paths = match profile {
            PolitenessProfile::Conservative => vec!["/admin".into(), "/private".into()],
            PolitenessProfile::Balanced => vec!["/admin".into()],
            PolitenessProfile::Developer => Vec::new(),
        };
        Self {
            profile,
            disallowed_paths,
        }
    }

    /// Whether fetching the URL is allowed under this policy.
    pub fn is_allowed(&self, url: &Url) -> bool {
        if url.scheme() != "http" && url.scheme() != "https" && url.scheme() != "file" {
            return false;
        }
        if matches!(self.profile, PolitenessProfile::Developer) {
            return true;
        }
        let path = url.path();
        !self
            .disallowed_paths
            .iter()
            .any(|prefix| path.starts_with(prefix))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pandark_types::PolitenessProfile;
    use url::Url;

    #[test]
    fn conservative_blocks_admin_paths() {
        let policy = RobotsPolicy::from_profile(PolitenessProfile::Conservative);
        let url = Url::parse("https://example.com/admin/settings").expect("url");
        assert!(!policy.is_allowed(&url));
    }
}
