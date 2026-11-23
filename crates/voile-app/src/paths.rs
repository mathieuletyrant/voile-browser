use std::path::PathBuf;

/// Emplacements des données de Voile.
///
/// * macOS : `~/Library/Application Support/cc.cheztomy.Voile/`
/// * Linux : `~/.local/share/voile/`
/// * Windows : `%APPDATA%\cheztomy\Voile\data\`
///
/// `VOILE_DATA_DIR` remplace le tout (profil de test jetable, par ex.).
#[derive(Debug, Clone)]
pub struct Dirs {
    /// `workspace.json`, `window.json`.
    pub data: PathBuf,
    /// Racine du cache Chromium (doit contenir `profile`).
    pub chromium: PathBuf,
    /// Profil Chromium : cookies, extensions, historique, mots de passe.
    pub profile: PathBuf,
}

impl Dirs {
    pub fn resolve() -> Self {
        let data = std::env::var_os("VOILE_DATA_DIR")
            .map(PathBuf::from)
            .or_else(|| {
                directories::ProjectDirs::from("cc", "cheztomy", "Voile")
                    .map(|p| p.data_dir().to_path_buf())
            })
            .unwrap_or_else(|| PathBuf::from(".voile"));
        let chromium = data.join("chromium");
        let profile = chromium.join("profile");
        Self {
            data,
            chromium,
            profile,
        }
    }
}
