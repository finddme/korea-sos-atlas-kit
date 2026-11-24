// SPDX-License-Identifier: AGPL-3.0-only
//! On-demand offline Organic Maps launcher. No model or network is started.
use super::*;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
fn file_hash(path: &Path) -> io::Result<String> {
    use std::io::Read;
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
static RUNNING: AtomicBool = AtomicBool::new(false);

pub(super) fn destination(root: &Path, location: Option<&shelters::Location>) -> String {
    let Some(location) = location else {
        return "No verified coordinates for this record".into();
    };
    if !location.valid() {
        return "Invalid destination coordinates".into();
    }
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let root = root.canonicalize()?;
        let state = root
            .parent()
            .ok_or("Missing runtime parent")?
            .join("runtime/maps/state");
        fs::create_dir_all(&state)?;
        fs::write(state.join("destination.tmp"), serde_json::to_vec(location)?)?;
        fs::rename(
            state.join("destination.tmp"),
            state.join("destination.json"),
        )?;
        Ok(())
    })();
    if let Err(e) = result {
        return format!("Maps: {e}");
    }
    if RUNNING.load(Ordering::SeqCst) {
        "Destination sent to Organic Maps · Select a start on the map".into()
    } else {
        launch(root)
    }
}

const NOT_THIS_OS: &str = "지도 보기는 아직 Linux에서만 됩니다. Windows·macOS용 지도는 준비 중이며, 대피시설 목록과 주소 검색은 그대로 쓸 수 있습니다.";

fn command(root: &Path) -> Result<(Command, PathBuf), Box<dyn std::error::Error>> {
    if !cfg!(target_os = "linux") {
        return Err(NOT_THIS_OS.into());
    }
    let root = root.canonicalize()?;
    let manifest: Value = serde_json::from_slice(&fs::read(root.join("maps.json"))?)?;
    if manifest["approved"] != true {
        return Err("Map bundle has not passed offline verification".into());
    }
    if manifest["files"].as_array().is_none_or(|f| f.is_empty()) {
        return Err("Empty map bundle".into());
    }
    let relative = manifest["directory"]
        .as_str()
        .ok_or("Missing map directory")?;
    let dir = root.join(relative).canonicalize()?;
    if !dir.starts_with(&root) {
        return Err("Map directory escapes content library".into());
    }
    for item in manifest["files"].as_array().ok_or("Missing map files")? {
        let path = dir
            .join(item["file"].as_str().ok_or("Missing file name")?)
            .canonicalize()?;
        if !path.starts_with(&dir) || file_hash(&path)? != item["sha256"].as_str().unwrap_or("") {
            return Err("Map file integrity check failed".into());
        }
    }
    let runtime = root
        .parent()
        .ok_or("Missing runtime parent")?
        .join("runtime/maps")
        .canonicalize()?;
    let viewer = runtime
        .join(
            manifest["viewer"]["binary"]
                .as_str()
                .ok_or("Missing verified map viewer")?,
        )
        .canonicalize()?;
    if !viewer.starts_with(&runtime)
        || file_hash(&viewer)? != manifest["viewer"]["sha256"].as_str().unwrap_or("")
    {
        return Err("Map viewer integrity check failed".into());
    }
    let boot = runtime.join("bootstrap/usr");
    let executable = boot.join("bin/flatpak");
    if !executable.is_file() {
        return Err("Bundled Organic Maps runtime is missing".into());
    }
    let state = runtime.join("state");
    fs::create_dir_all(&state)?;
    if [
        "ime/usr/bin/fcitx5",
        "ime/usr/bin/dbus-run-session",
        "ime/usr/bin/dbus-daemon",
        "ime/usr/bin/dbus-send",
        "ime/usr/lib/x86_64-linux-gnu/fcitx5/libhangul.so",
        "ime/usr/share/libhangul/hanja/hanja.txt",
    ]
    .iter()
    .any(|path| !runtime.join(path).is_file())
    {
        return Err("Bundled Korean input runtime is missing".into());
    }
    let input_launcher = state.join("with-korean-input.sh");
    fs::write(
        state.join("input-session.conf"),
        include_str!("../integrations/organicmaps/input-session.conf"),
    )?;
    fs::write(
        &input_launcher,
        include_str!("../integrations/organicmaps/with-korean-input.sh"),
    )?;
    fs::write(
        state.join("fonts.conf"),
        r#"<?xml version="1.0"?><!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd"><fontconfig><include>/etc/fonts/fonts.conf</include><dir>/app/share/organicmaps/data/fonts</dir></fontconfig>"#,
    )?;
    let log = state.join("viewer.log");
    let output = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)?;
    let mut cmd = Command::new(executable);
    cmd.env("LD_LIBRARY_PATH", boot.join("lib/x86_64-linux-gnu"))
        .env("XDG_DATA_HOME", runtime.join("data"))
        .env("FLATPAK_BWRAP", boot.join("bin/bwrap"))
        .env("FLATPAK_DBUSPROXY", boot.join("bin/xdg-dbus-proxy"))
        .args([
            "run",
            "--user",
            "--unshare=network",
            "--socket=x11",
            "--nosocket=wayland",
        ])
        .arg(format!("--filesystem={}", root.parent().unwrap().display()));
    cmd.args([
        "--env=QT_QPA_PLATFORM=xcb",
        "--env=LANG=ko_KR.UTF-8",
        "--env=LANGUAGE=ko",
        "--command=env",
        "app.organicmaps.desktop",
    ])
    .arg(format!(
        "FONTCONFIG_FILE={}",
        state.join("fonts.conf").display()
    ))
    .arg(format!(
        "XDG_CONFIG_HOME={}",
        state.join("config").display()
    ))
    .arg(format!("XDG_CACHE_HOME={}", state.join("cache").display()))
    .arg(format!(
        "SOS_ATLAS_REQUEST={}",
        state.join("destination.json").display()
    ))
    .arg(format!(
        "LD_LIBRARY_PATH={}",
        runtime.join("ime/usr/lib/x86_64-linux-gnu").display()
    ))
    .arg(runtime.join("ime/usr/bin/dbus-run-session"))
    .arg(format!(
        "--config-file={}",
        state.join("input-session.conf").display()
    ))
    .arg(format!(
        "--dbus-daemon={}",
        runtime.join("ime/usr/bin/dbus-daemon").display()
    ))
    .args(["--", "sh"])
    .arg(input_launcher)
    .arg(&runtime)
    .arg(viewer)
    .arg("--resources_path=/app/share/organicmaps/data")
    .arg(format!("--data_path={}", dir.join("maps").display()))
    .arg("--lang=ko")
    .stdin(Stdio::null())
    .stdout(output.try_clone()?)
    .stderr(output);
    Ok((cmd, log))
}

