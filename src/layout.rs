// SPDX-License-Identifier: AGPL-3.0-only
//! Where the app finds its data. Paths are derived from the executable's
//! location so a copied folder (USB or local disk) works on any machine.
//!
//! Portable layout (release):          Development layout:
//!   korea-sos-atlas-kit[.exe]           <project>/target/release/korea-sos-atlas-kit
//!   data/assets.json                    <project>/dev/assets.json
//!   data/content-library/               <project>/dev/content-library/
//!   data/models/, data/runtime/         <project>/dev/models/, dev/runtime/
use serde_json::Value;
use std::path::{Path, PathBuf};

const CONTENT: &str = "content-library";

fn is_content_root(p: &Path) -> bool {
    p.join("approved.json").is_file()
}

/// Finds the content library next to the executable (`data/`), or in an
/// ancestor's `dev/` while developing. Never falls back to the build machine.
pub fn default_content_root() -> Result<PathBuf, String> {
    let exe =
        std::env::current_exe().map_err(|e| format!("실행 파일 위치를 알 수 없습니다: {e}"))?;
    let exe = plain(exe.canonicalize().unwrap_or(exe));
    let dir = exe.parent().ok_or("실행 파일 폴더를 알 수 없습니다")?;
    find_content_root(dir).ok_or_else(|| {
        format!(
            "자료 폴더를 찾지 못했습니다. 실행 파일 옆에 data/{CONTENT} 폴더가 있어야 합니다 (실행 위치: {}).",
            dir.display()
        )
    })
}

/// Windows `canonicalize` returns `\\?\C:\...`; some tools (llama.cpp model
/// loading, PowerShell `Uri`) mishandle that prefix, so drop it for drive paths.
pub(crate) fn plain(p: PathBuf) -> PathBuf {
    match p.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => p,
    }
}

pub fn find_content_root(exe_dir: &Path) -> Option<PathBuf> {
    let portable = exe_dir.join("data").join(CONTENT);
    if is_content_root(&portable) {
        return Some(portable);
    }
    exe_dir
        .ancestors()
        .map(|a| a.join("dev").join(CONTENT))
        .find(|p| is_content_root(p))
}

/// The folder that holds content-library, models, runtime and assets.json.
pub fn data_root(content_root: &Path) -> PathBuf {
    content_root
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    pub path: PathBuf,
    pub sha256: String,
}
#[derive(Debug, Clone, PartialEq)]
pub struct AiAssets {
    pub model: Asset,
    /// Further GGUF shards after `model` (the AI pack splits the model so each
    /// download stays under GitHub's 2 GB limit; llama.cpp loads the rest itself).
    pub model_parts: Vec<Asset>,
    pub engine: Asset,
    /// Shared libraries the engine loads (llama.cpp's `llama-server.exe` is a
    /// small launcher; the inference code lives in these files).
    pub engine_libs: Vec<Asset>,
}

pub fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn asset(data: &Path, v: &Value, what: &str) -> Result<Asset, String> {
    let file = v["file"]
        .as_str()
        .ok_or_else(|| format!("assets.json: {what} 파일 경로가 없습니다"))?;
    if Path::new(file).is_absolute() || file.split(['/', '\\']).any(|c| c == "..") {
        return Err(format!(
            "assets.json: {what} 경로는 data 폴더 안의 상대 경로여야 합니다"
        ));
    }
    let sha256 = v["sha256"].as_str().unwrap_or("").to_string();
    if sha256.len() != 64 || !sha256.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("assets.json: {what} SHA-256이 올바르지 않습니다"));
    }
    Ok(Asset {
        path: data.join(file),
        sha256,
    })
}

