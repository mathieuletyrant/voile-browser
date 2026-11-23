//! `cargo run -p voile-bundle -- [--release] [-o dossier]`
//!
//! Compile `voile` et `voile_helper`, puis assemble une application
//! exécutable avec les binaires CEF (utilitaires de `cef::build_util`).
//!
//! * macOS : `<dossier>/Voile.app`
//! * Linux : `<dossier>/voile` et les ressources Chromium à côté
//! * Windows : `<dossier>\voile.exe` et les ressources Chromium à côté

use std::path::{Path, PathBuf};
use std::process::Command;

const APP: &str = "voile";
#[cfg(target_os = "macos")]
const HELPER: &str = "voile_helper";

fn main() -> anyhow::Result<()> {
    let mut release = false;
    let mut output = PathBuf::from("target/bundle");
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--release" => release = true,
            "-o" | "--output" => {
                output = args
                    .next()
                    .map(PathBuf::from)
                    .ok_or_else(|| anyhow::anyhow!("-o attend un dossier"))?
            }
            "-h" | "--help" => {
                println!("voile-bundle [--release] [-o dossier]");
                return Ok(());
            }
            other => anyhow::bail!("argument inconnu : {other}"),
        }
    }
    std::fs::create_dir_all(&output)?;
    let path = bundle(&output, release)?;
    println!("{}", path.display());
    Ok(())
}

/// Racine du workspace (deux niveaux au-dessus de ce crate).
fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Dossier des binaires compilés (`target/release` ou `target/debug`).
fn target_dir(release: bool) -> PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace().join("target"));
    target.join(if release { "release" } else { "debug" })
}

#[cfg_attr(target_os = "windows", allow(dead_code))]
fn cargo_build(release: bool) -> anyhow::Result<()> {
    let mut cmd = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cmd.current_dir(workspace())
        .args(["build", "-p", "voile-app", "--bins"]);
    if release {
        cmd.arg("--release");
    }
    let status = cmd.status()?;
    anyhow::ensure!(status.success(), "la compilation de voile-app a échoué");
    Ok(())
}

#[cfg(target_os = "macos")]
fn bundle(output: &Path, release: bool) -> anyhow::Result<PathBuf> {
    use cef::build_util::mac::{bundle, BundleInfo};

    cargo_build(release)?;
    let version = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;
    let info = BundleInfo::new("Voile", "cc.cheztomy.voile", "Voile", "en", version);
    let resources = workspace().join("crates/voile-app/resources");

    // cef-rs nomme le bundle d'après l'exécutable (`voile.app`). L'exécutable
    // et les helpers doivent garder ce nom, seul le dossier est renommé. Les
    // volumes macOS ignorant la casse, `voile.app` et `Voile.app` y sont le
    // même chemin : on construit dans un dossier à part.
    let staging = output.join(".staging");
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }
    std::fs::create_dir_all(&staging)?;
    let built = bundle(
        &staging,
        &target_dir(release),
        APP,
        HELPER,
        Some(resources),
        info,
    )?;
    let app = output.join("Voile.app");
    if app.exists() {
        std::fs::remove_dir_all(&app)?;
    }
    std::fs::rename(&built, &app)?;
    std::fs::remove_dir_all(&staging)?;
    Ok(app)
}

#[cfg(target_os = "linux")]
fn bundle(output: &Path, release: bool) -> anyhow::Result<PathBuf> {
    cargo_build(release)?;
    Ok(cef::build_util::linux::bundle(
        output,
        &target_dir(release),
        APP,
    )?)
}

#[cfg(target_os = "windows")]
fn bundle(output: &Path, release: bool) -> anyhow::Result<PathBuf> {
    Ok(cef::build_util::win::build_bundle(output, APP, release)?)
}
