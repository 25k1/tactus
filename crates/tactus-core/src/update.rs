use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tactus_api::error::{Error, Result};
use tactus_api::net;
use tactus_api::parse::Json;

pub const REPOSITORY: &str = "25k1/tactus";
pub const ASSET: &str = "tactus.exe";
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub url: String,
    pub digest: Option<String>,
    pub page: String,
}

fn numbers(version: &str) -> Vec<u64> {
    version.trim().trim_start_matches(['v', 'V']).split(['.', '-', '+']).map_while(|part| part.parse().ok()).collect()
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    let (candidate, current) = (numbers(candidate), numbers(current));
    !candidate.is_empty() && candidate > current
}

pub fn parse_release(value: &Value) -> Option<Release> {
    let asset = value.list("assets").iter().find(|asset| asset.text("name").eq_ignore_ascii_case(ASSET))?;
    let digest = asset.text("digest").strip_prefix("sha256:").map(str::to_ascii_lowercase);
    Some(Release {
        version: value.text("tag_name").trim_start_matches(['v', 'V']).to_owned(),
        notes: value.text("body").to_owned(),
        url: asset.text("browser_download_url").to_owned(),
        digest,
        page: value.text("html_url").to_owned(),
    })
}

pub fn latest() -> Result<Option<Release>> {
    let url = format!("https://api.github.com/repos/{REPOSITORY}/releases/latest");
    net::block_on(async {
        let response = net::client().get(url).header("Accept", "application/vnd.github+json").send().await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let value: Value = response.error_for_status()?.json().await?;
        Ok::<_, Error>(parse_release(&value).filter(|release| is_newer(&release.version, CURRENT)))
    })
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn verify(data: &[u8], digest: Option<&str>) -> Result<()> {
    match digest {
        Some(expected) if hex(&Sha256::digest(data)) != expected => {
            Err(Error::Api("the downloaded update does not match its checksum".into()))
        }
        _ => Ok(()),
    }
}

fn sibling(executable: &Path, extension: &str) -> PathBuf {
    executable.with_extension(extension)
}

pub fn replace(executable: &Path, data: &[u8]) -> Result<()> {
    let (fresh, old) = (sibling(executable, "exe.new"), sibling(executable, "exe.old"));
    std::fs::write(&fresh, data)?;
    let _ = std::fs::remove_file(&old);
    std::fs::rename(executable, &old)?;
    if let Err(error) = std::fs::rename(&fresh, executable) {
        let _ = std::fs::rename(&old, executable);
        return Err(error.into());
    }
    Ok(())
}

pub fn install(release: &Release) -> Result<PathBuf> {
    let data = net::block_on(async {
        let response = net::client().get(&release.url).send().await?.error_for_status()?;
        Ok::<_, Error>(response.bytes().await?.to_vec())
    })?;
    verify(&data, release.digest.as_deref())?;
    let executable = std::env::current_exe()?;
    replace(&executable, &data)?;
    Ok(executable)
}

pub fn cleanup() {
    if let Ok(executable) = std::env::current_exe() {
        let _ = std::fs::remove_file(sibling(&executable, "exe.old"));
        let _ = std::fs::remove_file(sibling(&executable, "exe.new"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compares_versions() {
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
        assert!(!is_newer("nightly", "0.1.0"));
    }

    proptest::proptest! {
        #[test]
        fn newer_is_a_strict_order(a in "[0-9]{1,3}(\\.[0-9]{1,3}){0,3}", b in "[0-9]{1,3}(\\.[0-9]{1,3}){0,3}") {
            proptest::prop_assert!(!is_newer(&a, &a));
            proptest::prop_assert!(!(is_newer(&a, &b) && is_newer(&b, &a)));
            proptest::prop_assert_eq!(is_newer(&format!("v{a}"), &b), is_newer(&a, &b));
        }
    }

    #[test]
    fn parses_the_release_asset() {
        let value = json!({
            "tag_name": "v0.2.0",
            "body": "Notes",
            "html_url": "https://github.com/25k1/tactus/releases/tag/v0.2.0",
            "assets": [
                { "name": "tactus-v0.2.0-windows-x64.zip", "browser_download_url": "zip" },
                { "name": "tactus.exe", "browser_download_url": "exe", "digest": "sha256:ABC" }
            ]
        });
        let release = parse_release(&value).unwrap();
        assert_eq!((release.version.as_str(), release.url.as_str()), ("0.2.0", "exe"));
        assert_eq!(release.digest.as_deref(), Some("abc"));
        assert!(parse_release(&json!({ "tag_name": "v1", "assets": [] })).is_none());
    }

    #[test]
    fn verifies_checksums() {
        let digest = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        assert!(verify(b"hello", Some(digest)).is_ok());
        assert!(verify(b"hullo", Some(digest)).is_err());
        assert!(verify(b"anything", None).is_ok());
    }

    #[test]
    fn replaces_the_executable_and_keeps_the_old_one() {
        let folder = std::env::temp_dir().join(format!("tactus-update-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let executable = folder.join("tactus.exe");
        std::fs::write(&executable, b"old").unwrap();
        replace(&executable, b"new").unwrap();
        assert_eq!(std::fs::read(&executable).unwrap(), b"new");
        assert_eq!(std::fs::read(folder.join("tactus.exe.old")).unwrap(), b"old");
        std::fs::remove_dir_all(folder).unwrap();
    }
}