/// Reads the AI asset manifest for this OS/CPU. Environment variables
/// (SOS_AI_MODEL[_SHA256], SOS_LLAMA_SERVER[_SHA256]) override entries for
/// experiments. Does not check that files exist.
pub fn ai_assets(data: &Path) -> Result<AiAssets, String> {
    let manifest: Value = match std::fs::read(data.join("assets.json")) {
        Ok(b) => serde_json::from_slice(&b).map_err(|e| format!("assets.json 형식 오류: {e}"))?,
        Err(_) => Value::Null,
    };
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let pick =
        |entry: &Value, what: &str, path_var: &str, hash_var: &str| -> Result<Asset, String> {
            let mut a = match asset(data, entry, what) {
                Ok(a) => a,
                Err(e) if env(path_var).is_none() => return Err(e),
                Err(_) => Asset {
                    path: PathBuf::new(),
                    sha256: String::new(),
                },
            };
            if let Some(p) = env(path_var) {
                a.path = PathBuf::from(p);
            }
            if let Some(h) = env(hash_var) {
                a.sha256 = h;
            }
            Ok(a)
        };
    if manifest.is_null() && env("SOS_AI_MODEL").is_none() {
        return Err("AI 자산 목록(assets.json)이 없습니다. AI를 쓸 수 없는 구성입니다.".into());
    }
    let model = pick(
        &manifest["model"],
        "모델",
        "SOS_AI_MODEL",
        "SOS_AI_MODEL_SHA256",
    )?;
    let model_parts = if env("SOS_AI_MODEL").is_some() {
        vec![]
    } else {
        manifest["model"]["parts"]
            .as_array()
            .map(|parts| {
                parts
                    .iter()
                    .map(|p| asset(data, p, "모델 조각"))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default()
    };
    let entry = &manifest["engine"][platform()];
    let what = format!("{} 추론 엔진", platform());
    let engine = pick(entry, &what, "SOS_LLAMA_SERVER", "SOS_LLAMA_SERVER_SHA256")?;
    // An overridden engine has its own libraries; the manifest's list would not match.
    let engine_libs = if env("SOS_LLAMA_SERVER").is_some() {
        vec![]
    } else {
        entry["libs"]
            .as_object()
            .map(|libs| {
                libs.iter()
                    .map(|(file, sha)| {
                        asset(
                            data,
                            &serde_json::json!({"file": file, "sha256": sha}),
                            &what,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default()
    };
    Ok(AiAssets {
        model,
        model_parts,
        engine,
        engine_libs,
    })
}

pub const AI_PACK_MISSING: &str = "AI 추가팩이 없습니다. 배포 페이지(Releases)에서 AI 추가팩 zip 2개(ai-pack-1, ai-pack-2)를 받아 다운로드 폴더나 이 키트 폴더에 두고 p를 누르면 앱이 설치합니다. 자료 검색·대피시설·지도는 그대로 쓸 수 있습니다.";

/// The manifest is readable but some model or engine file is not there yet:
/// the AI pack can be installed from its zips (see `ai_pack`).
pub fn ai_files_missing(data: &Path) -> bool {
    ai_assets(data).is_ok_and(|a| {
        std::iter::once(&a.model)
            .chain(&a.model_parts)
            .chain(std::iter::once(&a.engine))
            .chain(&a.engine_libs)
            .any(|x| !x.path.is_file())
    })
}

/// Cheap startup check: manifest readable and both files present. Hashes are
/// verified later, when the user turns AI on.
pub fn ai_unavailable_reason(data: &Path) -> Option<String> {
    match ai_assets(data) {
        Err(e) => Some(e),
        Ok(a) if !a.model.path.is_file() || a.model_parts.iter().any(|p| !p.path.is_file()) => {
            Some(AI_PACK_MISSING.into())
        }
        Ok(a) if !a.engine.path.is_file() || a.engine_libs.iter().any(|l| !l.path.is_file()) => {
            Some(format!(
            "이 운영체제({})용 추론 엔진이 없습니다. 자료 검색과 열람은 그대로 사용할 수 있습니다.",
            platform()
        ))
        }
        Ok(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ksak-layout-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn portable_data_folder_wins_and_dev_ancestor_is_found() {
        let base = tmp("find");
        let app = base.join("app");
        fs::create_dir_all(app.join("data/content-library")).unwrap();
        fs::write(app.join("data/content-library/approved.json"), "[]").unwrap();
        assert_eq!(
            find_content_root(&app),
            Some(app.join("data/content-library"))
        );
        let release = base.join("proj/target/release");
        fs::create_dir_all(&release).unwrap();
        fs::create_dir_all(base.join("proj/dev/content-library")).unwrap();
        fs::write(base.join("proj/dev/content-library/approved.json"), "[]").unwrap();
        assert_eq!(
            find_content_root(&release),
            Some(base.join("proj/dev/content-library"))
        );
        assert_eq!(find_content_root(&base.join("elsewhere")), None);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn manifest_selects_this_platform_and_rejects_escapes() {
        let data = tmp("assets");
        let h = "a".repeat(64);
        let manifest = serde_json::json!({
            "version": 1,
            "model": {"file": "models/m.gguf", "sha256": h, "parts": [{"file": "models/m-2.gguf", "sha256": h}]},
            "engine": {platform(): {"file": "runtime/e", "sha256": h, "libs": {"runtime/l.dll": h}}}
        });
        fs::write(data.join("assets.json"), manifest.to_string()).unwrap();
        let a = ai_assets(&data).unwrap();
        assert_eq!(a.model.path, data.join("models/m.gguf"));
        assert_eq!(a.engine.path, data.join("runtime/e"));
        assert_eq!(a.model_parts[0].path, data.join("models/m-2.gguf"));
        assert_eq!(ai_unavailable_reason(&data).unwrap(), AI_PACK_MISSING);
        fs::create_dir_all(data.join("models")).unwrap();
        fs::write(data.join("models/m.gguf"), "x").unwrap();
        assert_eq!(ai_unavailable_reason(&data).unwrap(), AI_PACK_MISSING);
        fs::write(data.join("models/m-2.gguf"), "x").unwrap();
        assert!(ai_unavailable_reason(&data).unwrap().contains("추론 엔진"));
        fs::create_dir_all(data.join("runtime")).unwrap();
        fs::write(data.join("runtime/e"), "x").unwrap();
        assert_eq!(a.engine_libs[0].path, data.join("runtime/l.dll"));
        assert!(ai_unavailable_reason(&data).unwrap().contains("추론 엔진"));
        fs::write(data.join("runtime/l.dll"), "x").unwrap();
        assert_eq!(ai_unavailable_reason(&data), None);

        let bad = serde_json::json!({"model": {"file": "../m", "sha256": h}});
        fs::write(data.join("assets.json"), bad.to_string()).unwrap();
        assert!(ai_assets(&data).unwrap_err().contains("상대 경로"));
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn verbatim_drive_prefix_is_dropped() {
        assert_eq!(
            plain(PathBuf::from(r"\\?\C:\app")),
            PathBuf::from(r"C:\app")
        );
        assert_eq!(
            plain(PathBuf::from(r"\\?\UNC\srv\x")),
            PathBuf::from(r"\\?\UNC\srv\x")
        );
        assert_eq!(plain(PathBuf::from("/usr/bin")), PathBuf::from("/usr/bin"));
    }

    #[test]
    fn light_build_without_manifest_is_unavailable_not_an_error() {
        let data = tmp("light");
        if std::env::var_os("SOS_AI_MODEL").is_none() {
            assert!(ai_unavailable_reason(&data)
                .unwrap()
                .contains("assets.json"));
        }
        fs::remove_dir_all(data).unwrap();
    }
}
