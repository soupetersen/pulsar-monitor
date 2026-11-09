//! Auto-update, compatible with the releases of the Tauri versions: the
//! latest GitHub release has a `latest.json` listing a signed package per
//! platform. Checks run shortly after startup, every hour, and when the
//! widget is shown again (at most every 10 minutes). When a newer, correctly
//! signed version exists, the UI shows an update button; clicking it
//! downloads and installs it, and Pulsar restarts.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use base64::Engine;
use serde::Deserialize;

const LATEST: &str = "https://github.com/soupetersen/pulsar-monitor/releases/latest/download/latest.json";
/// The minisign public key the releases are signed with (base64 of the
/// key file, as in the Tauri config).
const PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDlGRkVEQkU3MEZGOEZDMEMKUldRTS9QZ1A1OXYrbnk1Q0JVUmdJT0x3T2xWQVdLTnZubnAzYktIOGRZaHJmREducVZSQlpHWWYK";

const FIRST_CHECK: Duration = Duration::from_secs(20);
const CHECK_EVERY: Duration = Duration::from_secs(60 * 60);
const MIN_GAP: Duration = Duration::from_secs(10 * 60);
/// Packages are a few MB; anything much bigger is not ours.
const MAX_DOWNLOAD: u64 = 200 * 1024 * 1024;

/// Set on the new process after an update, so it waits for the old one to
/// quit instead of handing over to it (see single_instance).
pub const RESTARTED_ENV: &str = "PULSAR_RESTARTED";

#[derive(Clone, Debug)]
pub struct Update {
    pub version: String,
    url: String,
    signature: String,
}

#[derive(Deserialize)]
struct Latest {
    version: String,
    platforms: std::collections::HashMap<String, Platform>,
}

#[derive(Deserialize)]
struct Platform {
    signature: String,
    url: String,
}

/// Starts the check thread. `found` gets each newer version found; `nudge`
/// wakes the thread for an early check (rate-limited).
pub fn spawn(found: impl Fn(Update) + Send + 'static, nudge: Receiver<()>) {
    std::thread::Builder::new()
        .name("updater".into())
        .spawn(move || {
            std::thread::sleep(FIRST_CHECK);
            let mut last = Instant::now();
            let check = |found: &dyn Fn(Update)| {
                // Errors (offline, no package for this platform yet...)
                // are ignored: the next check tries again.
                if let Ok(Some(update)) = check() {
                    found(update);
                }
            };
            check(&found);
            loop {
                match nudge.recv_timeout(CHECK_EVERY) {
                    Ok(()) if last.elapsed() < MIN_GAP => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    _ => {}
                }
                last = Instant::now();
                check(&found);
            }
        })
        .expect("failed to start the updater thread");
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(120)))
        .user_agent(concat!("Pulsar/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn check() -> Result<Option<Update>, String> {
    let latest: Latest = agent()
        .get(LATEST)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_json()
        .map_err(|e| e.to_string())?;
    let newer = semver::Version::parse(latest.version.trim_start_matches('v'))
        .ok()
        .zip(semver::Version::parse(env!("CARGO_PKG_VERSION")).ok())
        .is_some_and(|(theirs, ours)| theirs > ours);
    if !newer {
        return Ok(None);
    }
    let platform = platform_keys()
        .iter()
        .find_map(|key| latest.platforms.get(key.as_str()))
        .ok_or("no package for this platform")?;
    Ok(Some(Update { version: latest.version, url: platform.url.clone(), signature: platform.signature.clone() }))
}

/// The `latest.json` keys for this installation, most specific first (the
/// same scheme as Tauri's updater: `{os}-{arch}-{installer}`, `{os}-{arch}`).
fn platform_keys() -> Vec<String> {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        os => os,
    };
    let arch = std::env::consts::ARCH;
    let mut keys = Vec::new();
    if let Some(kind) = install_kind().installer() {
        keys.push(format!("{os}-{arch}-{kind}"));
    }
    // The bare key is the AppImage on Linux: only an AppImage takes it.
    if !cfg!(target_os = "linux") || matches!(install_kind(), InstallKind::AppImage(_)) {
        keys.push(format!("{os}-{arch}"));
    }
    keys
}

