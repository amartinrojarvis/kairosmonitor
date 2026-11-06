//! GitHub releases, bounded downloads and a one-shot transactional installer.
use crate::{config::Config, login, network::Client};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
const API: &str = "https://api.github.com/repos/amartinrojarvis/kairosmonitor/releases/latest";
const DOWNLOADS: &str = "https://github.com/amartinrojarvis/kairosmonitor/releases/download/";
const MAX_DOWNLOAD: usize = 16 * 1024 * 1024;
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn version(input: &str) -> Option<[u64; 3]> {
    let mut result = [0; 3];
    let parts = input
        .strip_prefix('v')
        .unwrap_or(input)
        .split('.')
        .collect::<Vec<_>>();
    if parts.len() != 3 {
        return None;
    }
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        result[index] = part.parse().ok()?;
    }
    Some(result)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    pub version: String,
    pub url: String,
    pub digest: String,
    pub size: usize,
}
impl Release {
    fn validate(&self) -> Result<(), String> {
        if version(&self.version).is_none()
            || self.url != format!("{DOWNLOADS}v{0}/KairosMonitor-{0}-arm64.dmg", self.version)
            || self.digest.len() != 64
            || !self.digest.bytes().all(|b| b.is_ascii_hexdigit())
            || self.size == 0
            || self.size > MAX_DOWNLOAD
        {
            return Err("La publicación no contiene un paquete arm64 verificable".into());
        }
        Ok(())
    }
}
pub fn parse_release(bytes: &[u8], current: &str) -> Result<Option<Release>, String> {
    let data: Value = serde_json::from_slice(bytes).map_err(|_| "Respuesta de GitHub no válida")?;
    if data["draft"] == true || data["prerelease"] == true {
        return Ok(None);
    }
    let tag = data["tag_name"]
        .as_str()
        .ok_or("La publicación no tiene versión")?;
    let next = version(tag).ok_or("La publicación no tiene una versión estable válida")?;
    if next <= version(current).ok_or("Versión instalada no válida")? {
        return Ok(None);
    }
    let name = tag.strip_prefix('v').unwrap_or(tag);
    let expected = format!("KairosMonitor-{name}-arm64.dmg");
    let asset = data["assets"]
        .as_array()
        .and_then(|assets| assets.iter().find(|a| a["name"] == expected))
        .ok_or("La nueva versión todavía no tiene un DMG para Apple Silicon")?;
    let release = Release {
        version: name.into(),
        url: asset["browser_download_url"]
            .as_str()
            .unwrap_or_default()
            .into(),
        digest: asset["digest"]
            .as_str()
            .and_then(|s| s.strip_prefix("sha256:"))
            .unwrap_or_default()
            .into(),
        size: asset["size"]
            .as_u64()
            .and_then(|s| usize::try_from(s).ok())
            .unwrap_or(0),
    };
    release.validate()?;
    Ok(Some(release))
}
pub fn check(client: &Client) -> Result<Option<Release>, String> {
    parse_release(&client.get_bounded(API, 15, 2 * 1024 * 1024)?, CURRENT)
}
#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub automatic: bool,
    pub checked_at: u64,
    pub available: Option<Release>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            automatic: true,
            checked_at: 0,
            available: None,
        }
    }
}
impl Preferences {
    fn path() -> PathBuf {
        Config::path().with_file_name("updates.json")
    }
    pub fn load() -> Self {
        let mut prefs: Self = fs::read(Self::path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        if prefs
            .available
            .as_ref()
            .is_some_and(|r| r.validate().is_err() || version(&r.version) <= version(CURRENT))
        {
            prefs.available = None;
        }
        prefs
    }
    pub fn save(&self) -> Result<(), String> {
        let path = Self::path();
        fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
        fs::write(&tmp, serde_json::to_vec(self).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        fs::rename(tmp, path).map_err(|e| e.to_string())
    }
    pub fn due(&self) -> bool {
        self.automatic
            && (now().saturating_sub(self.checked_at) >= 86400 || self.checked_at > now())
    }
    pub fn mark_checked(&mut self) {
        self.checked_at = now();
    }
}
fn command(program: &str, args: &[&std::ffi::OsStr]) -> Result<(), String> {
    let out = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{}: {}",
            Path::new(program).file_name().unwrap().to_string_lossy(),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}
fn sha256(bytes: &[u8]) -> String {
    unsafe extern "C" {
        fn CC_SHA256(data: *const u8, len: u32, md: *mut u8) -> *mut u8;
    }
    let mut digest = [0_u8; 32];
    // CommonCrypto is part of libSystem; the download is bounded well below u32::MAX.
    unsafe {
        CC_SHA256(bytes.as_ptr(), bytes.len() as u32, digest.as_mut_ptr());
    }
    digest.iter().map(|b| format!("{b:02x}")).collect()
}
fn verify_download(bytes: &[u8], release: &Release) -> Result<(), String> {
    if bytes.len() != release.size || sha256(bytes) != release.digest.to_ascii_lowercase() {
        Err(
            "La descarga no coincide con el SHA-256 publicado. No se ha cambiado la aplicación."
                .into(),
        )
    } else {
        Ok(())
    }
}
fn verify_bundle(bundle: &Path, release: &Release) -> Result<(), String> {
    command(
        "/usr/bin/codesign",
        &["--verify".as_ref(), "--strict".as_ref(), bundle.as_os_str()],
    )?;
    command(
        "/usr/bin/lipo",
        &[
            "-verify_arch".as_ref(),
            "arm64".as_ref(),
            bundle.join("Contents/MacOS/kairosmonitor").as_os_str(),
        ],
    )?;
    let info = Command::new("/usr/bin/plutil")
        .args(["-convert", "json", "-o", "-"])
        .arg(bundle.join("Contents/Info.plist"))
        .output()
        .map_err(|e| e.to_string())?;
    if !info.status.success() {
        return Err("El paquete no tiene un Info.plist válido".into());
    }
    let data: Value = serde_json::from_slice(&info.stdout).map_err(|e| e.to_string())?;
    if data["CFBundleIdentifier"] != "es.amartin.kairosmonitor"
        || data["CFBundleShortVersionString"] != release.version
        || data["CFBundleExecutable"] != "kairosmonitor"
    {
        return Err("El paquete no corresponde a esta aplicación o versión".into());
    }
    let os = Command::new("/usr/bin/sw_vers")
        .arg("-productVersion")
        .output()
        .map_err(|e| e.to_string())?;
    let normalize = |s: &str| {
        let mut v = s
            .trim()
            .split('.')
            .map(|n| n.parse::<u64>().unwrap_or(u64::MAX))
            .collect::<Vec<_>>();
        v.resize(3, 0);
        v
    };
    if normalize(
        data["LSMinimumSystemVersion"]
            .as_str()
            .ok_or("Versión mínima de macOS ausente")?,
    ) > normalize(&String::from_utf8_lossy(&os.stdout))
    {
        return Err("La nueva versión requiere una versión más reciente de macOS".into());
    }
    Ok(())
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Prepared {
    workspace: PathBuf,
    staged: PathBuf,
    target: PathBuf,
    backup: PathBuf,
    parent_pid: u32,
}
impl Prepared {
    fn validate_paths(&self) -> Result<(), String> {
        let home = PathBuf::from(std::env::var_os("HOME").ok_or("Usuario no disponible")?);
        let cache = home.join("Library/Caches/KairosMonitor/updates");
        let name = self
            .workspace
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("Ruta de actualización inválida")?;
        let stamp = name
            .strip_prefix("update-")
            .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .ok_or("Ruta de actualización inválida")?;
        if self.workspace.parent() != Some(cache.as_path())
            || login::installed_bundle(&self.target.join("Contents/MacOS/kairosmonitor"), &home)
                .as_ref()
                != Some(&self.target)
            || self.staged
                != self
                    .target
                    .with_file_name(format!(".KairosMonitor-staged-{stamp}.app"))
            || self.backup
                != self
                    .target
                    .with_file_name(format!(".KairosMonitor-backup-{stamp}.app"))
            || self.workspace.is_symlink()
            || self.target.is_symlink()
            || self.staged.is_symlink()
            || self.backup.is_symlink()
            || self.parent_pid <= 1
        {
            return Err("Rutas de instalación inválidas".into());
        }
        Ok(())
    }
}
impl Drop for Prepared {
    fn drop(&mut self) {
        if self.validate_paths().is_err() {
            return;
        }
        let _ = fs::remove_dir_all(&self.staged);
        let _ = fs::remove_dir_all(&self.workspace);
    }
}
pub fn prepare(client: &Client, release: &Release, target: PathBuf) -> Result<Prepared, String> {
    release.validate()?;
    if version(&release.version) <= version(CURRENT) {
        return Err("La versión ya está instalada".into());
    }
    let home = PathBuf::from(std::env::var_os("HOME").ok_or("Usuario no disponible")?);
    if login::installed_bundle(&target.join("Contents/MacOS/kairosmonitor"), &home).as_ref()
        != Some(&target)
        || target.is_symlink()
    {
        return Err("Instala la app en Aplicaciones antes de actualizarla".into());
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let cache = home.join("Library/Caches/KairosMonitor/updates");
    fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let workspace = cache.join(format!("update-{stamp}"));
    fs::create_dir(&workspace).map_err(|e| e.to_string())?;
    let prepared = Prepared {
        workspace,
        staged: target.with_file_name(format!(".KairosMonitor-staged-{stamp}.app")),
        backup: target.with_file_name(format!(".KairosMonitor-backup-{stamp}.app")),
        target,
        parent_pid: std::process::id(),
    };
    // Probe write access before downloading. No elevation or permission changes.
    fs::create_dir(&prepared.staged).map_err(|_| "No se puede escribir en Aplicaciones. Instala la app en ~/Applications para actualizar desde ella.")?;
    let bytes = client.get_bounded(&release.url, 120, MAX_DOWNLOAD)?;
    verify_download(&bytes, release)?;
    let dmg = prepared.workspace.join("update.dmg");
    fs::write(&dmg, bytes).map_err(|e| e.to_string())?;
    let mount = prepared.workspace.join("volume");
    fs::create_dir(&mount).map_err(|e| e.to_string())?;
    command(
        "/usr/bin/hdiutil",
        &[
            "attach".as_ref(),
            "-readonly".as_ref(),
            "-nobrowse".as_ref(),
            "-mountpoint".as_ref(),
            mount.as_os_str(),
            dmg.as_os_str(),
        ],
    )?;
    let source = mount.join("KairosMonitor.app");
    let result = verify_bundle(&source, release).and_then(|_| {
        command(
            "/usr/bin/ditto",
            &[source.as_os_str(), prepared.staged.as_os_str()],
        )
    });
    let detached = command("/usr/bin/hdiutil", &["detach".as_ref(), mount.as_os_str()]);
    result?;
    detached?;
    verify_bundle(&prepared.staged, release)?;
    Ok(prepared)
}
pub fn start_install(prepared: Prepared) -> Result<(), String> {
    prepared.validate_paths()?;
    let helper = prepared.workspace.join("installer");
    fs::copy(std::env::current_exe().map_err(|e| e.to_string())?, &helper)
        .map_err(|e| e.to_string())?;
    let manifest = prepared.workspace.join("install.json");
    fs::write(
        &manifest,
        serde_json::to_vec(&prepared).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Command::new(helper)
        .arg("--finish-update")
        .arg(manifest)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    // The helper now owns cleanup; the UI can safely terminate.
    std::mem::forget(prepared);
    Ok(())
}
fn replace(
    target: &Path,
    staged: &Path,
    backup: &Path,
    open: impl Fn(&Path) -> Result<(), String>,
) -> Result<(), String> {
    if backup.exists() {
        return Err("Ya existe una copia de recuperación".into());
    }
    fs::rename(target, backup).map_err(|e| e.to_string())?;
    if let Err(e) = fs::rename(staged, target) {
        fs::rename(backup, target).map_err(|restore| format!("{e}; recuperación: {restore}"))?;
        return Err(e.to_string());
    }
    if let Err(e) = open(target) {
        fs::rename(target, staged).map_err(|restore| format!("{e}; recuperación: {restore}"))?;
        fs::rename(backup, target).map_err(|restore| format!("{e}; recuperación: {restore}"))?;
        let _ = open(target);
        return Err(e);
    }
    let _ = fs::remove_dir_all(backup);
    Ok(())
}
pub fn finish_install(manifest: &Path) -> Result<(), String> {
    let prepared: Prepared =
        serde_json::from_slice(&fs::read(manifest).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    prepared.validate_paths()?;
    let result = (|| {
        for _ in 0..150 {
            if !Command::new("/bin/kill")
                .args(["-0", &prepared.parent_pid.to_string()])
                .stderr(Stdio::null())
                .status()
                .map_err(|e| e.to_string())?
                .success()
            {
                return replace(
                    &prepared.target,
                    &prepared.staged,
                    &prepared.backup,
                    |target| command("/usr/bin/open", &["-g".as_ref(), target.as_os_str()]),
                );
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        Err("La app no se cerró a tiempo; se ha conservado la versión anterior".into())
    })();
    if result.is_err()
        && prepared
            .target
            .join("Contents/MacOS/kairosmonitor")
            .is_file()
    {
        let _ = command(
            "/usr/bin/open",
            &["-g".as_ref(), prepared.target.as_os_str()],
        );
    }
    let result_path = Config::path().with_file_name("update-result.txt");
    let _ = fs::write(
        result_path,
        match &result {
            Ok(()) => "Actualización instalada correctamente".to_owned(),
            Err(e) => format!("No se pudo actualizar: {e}"),
        },
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release() -> Value {
        serde_json::json!({"tag_name":"v0.10.0", "draft":false, "prerelease":false, "assets":[{"name":"KairosMonitor-0.10.0-arm64.dmg", "browser_download_url":format!("{DOWNLOADS}v0.10.0/KairosMonitor-0.10.0-arm64.dmg"),"size":3,"digest":format!("sha256:{}",sha256(b"abc"))}]})
    }
    #[test]
    fn compares_numeric_versions_and_rejects_untrusted_assets() {
        let mut data = release();
        assert!(
            parse_release(&serde_json::to_vec(&data).unwrap(), "0.9.9")
                .unwrap()
                .is_some()
        );
        assert!(
            parse_release(&serde_json::to_vec(&data).unwrap(), "0.10.0")
                .unwrap()
                .is_none()
        );
        assert!(
            parse_release(&serde_json::to_vec(&data).unwrap(), "1.0.0")
                .unwrap()
                .is_none()
        );
        data["prerelease"] = true.into();
        assert!(
            parse_release(&serde_json::to_vec(&data).unwrap(), "0.9.9")
                .unwrap()
                .is_none()
        );
        data["prerelease"] = false.into();
        data["assets"][0]["browser_download_url"] = "https://example.com/malicious.dmg".into();
        assert!(parse_release(&serde_json::to_vec(&data).unwrap(), "0.9.9").is_err());
        data = release();
        data["assets"][0]["digest"] = Value::Null;
        assert!(parse_release(&serde_json::to_vec(&data).unwrap(), "0.9.9").is_err());
    }
    #[test]
    fn corrupted_download_never_reaches_installer() {
        assert_eq!(
            sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let r = parse_release(&serde_json::to_vec(&release()).unwrap(), "0.9.9")
            .unwrap()
            .unwrap();
        assert!(verify_download(b"abc", &r).is_ok());
        assert!(verify_download(b"abd", &r).is_err());
        assert!(verify_download(b"abcd", &r).is_err());
    }
    #[test]
    fn failed_install_restores_original_bundle() {
        let root = std::env::temp_dir().join(format!("kairos-update-test-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let target = root.join("app");
        let staged = root.join("staged");
        let backup = root.join("backup");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("version"), "old").unwrap();
        assert!(replace(&target, &staged, &backup, |_| Ok(())).is_err());
        assert_eq!(fs::read_to_string(target.join("version")).unwrap(), "old");
        fs::create_dir(&staged).unwrap();
        fs::write(staged.join("version"), "new").unwrap();
        assert!(replace(&target, &staged, &backup, |_| Err("launch failure".into())).is_err());
        assert_eq!(fs::read_to_string(target.join("version")).unwrap(), "old");
        replace(&target, &staged, &backup, |_| Ok(())).unwrap();
        assert_eq!(fs::read_to_string(target.join("version")).unwrap(), "new");
        assert!(!backup.exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn automatic_checks_are_at_most_daily() {
        let mut p = Preferences::default();
        assert!(p.due());
        p.mark_checked();
        assert!(!p.due());
        p.checked_at = 0;
        p.automatic = false;
        assert!(!p.due());
    }
}
