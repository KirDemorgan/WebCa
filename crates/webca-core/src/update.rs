use std::{cmp::Ordering, fmt, str::FromStr};

use serde_json::Value;

/// A `major.minor.patch` version as used in release tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl FromStr for Version {
    type Err = ();

    /// Accepts `1.2.3`, `v1.2.3` and `1.2` (patch 0); anything after a `-` or `+` is ignored.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let core = text.trim().trim_start_matches(['v', 'V']).split(['-', '+']).next().unwrap_or_default();
        let numbers: Vec<u32> = core.split('.').map(str::parse).collect::<Result<_, _>>().map_err(drop)?;
        match numbers.as_slice() {
            [major] => Ok(Self { major: *major, minor: 0, patch: 0 }),
            [major, minor] => Ok(Self { major: *major, minor: *minor, patch: 0 }),
            [major, minor, patch] => Ok(Self { major: *major, minor: *minor, patch: *patch }),
            _ => Err(()),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// A published release newer than the running build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    pub version: Version,
    pub url: String,
}

/// GitHub API address of the latest release of `repository`, given as a GitHub URL or `owner/name`.
#[must_use]
pub fn latest_release_endpoint(repository: &str) -> Option<String> {
    let path = repository
        .trim()
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("github.com/");
    let (owner, name) = path.split_once('/')?;
    if owner.is_empty() || name.is_empty() || name.contains('/') {
        return None;
    }
    Some(format!("https://api.github.com/repos/{owner}/{name}/releases/latest"))
}

/// Reads a GitHub "latest release" response and returns it when it is newer than `current`.
#[must_use]
pub fn newer_release(current: Version, response: &str) -> Option<Update> {
    let release: Value = serde_json::from_str(response).ok()?;
    let flagged = |key: &str| release.get(key).and_then(Value::as_bool).unwrap_or(false);
    if flagged("draft") || flagged("prerelease") {
        return None;
    }
    let version: Version = release.get("tag_name")?.as_str()?.parse().ok()?;
    let url = release.get("html_url")?.as_str()?.to_owned();
    (version.cmp(&current) == Ordering::Greater).then_some(Update { version, url })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(text: &str) -> Version {
        text.parse().unwrap()
    }

    #[test]
    fn parses_release_tags() {
        assert_eq!(version("v1.2.3"), Version { major: 1, minor: 2, patch: 3 });
        assert_eq!(version("V2.0"), Version { major: 2, minor: 0, patch: 0 });
        assert_eq!(version("3"), Version { major: 3, minor: 0, patch: 0 });
        assert_eq!(version(" 0.4.1-beta.2+build "), Version { major: 0, minor: 4, patch: 1 });
        assert_eq!(version("1.10.0").to_string(), "1.10.0");
        for bad in ["", "v", "1.x", "1.2.y", "1.2.3.4", "a.1"] {
            assert!(bad.parse::<Version>().is_err(), "{bad}");
        }
        assert!(version("1.10.0") > version("1.9.9"));
    }

    #[test]
    fn builds_the_api_endpoint() {
        let expected = Some("https://api.github.com/repos/owner/webca/releases/latest".to_owned());
        assert_eq!(latest_release_endpoint("https://github.com/owner/webca"), expected);
        assert_eq!(latest_release_endpoint("https://github.com/owner/webca.git/"), expected);
        assert_eq!(latest_release_endpoint("owner/webca"), expected);
        assert_eq!(latest_release_endpoint("http://github.com/owner/webca"), expected);
        assert_eq!(latest_release_endpoint(""), None);
        assert_eq!(latest_release_endpoint("owner"), None);
        assert_eq!(latest_release_endpoint("owner/"), None);
        assert_eq!(latest_release_endpoint("https://github.com/owner/webca/releases"), None);
    }

    #[test]
    fn reports_only_newer_published_releases() {
        let current = version("0.1.0");
        let release = r#"{"tag_name":"v0.2.0","html_url":"https://github.com/o/w/releases/tag/v0.2.0"}"#;
        assert_eq!(
            newer_release(current, release),
            Some(Update {
                version: version("0.2.0"),
                url: "https://github.com/o/w/releases/tag/v0.2.0".into()
            })
        );
        assert_eq!(newer_release(version("0.2.0"), release), None);
        let draft = r#"{"tag_name":"v9.0.0","html_url":"u","draft":true}"#;
        let pre = r#"{"tag_name":"v9.0.0","html_url":"u","prerelease":true}"#;
        assert_eq!(newer_release(current, draft), None);
        assert_eq!(newer_release(current, pre), None);
        for broken in
            ["nope", "{}", r#"{"tag_name":7}"#, r#"{"tag_name":"next"}"#, r#"{"tag_name":"v1.0.0"}"#]
        {
            assert_eq!(newer_release(current, broken), None, "{broken}");
        }
        let missing_url = r#"{"tag_name":"v1.0.0","html_url":3}"#;
        assert_eq!(newer_release(current, missing_url), None);
    }
}