#[derive(Clone, Debug)]
enum InstallKind {
    /// Windows: installed by the NSIS installer.
    Nsis,
    /// macOS: the bundle to replace.
    App(PathBuf),
    AppImage(PathBuf),
    Deb,
    Rpm,
    /// A bare binary nobody installed: can't update it.
    Unknown,
}

impl InstallKind {
    fn installer(&self) -> Option<&'static str> {
        match self {
            Self::Nsis => Some("nsis"),
            Self::App(_) => Some("app"),
            Self::AppImage(_) => Some("appimage"),
            Self::Deb => Some("deb"),
            Self::Rpm => Some("rpm"),
            Self::Unknown => None,
        }
    }
}

fn install_kind() -> InstallKind {
    let Ok(exe) = std::env::current_exe() else { return InstallKind::Unknown };
    if cfg!(windows) {
        return InstallKind::Nsis;
    }
    if cfg!(target_os = "macos") {
        // .../Pulsar.app/Contents/MacOS/pulsar
        let bundle = exe.ancestors().nth(3).filter(|p| p.extension().is_some_and(|e| e == "app"));
        return bundle.map_or(InstallKind::Unknown, |b| InstallKind::App(b.to_path_buf()));
    }
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        return InstallKind::AppImage(appimage.into());
    }
    let owned_by = |cmd: &str, args: &[&str]| {
        std::process::Command::new(cmd)
            .args(args)
            .arg(&exe)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    };
    if owned_by("rpm", &["-qf"]) {
        InstallKind::Rpm
    } else if owned_by("dpkg", &["-S"]) {
        InstallKind::Deb
    } else {
        InstallKind::Unknown
    }
}

/// Downloads the package and checks its signature. Returns its path.
fn download(update: &Update) -> Result<PathBuf, String> {
    let mut body = Vec::new();
    agent()
        .get(&update.url)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .as_reader()
        .take(MAX_DOWNLOAD)
        .read_to_end(&mut body)
        .map_err(|e| e.to_string())?;
    verify(&body, &update.signature)?;

    let name = update.url.rsplit('/').next().filter(|n| !n.is_empty()).unwrap_or("pulsar-update");
    let dir = std::env::temp_dir().join(format!("pulsar-update-{}", update.version));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(name);
    std::fs::write(&path, &body).map_err(|e| e.to_string())?;
    Ok(path)
}

fn decode(b64: &str) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim()).map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

/// Tauri stores the minisign key and signature files base64-encoded.
fn verify(data: &[u8], signature: &str) -> Result<(), String> {
    let key = minisign_verify::PublicKey::decode(&decode(PUBKEY)?).map_err(|e| e.to_string())?;
    let sig = minisign_verify::Signature::decode(&decode(signature)?).map_err(|e| e.to_string())?;
    key.verify(data, &sig, true).map_err(|e| format!("bad signature: {e}"))
}

/// Downloads, verifies and installs the update. On success the caller
/// quits; for the Windows installer, it relaunches Pulsar itself, elsewhere
/// `relaunch` has to be called after the app let go of the single instance.
pub fn install(update: &Update) -> Result<Restart, String> {
    let kind = install_kind();
    let package = download(update)?;
    match kind {
        InstallKind::Nsis => {
            // Passive (progress only) and restart Pulsar when done, as with
            // the Tauri updater.
            std::process::Command::new(&package).args(["/P", "/R"]).spawn().map_err(|e| e.to_string())?;
            Ok(Restart::ByInstaller)
        }
        InstallKind::App(bundle) => {
            replace_app(&package, &bundle)?;
            Ok(Restart::Relaunch(bundle))
        }
        InstallKind::AppImage(path) => {
            replace_file(&package, &path)?;
            Ok(Restart::Relaunch(path))
        }
        InstallKind::Deb => {
            run_as_root(&["dpkg", "-i"], &package)?;
            Ok(Restart::Relaunch(std::env::current_exe().map_err(|e| e.to_string())?))
        }
        InstallKind::Rpm => {
            run_as_root(&["rpm", "-U", "--replacepkgs"], &package)?;
            Ok(Restart::Relaunch(std::env::current_exe().map_err(|e| e.to_string())?))
        }
        InstallKind::Unknown => Err("this copy of Pulsar wasn't installed by a package".into()),
    }
}

