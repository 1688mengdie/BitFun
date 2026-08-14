//! Materialize the bundled DeepSeek Harness bridge into the user's dsh install.
//!
//! BitFun ships a compiled ACP bridge, but it does not ship a harness: `dsh` is
//! a profile launcher, and a profile is just a directory under
//! `$DSH_HOME/profiles/`. So instead of publishing the bridge to a registry, we
//! copy the built profile next to the harness the user already installed and
//! launch it with `dsh --profile bitfun-acp`. Every `@deepseek-ai/dsh-*` row in
//! that profile resolves out of the user's own installation through the flat
//! `profiles/node_modules` fallback the launcher maintains, so nothing here
//! installs a second harness, and the user's model and credentials — which live
//! in dsh, not in BitFun — apply unchanged.
//!
//! The copy is idempotent: the built profile carries a content digest, and a
//! destination already stamped with the same digest is left alone.

use std::path::{Path, PathBuf};

use bitfun_core::util::errors::{BitFunError, BitFunResult};
use serde::Deserialize;

use super::requirements::probe_executable;

/// Points directly at a built profile directory, overriding every other
/// candidate. This is how a packaged build that lays resources out unusually —
/// or a developer testing an unbuilt tree — names the source explicitly.
const SOURCE_DIR_ENV: &str = "BITFUN_DSH_PROFILE_DIR";

/// dsh's own state directory variable. Respecting it matters: a user who moved
/// `$DSH_HOME` keeps their models and credentials there, and a profile written
/// under `~/.dsh` instead would simply not be found by the launcher.
const DSH_HOME_ENV: &str = "DSH_HOME";

/// Basename of the build stamp `scripts/build-profile.mjs` writes.
const STAMP_FILENAME: &str = ".bitfun-bridge.json";

/// Directories the build owns end to end, replaced rather than merged.
///
/// Merging would leave a previous build's emit behind — a `lib/*.js` that no
/// longer exists in the source still resolves, and the profile boots a mix of
/// two versions. Everything outside these three is left untouched, so anything
/// the user added to the profile directory survives an upgrade.
const MANAGED_SUBDIRECTORIES: &[&str] = &["lib", "presets", "node_modules"];

/// What `scripts/build-profile.mjs` records about a build.
#[derive(Debug, Deserialize)]
struct BridgeStamp {
    /// Digest over every path and its bytes in the built profile. A version
    /// string would not do: during development the bridge version stands still
    /// while its code changes, and a stale profile would look current.
    content: String,
    /// The oldest `@deepseek-ai/dsh` this build is known to boot against.
    #[serde(rename = "minDshVersion")]
    min_dsh_version: String,
}

/// Copy the bundled profile into the user's dsh install if it is not there yet.
///
/// Returns the profile directory the launcher will boot. Errors are the ones
/// worth stopping for: no bundled profile to copy, a dsh too old to run it, or
/// a filesystem that refused the write — each of which would otherwise surface
/// as `dsh` complaining about a profile that does not exist, or as a pile of
/// module resolution failures with no hint about the cause.
pub(crate) async fn ensure_bundled_profile(profile: &str) -> BitFunResult<PathBuf> {
    let source = bundled_profile_source(profile).ok_or_else(|| {
        BitFunError::service(format!(
            "The bundled DeepSeek Harness profile '{profile}' is missing from this BitFun build. \
             Build it with `npm run build && node scripts/build-profile.mjs` in packages/dsh-acp, \
             or point {SOURCE_DIR_ENV} at a built profile directory."
        ))
    })?;
    let stamp = read_stamp(&source)?.ok_or_else(|| {
        BitFunError::service(format!(
            "The bundled DeepSeek Harness profile at {} has no {STAMP_FILENAME}; it is not a \
             finished build.",
            source.display()
        ))
    })?;

    // Checked on every launch, not just when copying: the pairing can break
    // later by the user downgrading dsh under a profile that is already current.
    require_supported_dsh(&stamp.min_dsh_version).await?;

    let destination = dsh_profiles_directory()?.join(profile);
    if read_stamp(&destination)
        .ok()
        .flatten()
        .is_some_and(|installed| installed.content == stamp.content)
    {
        return Ok(destination);
    }

    let target = destination.clone();
    tokio::task::spawn_blocking(move || install_profile(&source, &target))
        .await
        .map_err(|error| {
            BitFunError::service(format!(
                "Failed to install the DeepSeek Harness profile: {error}"
            ))
        })??;

    log::info!(
        "Installed the bundled DeepSeek Harness profile at {}",
        destination.display()
    );
    Ok(destination)
}

