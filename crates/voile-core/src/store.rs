//! Persistance JSON atomique (écriture dans un fichier temporaire puis rename).

use crate::workspace::{Workspace, FORMAT_VERSION};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("lecture/écriture de {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("workspace.json invalide: {0}")]
    Json(#[from] serde_json::Error),
    #[error("version de format {0} plus récente que celle supportée ({FORMAT_VERSION})")]
    TooNew(u32),
}

pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn new(dir: impl AsRef<Path>) -> Self {
        Self {
            path: dir.as_ref().join("workspace.json"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Charge le workspace, ou en crée un neuf s'il n'existe pas encore.
    /// Un fichier corrompu est mis de côté (`workspace.json.broken`) plutôt
    /// que de faire planter le navigateur.
    pub fn load(&self) -> Result<Workspace, StoreError> {
        let raw = match fs::read(&self.path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Workspace::new()),
            Err(source) => {
                return Err(StoreError::Io {
                    path: self.path.clone(),
                    source,
                })
            }
        };
        match serde_json::from_slice::<Workspace>(&raw) {
            Ok(ws) if ws.version > FORMAT_VERSION => Err(StoreError::TooNew(ws.version)),
            Ok(mut ws) => {
                ws.after_load();
                Ok(ws)
            }
            Err(e) => {
                tracing::error!("workspace.json illisible ({e}), on repart de zéro");
                let _ = fs::rename(&self.path, self.path.with_extension("json.broken"));
                Ok(Workspace::new())
            }
        }
    }

    pub fn save(&self, ws: &Workspace) -> Result<(), StoreError> {
        let io = |source| StoreError::Io {
            path: self.path.clone(),
            source,
        };
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(io)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let mut file = fs::File::create(&tmp).map_err(io)?;
        file.write_all(&serde_json::to_vec_pretty(ws)?)
            .map_err(io)?;
        file.sync_all().map_err(io)?;
        fs::rename(&tmp, &self.path).map_err(io)?;
        Ok(())
    }
}