pub enum Restart {
    ByInstaller,
    /// Start this (binary, AppImage or .app bundle) once the app quit.
    Relaunch(PathBuf),
}

/// Starts the new version. It waits for this process to quit (see
/// `RESTARTED_ENV`).
pub fn relaunch(path: &Path) {
    let mut cmd = if path.extension().is_some_and(|e| e == "app") {
        let mut cmd = std::process::Command::new("open");
        cmd.arg("-n").arg(path).args(["--env", &format!("{RESTARTED_ENV}=1")]);
        cmd
    } else {
        std::process::Command::new(path)
    };
    let _ = cmd.env(RESTARTED_ENV, "1").spawn();
}

fn run_as_root(cmd: &[&str], package: &Path) -> Result<(), String> {
    let status = std::process::Command::new("pkexec")
        .args(cmd)
        .arg(package)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(format!("{} failed ({status})", cmd[0])) }
}

/// Swaps a file for the new one. Renaming over a running AppImage is fine:
/// the old one stays open until it quits.
fn replace_file(new: &Path, target: &Path) -> Result<(), String> {
    let tmp = target.with_extension("new");
    std::fs::copy(new, &tmp).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, target).map_err(|e| e.to_string())
}

/// macOS: unpacks `Pulsar.app.tar.gz` next to the bundle and swaps them.
#[cfg(target_os = "macos")]
fn replace_app(archive: &Path, bundle: &Path) -> Result<(), String> {
    let parent = bundle.parent().ok_or("bundle without a parent folder")?;
    let staging = parent.join(".pulsar-update");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    tar::Archive::new(flate2::read::GzDecoder::new(file)).unpack(&staging).map_err(|e| e.to_string())?;
    let new = std::fs::read_dir(&staging)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .ok_or("no .app in the update")?;
    let old = parent.join(".pulsar-old.app");
    let _ = std::fs::remove_dir_all(&old);
    std::fs::rename(bundle, &old).map_err(|e| e.to_string())?;
    if let Err(e) = std::fs::rename(&new, bundle) {
        let _ = std::fs::rename(&old, bundle);
        return Err(e.to_string());
    }
    let _ = std::fs::remove_dir_all(&old);
    let _ = std::fs::remove_dir_all(&staging);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn replace_app(_archive: &Path, _bundle: &Path) -> Result<(), String> {
    Err("not macOS".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The signature of the 0.4.6 AppImage, from its latest.json, must not
    /// verify for other data (and the key and signature must decode).
    #[test]
    fn rejects_wrong_data() {
        let sig = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVRTS9QZ1A1OXYrbjRwT1pldHlMR1krSUlPazhLd05xWmw1MUZ3TG05R2VoSkpSUGRzTHY1TWlyTDdSK2FFcyt3eTdMNDEwSng1SWNYL1JDSmkwRVVsOU04cUFXc2s2NHdRPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk0MjI5NTA2CWZpbGU6UHVsc2FyXzAuNC42X2FtZDY0LkFwcEltYWdlCXZlcnNpb246MC40LjYKTXhQaE9zTFVhWVhrMGw2MG1tS05PNy9TSmlXWTRPdzFUTC9TSUpxeWxlNU1vVlQwdmxhcC9WcTN5VTdXRmVCck9FU3ovTyswSkdNOGk0aExVVTBBQ1E9PQo=";
        let err = verify(b"not the AppImage", sig).unwrap_err();
        assert!(err.starts_with("bad signature"), "{err}");
    }

    /// Fetches the real latest.json (network).
    #[test]
    #[ignore]
    fn fetches_latest() {
        let latest: Latest = agent().get(LATEST).call().unwrap().body_mut().read_json().unwrap();
        assert!(semver::Version::parse(&latest.version).is_ok());
        assert!(!latest.platforms.is_empty());
    }

    /// A real package against its latest.json signature:
    /// `PULSAR_TEST_PACKAGE=<file> PULSAR_TEST_SIGNATURE=<base64> cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn accepts_signed_package() {
        let package = std::env::var("PULSAR_TEST_PACKAGE").expect("PULSAR_TEST_PACKAGE");
        let sig = std::env::var("PULSAR_TEST_SIGNATURE").expect("PULSAR_TEST_SIGNATURE");
        verify(&std::fs::read(package).unwrap(), &sig).unwrap();
    }
}