/// Fail with an actionable message when the installed dsh predates this build.
///
/// A dsh we cannot find, or whose version we cannot parse, is not treated as a
/// failure here: a missing `dsh` already surfaces as an install prompt in the
/// agent list, and guessing about an unrecognized version string would block a
/// launch that may well work.
async fn require_supported_dsh(minimum: &str) -> BitFunResult<()> {
    let Some(installed) = probe_executable("dsh").await.version else {
        return Ok(());
    };
    if version_is_supported(&installed, minimum) {
        return Ok(());
    }
    Err(BitFunError::service(format!(
        "DeepSeek Harness {installed} is older than {minimum}, which this BitFun build requires. \
         Update it with `npm install -g @deepseek-ai/dsh`."
    )))
}

/// Whether `installed` is new enough for a build that needs `minimum`.
///
/// Unparsable on either side means "assume yes" — see the caller.
fn version_is_supported(installed: &str, minimum: &str) -> bool {
    let Ok(minimum) = semver::Version::parse(minimum.trim_start_matches('v')) else {
        return true;
    };
    let Ok(installed) = semver::Version::parse(installed.trim().trim_start_matches('v')) else {
        return true;
    };
    installed >= minimum
}

/// Locate the built profile that ships with this BitFun.
fn bundled_profile_source(profile: &str) -> Option<PathBuf> {
    if let Some(configured) = std::env::var_os(SOURCE_DIR_ENV) {
        let configured = PathBuf::from(configured);
        return configured.is_dir().then_some(configured);
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            candidates.push(directory.join("resources").join("dsh-profile"));
            if let Some(parent) = directory.parent() {
                candidates.push(
                    parent
                        .join("Resources")
                        .join("resources")
                        .join("dsh-profile"),
                );
                candidates.push(parent.join("Resources").join("dsh-profile"));
                if let Some(name) = executable.file_name().and_then(|name| name.to_str()) {
                    candidates.push(
                        parent
                            .join("lib")
                            .join(name)
                            .join("resources")
                            .join("dsh-profile"),
                    );
                    candidates.push(
                        parent
                            .join("share")
                            .join(name)
                            .join("resources")
                            .join("dsh-profile"),
                    );
                }
            }
        }
    }
    // The development tree, where the build writes straight into the package.
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../packages/dsh-acp/dist-profile"),
    );

    candidates
        .into_iter()
        // A profile directory is only the right one if it is the named profile.
        .find(|candidate| {
            candidate.is_dir() && profile_name_of(candidate) == Some(profile.to_string())
        })
}

/// The profile a built directory declares itself to be.
fn profile_name_of(directory: &Path) -> Option<String> {
    #[derive(Deserialize)]
    struct NamedProfile {
        profile: String,
    }
    let contents = std::fs::read(directory.join(STAMP_FILENAME)).ok()?;
    serde_json::from_slice::<NamedProfile>(&contents)
        .ok()
        .map(|stamp| stamp.profile)
}

/// Read a build stamp, distinguishing "not installed" from "unreadable".
fn read_stamp(directory: &Path) -> BitFunResult<Option<BridgeStamp>> {
    let path = directory.join(STAMP_FILENAME);
    let contents = match std::fs::read(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(BitFunError::service(format!(
                "Failed to read {}: {error}",
                path.display()
            )))
        }
    };
    serde_json::from_slice(&contents)
        .map(Some)
        .map_err(|error| {
            BitFunError::service(format!("Failed to parse {}: {error}", path.display()))
        })
}

/// `$DSH_HOME/profiles`, created if the user has not run dsh yet.
fn dsh_profiles_directory() -> BitFunResult<PathBuf> {
    let home = match std::env::var_os(DSH_HOME_ENV) {
        Some(home) if !home.is_empty() => PathBuf::from(home),
        _ => dirs::home_dir()
            .ok_or_else(|| {
                BitFunError::service(
                    "Cannot locate the home directory that holds the DeepSeek Harness install"
                        .to_string(),
                )
            })?
            .join(".dsh"),
    };
    Ok(home.join("profiles"))
}

/// Write the built profile into `destination`, stamp last.
///
/// The stamp is copied only after every other file lands, so an interrupted
/// install reads as absent rather than current and is simply redone.
fn install_profile(source: &Path, destination: &Path) -> BitFunResult<()> {
    let write_error = |error: std::io::Error| {
        BitFunError::service(format!(
            "Failed to install the DeepSeek Harness profile into {}: {error}",
            destination.display()
        ))
    };

    // A half-written stamp is the one file that must never be believed, so drop
    // it up front: everything below runs with the destination marked stale.
    match std::fs::remove_file(destination.join(STAMP_FILENAME)) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(write_error(error)),
    }
    std::fs::create_dir_all(destination).map_err(write_error)?;
    for managed in MANAGED_SUBDIRECTORIES {
        match std::fs::remove_dir_all(destination.join(managed)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(write_error(error)),
        }
    }

    copy_tree(source, destination, true).map_err(write_error)?;
    std::fs::copy(
        source.join(STAMP_FILENAME),
        destination.join(STAMP_FILENAME),
    )
    .map_err(write_error)?;
    Ok(())
}

