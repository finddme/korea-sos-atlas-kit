// SPDX-License-Identifier: AGPL-3.0-only
//! Windows file properties (VERSIONINFO): product name and version in the
//! .exe, which code signing requires to be set and the same in every build.
//!
//! The resource is compiled with `llvm-rc` (shipped with llvm-mingw; also in
//! LLVM for Windows) or `rc` (Windows SDK). Set `RC` to choose one. Without a
//! resource compiler the build continues without file properties, unless
//! `KSAK_REQUIRE_VERSIONINFO=1` (release and CI builds) makes that an error.
use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=RC");
    println!("cargo:rerun-if-env-changed=KSAK_REQUIRE_VERSIONINFO");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let required = env::var("KSAK_REQUIRE_VERSIONINFO").as_deref() == Ok("1");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let nums: Vec<u16> = version
        .split(['.', '-', '+'])
        .take(3)
        .map(|n| n.parse().unwrap_or(0))
        .collect();
    let (a, b, c) = (nums[0], nums.get(1).copied().unwrap_or(0), nums.get(2).copied().unwrap_or(0));
    let rc = format!(
        r#"#pragma code_page(65001)
1 VERSIONINFO
FILEVERSION {a},{b},{c},0
PRODUCTVERSION {a},{b},{c},0
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "ProductName", "Korea SOS Atlas Kit"
      VALUE "ProductVersion", "{version}"
      VALUE "FileDescription", "Korea SOS Atlas Kit"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "korea-sos-atlas-kit"
      VALUE "OriginalFilename", "korea-sos-atlas-kit.exe"
      VALUE "CompanyName", "finddme"
      VALUE "LegalCopyright", "finddme. AGPL-3.0-only. https://github.com/finddme/korea-sos-atlas-kit"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x0409, 1200
  END
END
"#
    );
    let src = out.join("versioninfo.rc");
    let res = out.join("versioninfo.res");
    fs::write(&src, rc).unwrap();

    let candidates: Vec<String> = match env::var("RC") {
        Ok(rc) if !rc.is_empty() => vec![rc],
        _ => vec!["llvm-rc".into(), "rc".into()],
    };
    for tool in &candidates {
        let ok = Command::new(tool)
            .arg("/nologo")
            .arg("/fo")
            .arg(&res)
            .arg(&src)
            .status()
            .is_ok_and(|s| s.success());
        if ok {
            // Both link.exe and lld (MSVC and MinGW modes) take a .res input.
            println!("cargo:rustc-link-arg-bins={}", res.display());
            return;
        }
    }
    let msg = format!(
        "no resource compiler ({}) found: the .exe gets no product name/version",
        candidates.join(", ")
    );
    if required {
        panic!("{msg}; set RC to llvm-rc or rc");
    }
    println!("cargo:warning={msg}");
}
