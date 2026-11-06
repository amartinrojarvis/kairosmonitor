//! Per-user login item, compatible with macOS 12 and the previous installer.
//! Do not bootstrap/bootout here: that would start a second app or kill this one.
use std::{
    path::{Path, PathBuf},
    process::Command,
};

const LABEL: &str = "es.amartin.kairosmonitor";

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").expect("user home"))
}
fn agent_path() -> PathBuf {
    home().join(format!("Library/LaunchAgents/{LABEL}.plist"))
}
fn domain() -> Result<String, String> {
    let out = Command::new("/usr/bin/id")
        .arg("-u")
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("No se pudo identificar la sesión de usuario".into());
    }
    Ok(format!(
        "gui/{}",
        String::from_utf8_lossy(&out.stdout).trim()
    ))
}
pub fn enabled() -> bool {
    if !agent_path().exists() {
        return false;
    }
    let Ok(domain) = domain() else {
        return false;
    };
    let Ok(out) = Command::new("/bin/launchctl")
        .args(["print-disabled", &domain])
        .output()
    else {
        return false;
    };
    !String::from_utf8_lossy(&out.stdout)
        .lines()
        .any(|line| line.contains(&format!("\"{LABEL}\"")) && line.contains("=> true"))
}
fn installed_bundle(exe: &Path, user_home: &Path) -> Option<PathBuf> {
    let bundle = exe.parent()?.parent()?.parent()?;
    let parent = bundle.parent()?;
    (exe.ends_with("Contents/MacOS/kairosmonitor")
        && bundle.extension().is_some_and(|s| s == "app")
        && (parent == Path::new("/Applications") || parent == user_home.join("Applications")))
    .then(|| bundle.to_path_buf())
}
pub fn available() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| installed_bundle(&p, &home()))
        .is_some()
}
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn plist(exe: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>{LABEL}</string>
<key>ProgramArguments</key><array><string>{}</string></array>
<key>RunAtLoad</key><true/>
<key>ProcessType</key><string>Interactive</string>
<key>AssociatedBundleIdentifiers</key><array><string>{LABEL}</string></array>
</dict></plist>
"#,
        xml(&exe.to_string_lossy())
    )
}
pub fn set_enabled(enable: bool) -> Result<(), String> {
    let path = agent_path();
    if !enable && !path.exists() {
        return Ok(());
    }
    let old = std::fs::read(&path).ok();
    if enable {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        installed_bundle(&exe, &home())
            .ok_or("Copia KairosMonitor a Aplicaciones antes de activar el inicio de sesión")?;
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
        std::fs::write(&tmp, plist(&exe)).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    }
    if !enable {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    let result = (|| {
        let target = format!("{}/{LABEL}", domain()?);
        let out = Command::new("/bin/launchctl")
            .args([if enable { "enable" } else { "disable" }, &target])
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!(
                "No se pudo cambiar el inicio de sesión: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(())
    })();
    if result.is_err() {
        if let Some(bytes) = old {
            let _ = std::fs::write(&path, bytes);
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_installed_apps_can_register() {
        let h = Path::new("/Users/test");
        for p in [
            "/Applications/KairosMonitor.app/Contents/MacOS/kairosmonitor",
            "/Users/test/Applications/KairosMonitor.app/Contents/MacOS/kairosmonitor",
        ] {
            assert!(installed_bundle(Path::new(p), h).is_some());
        }
        for p in [
            "/Volumes/KairosMonitor/KairosMonitor.app/Contents/MacOS/kairosmonitor",
            "/private/AppTranslocation/KairosMonitor.app/Contents/MacOS/kairosmonitor",
            "/tmp/target/debug/kairosmonitor",
        ] {
            assert!(installed_bundle(Path::new(p), h).is_none());
        }
    }
    #[test]
    fn launch_path_is_xml_escaped() {
        let data = plist(Path::new(
            "/Users/A & B/Applications/KairosMonitor.app/Contents/MacOS/kairosmonitor",
        ));
        assert!(data.contains("A &amp; B"));
        assert!(!data.contains("KeepAlive"));
    }
}