/// Copy `source` over `destination`, overwriting files and keeping extras.
///
/// `skip_stamp` holds for the top level only — the stamp is the install marker
/// and its own step.
fn copy_tree(source: &Path, destination: &Path, skip_stamp: bool) -> std::io::Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        if skip_stamp && name == STAMP_FILENAME {
            continue;
        }
        let target = destination.join(&name);
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target, false)?;
        } else {
            // Remove first: overwriting in place would follow a symlink the
            // user (or a previous vendoring step) left behind.
            match std::fs::remove_file(&target) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_directory(label: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("bitfun-dsh-{label}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("scratch directory should be created");
        path
    }

    fn write_built_profile(root: &Path, content: &str) {
        std::fs::create_dir_all(root.join("lib")).expect("lib should be created");
        std::fs::write(root.join("lib/app.js"), content).expect("app should be written");
        std::fs::write(root.join("cordis.patch.yml"), "- insert: []\n").expect("patch");
        std::fs::write(
            root.join(STAMP_FILENAME),
            format!(
                r#"{{"bridge":"0.0.1","content":"{content}","minDshVersion":"0.1.0-rc.6","profile":"bitfun-acp"}}"#
            ),
        )
        .expect("stamp should be written");
    }

    #[test]
    fn installs_the_built_profile_and_replaces_a_previous_build() {
        let source = scratch_directory("source");
        let destination = scratch_directory("destination");
        write_built_profile(&source, "first");

        install_profile(&source, &destination).expect("first install");
        assert_eq!(
            std::fs::read_to_string(destination.join("lib/app.js")).expect("app"),
            "first"
        );

        // A file the previous build emitted but this one does not must not
        // survive into a tree that now claims to be the new build.
        std::fs::write(destination.join("lib/stale.js"), "stale").expect("stale file");
        // Anything outside the managed directories is the user's, and stays.
        std::fs::write(destination.join("notes.md"), "mine").expect("user file");

        write_built_profile(&source, "second");
        install_profile(&source, &destination).expect("second install");

        assert_eq!(
            std::fs::read_to_string(destination.join("lib/app.js")).expect("app"),
            "second"
        );
        assert!(!destination.join("lib/stale.js").exists());
        assert_eq!(
            std::fs::read_to_string(destination.join("notes.md")).expect("user file"),
            "mine"
        );

        let _ = std::fs::remove_dir_all(&source);
        let _ = std::fs::remove_dir_all(&destination);
    }

    #[test]
    fn reads_the_content_digest_that_decides_whether_to_reinstall() {
        let source = scratch_directory("stamp");
        write_built_profile(&source, "digest");

        let stamp = read_stamp(&source)
            .expect("stamp should be readable")
            .expect("stamp should be present");
        assert_eq!(stamp.content, "digest");
        assert_eq!(stamp.min_dsh_version, "0.1.0-rc.6");
        assert_eq!(profile_name_of(&source).as_deref(), Some("bitfun-acp"));

        let empty = scratch_directory("stamp-empty");
        assert!(read_stamp(&empty)
            .expect("a missing stamp is not an error")
            .is_none());

        let _ = std::fs::remove_dir_all(&source);
        let _ = std::fs::remove_dir_all(&empty);
    }

    #[test]
    fn compares_prerelease_harness_versions() {
        assert!(version_is_supported("0.1.0-rc.6", "0.1.0-rc.6"));
        assert!(version_is_supported("0.1.0-rc.7", "0.1.0-rc.6"));
        assert!(version_is_supported("0.1.0", "0.1.0-rc.6"));
        assert!(version_is_supported("1.2.3", "0.1.0-rc.6"));
        assert!(!version_is_supported("0.1.0-rc.5", "0.1.0-rc.6"));
        assert!(!version_is_supported("0.0.9", "0.1.0-rc.6"));
        // A version we cannot read must not block a launch that may work.
        assert!(version_is_supported("dsh dev build", "0.1.0-rc.6"));
    }

    #[test]
    fn source_lookup_honours_the_override_and_checks_the_profile_name() {
        let source = scratch_directory("override");
        write_built_profile(&source, "override");

        temp_env(
            SOURCE_DIR_ENV,
            Some(source.to_string_lossy().as_ref()),
            || {
                assert_eq!(
                    bundled_profile_source("bitfun-acp").as_deref(),
                    Some(source.as_path())
                );
            },
        );
        // Without the override, a directory that names a different profile is
        // not a match — the repo checkout below it may still be.
        assert_ne!(profile_name_of(&source).as_deref(), Some("other"));

        let _ = std::fs::remove_dir_all(&source);
    }

    /// Set an environment variable for the duration of `body`.
    ///
    /// Tests in one binary share a process, so this restores what it found.
    fn temp_env(key: &str, value: Option<&str>, body: impl FnOnce()) {
        let previous = std::env::var_os(key);
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
        body();
        match previous {
            Some(previous) => std::env::set_var(key, previous),
            None => std::env::remove_var(key),
        }
    }
}
