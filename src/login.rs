//! One-shot per-user login launcher; no resident helper and no KeepAlive.
use std::{
    path::{Path, PathBuf},
    process::Command,
};

const BUNDLE_ID: &str = "es.amartin.kairosmonitor";
const LABEL: &str = "es.amartin.kairosmonitor.login";
const LEGACY: &str = BUNDLE_ID;

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").expect("user home"))
}
fn agent_path(label: &str) -> PathBuf {
    home().join(format!("Library/LaunchAgents/{label}.plist"))
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
fn launchctl(args: &[&str]) -> Result<(), String> {
    let out = Command::new("/bin/launchctl")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "No se pudo registrar el inicio de sesión: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}
fn disabled(output: &str, label: &str) -> bool {
    output.lines().any(|line| {
        line.split_once("=>").is_some_and(|(key, value)| {
            key.trim().trim_matches('"') == label
                && matches!(value.trim().trim_end_matches(';'), "true" | "disabled")
        })
    })
}
fn configured(label: &str) -> bool {
    if !agent_path(label).is_file() {
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
    out.status.success() && !disabled(&String::from_utf8_lossy(&out.stdout), label)
}
fn requested() -> bool {
    if agent_path(LABEL).exists() {
        configured(LABEL)
    } else {
        configured(LEGACY)
    }
}
pub fn enabled() -> bool {
    if agent_path(LABEL).exists() {
        configured(LABEL) && domain().is_ok_and(|domain| registered(&format!("{domain}/{LABEL}")))
    } else {
        configured(LEGACY)
    }
}
pub fn installed_bundle(exe: &Path, user_home: &Path) -> Option<PathBuf> {
    let bundle = exe.parent()?.parent()?.parent()?;
    let parent = bundle.parent()?;
    (exe.ends_with("Contents/MacOS/kairosmonitor")
        && bundle.extension().is_some_and(|s| s == "app")
        && (parent == Path::new("/Applications") || parent == user_home.join("Applications")))
    .then(|| bundle.to_path_buf())
}
pub fn current_bundle() -> Result<PathBuf, String> {
    installed_bundle(
        &std::env::current_exe().map_err(|e| e.to_string())?,
        &home(),
    )
    .ok_or("Copia KairosMonitor a Aplicaciones antes de activar el inicio de sesión".into())
}
pub fn available() -> bool {
    current_bundle().is_ok()
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
<key>ProgramArguments</key><array><string>{}</string><string>--launch-at-login</string></array>
<key>RunAtLoad</key><true/>
<key>LimitLoadToSessionType</key><string>Aqua</string>
<key>ProcessType</key><string>Interactive</string>
<key>AssociatedBundleIdentifiers</key><array><string>{BUNDLE_ID}</string></array>
</dict></plist>
"#,
        xml(&exe.to_string_lossy())
    )
}
fn registered(target: &str) -> bool {
    Command::new("/bin/launchctl")
        .args(["print", target])
        .output()
        .is_ok_and(|out| out.status.success())
}
pub fn set_enabled(enable: bool) -> Result<(), String> {
    let domain = domain()?;
    let target = format!("{domain}/{LABEL}");
    let path = agent_path(LABEL);
    if enable {
        let bundle = current_bundle()?;
        let contents = plist(&bundle.join("Contents/MacOS/kairosmonitor"));
        let old = std::fs::read(&path).ok();
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
        std::fs::write(&tmp, &contents).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
        launchctl(&["enable", &target])?;
        if old.as_deref() != Some(contents.as_bytes()) && registered(&target) {
            // Only the short-lived launcher belongs to this job, never the UI process.
            launchctl(&["bootout", &target])?;
        }
        if !registered(&target) {
            launchctl(&["bootstrap", &domain, &path.to_string_lossy()])?;
        }
        if !registered(&target) {
            return Err("macOS no ha registrado el inicio de sesión".into());
        }
    } else {
        launchctl(&["disable", &target])?;
        if registered(&target) {
            launchctl(&["bootout", &target])?;
        }
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
    }
    // Migrate the former direct-executable agent without killing an app it launched.
    if agent_path(LEGACY).exists() {
        launchctl(&["disable", &format!("{domain}/{LEGACY}")])?;
        std::fs::remove_file(agent_path(LEGACY)).map_err(|e| e.to_string())?;
    }
    Ok(())
}
/// Repairs an enabled registration after moving/replacing the installed bundle.
pub fn repair_if_enabled() -> Result<(), String> {
    if available() && requested() {
        set_enabled(true)?;
    }
    Ok(())
}
/// LaunchServices owns the app, so disabling the login launcher cannot kill it.
pub fn launch() -> Result<(), String> {
    use objc2_app_kit::NSRunningApplication;
    use objc2_foundation::NSString;
    let bundle = current_bundle()?;
    let existing = NSRunningApplication::runningApplicationsWithBundleIdentifier(
        &NSString::from_str(BUNDLE_ID),
    );
    if existing
        .iter()
        .any(|app| app.processIdentifier() != std::process::id() as i32)
    {
        return Ok(());
    }
    let result = Command::new("/usr/bin/open")
        .arg("-g")
        .arg(bundle)
        .status()
        .map_err(|e| e.to_string())?;
    if result.success() {
        Ok(())
    } else {
        Err("No se pudo abrir Kairos Monitor al iniciar sesión".into())
    }
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
    fn launch_path_is_xml_escaped_and_uses_one_shot_launcher() {
        let data = plist(Path::new(
            "/Users/A & B/Applications/KairosMonitor.app/Contents/MacOS/kairosmonitor",
        ));
        assert!(data.contains("A &amp; B"));
        assert!(data.contains("--launch-at-login"));
        assert!(!data.contains("KeepAlive"));
        assert!(!data.contains("StandardErrorPath"));
    }
    #[test]
    fn understands_old_and_current_launchctl_disabled_formats() {
        for value in ["true", "disabled"] {
            assert!(disabled(&format!("  \"{LABEL}\" => {value}"), LABEL));
        }
        for value in ["false", "enabled"] {
            assert!(!disabled(&format!("  \"{LABEL}\" => {value}"), LABEL));
        }
        assert!(!disabled(&format!("\"{LABEL}.other\" => disabled"), LABEL));
        assert!(!disabled("disabled services = {}", LABEL));
    }
}