pub(super) fn launch(root: &Path) -> String {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return "Organic Maps is already open".into();
    }
    match (|| -> Result<PathBuf, Box<dyn std::error::Error>> {
        let (mut cmd, log) = command(root)?;
        let mut child = cmd.spawn()?;
        thread::sleep(Duration::from_millis(300));
        if let Some(status) = child.try_wait()? {
            return Err(format!("Viewer exited ({status}); see {}", log.display()).into());
        }
        thread::spawn(move || {
            let _ = child.wait();
            RUNNING.store(false, Ordering::SeqCst);
        });
        Ok(log)
    })() {
        Ok(_) => "Organic Maps opened · Offline · Close its window to release memory".into(),
        Err(e) => {
            RUNNING.store(false, Ordering::SeqCst);
            format!("Maps: {e}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(target_os = "linux")]
    fn approved_bundle_is_offline_and_has_no_model_command() {
        let root = crate::project_root().join("dev/content-library");
        let (cmd, _) = command(&root).unwrap();
        let args: Vec<_> = cmd
            .get_args()
            .map(|x| x.to_string_lossy().into_owned())
            .collect();
        assert!(args.contains(&"--unshare=network".to_owned()));
        assert!(args.contains(&"app.organicmaps.desktop".to_owned()));
        assert!(!args.iter().any(|x| x.contains("llama")));
        assert!(args.iter().any(|x| x.starts_with("FONTCONFIG_FILE=")));
        assert!(args.iter().any(|x| x.ends_with("overlay/bin/OMaps")));
        assert!(args.contains(&"--resources_path=/app/share/organicmaps/data".to_owned()));
        assert!(args
            .iter()
            .any(|x| x.ends_with("ime/usr/bin/dbus-run-session")));
        assert!(args.iter().any(|x| x.ends_with("with-korean-input.sh")));
    }
    #[test]
    #[cfg(target_os = "linux")]
    fn refuses_modified_viewer_even_when_maps_match() {
        let dir = env::temp_dir().join(format!("sos-viewer-integrity-{}", std::process::id()));
        let root = dir.join("content-library");
        fs::create_dir_all(root.join("maps")).unwrap();
        fs::create_dir_all(dir.join("runtime/maps/overlay/bin")).unwrap();
        let map = root.join("maps/test.mwm");
        fs::write(&map, b"verified-map").unwrap();
        fs::write(
            dir.join("runtime/maps/overlay/bin/OMaps"),
            b"modified-viewer",
        )
        .unwrap();
        let manifest = serde_json::json!({
            "approved": true, "directory": "maps",
            "files": [{"file": "test.mwm", "sha256": file_hash(&map).unwrap()}],
            "viewer": {"binary": "overlay/bin/OMaps", "sha256": "wrong"}
        });
        fs::write(root.join("maps.json"), manifest.to_string()).unwrap();
        assert!(command(&root)
            .err()
            .unwrap()
            .to_string()
            .contains("viewer integrity"));
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    #[cfg(target_os = "linux")]
    fn refuses_unapproved_map_bundle() {
        let dir = env::temp_dir().join(format!("sos-map-gate-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("maps.json"), r#"{"approved":false}"#).unwrap();
        assert!(command(&dir)
            .err()
            .unwrap()
            .to_string()
            .contains("not passed"));
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn maps_tab_hit_target() {
        let (x, _) = &tabs(0)[3];
        assert_eq!(hit(*x + 1, 1, 120, 30, &App::default(), 0), Some((0, 3)));
    }
    #[test]
    #[cfg(not(target_os = "linux"))]
    fn other_os_gets_a_clear_notice_instead_of_a_viewer() {
        let root = crate::project_root().join("dev/content-library");
        assert_eq!(command(&root).err().unwrap().to_string(), NOT_THIS_OS);
    }
    #[test]
    #[cfg(target_os = "linux")]
    fn maps_hash_guard() {
        let dir = env::temp_dir().join(format!("sos-map-integrity-{}", std::process::id()));
        fs::create_dir_all(dir.join("item")).unwrap();
        fs::write(dir.join("item/map"), b"modified").unwrap();
        fs::write(
            dir.join("maps.json"),
            r#"{"approved":true,"directory":"item","files":[{"file":"map","sha256":"wrong"}]}"#,
        )
        .unwrap();
        assert!(command(&dir)
            .err()
            .unwrap()
            .to_string()
            .contains("integrity"));
        fs::remove_dir_all(dir).unwrap();
    }
}
