//! Atomic, content-addressed run seals. The stored digest anchors the manifest;
//! readers verify every byte instead of trusting mutable live logs/worktrees.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{anyhow, Result};

use super::config::validate_relative_path;

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn private_directory(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink()=>return Err(anyhow!("GrowthLab state directory cannot be a file or symlink; existing files were preserved")),
        Err(error) if error.kind()!=std::io::ErrorKind::NotFound=>return Err(error.into()),
        _=>{},
    }
    std::fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    digest: String,
    size: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    entries: BTreeMap<String, Entry>,
}

fn archive_path(root: &Path, hash: &str) -> Result<PathBuf> {
    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(anyhow!("Invalid archive digest"));
    }
    Ok(root.join("growth-archives").join(hash))
}

pub fn seal(root: &Path, files: &BTreeMap<String, Vec<u8>>) -> Result<String> {
    let parent = root.join("growth-archives");
    private_directory(&parent)?;
    let temporary = parent.join(format!(".seal-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&temporary)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700))?;
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o700))?;
    }
    let result = (|| {
        let mut entries = BTreeMap::new();
        for (name, bytes) in files {
            validate_relative_path(name)?;
            if name == "manifest.json" || bytes.len() > 4 * 1024 * 1024 {
                return Err(anyhow!("Invalid or oversized archive entry"));
            }
            let path = temporary.join(name);
            std::fs::create_dir_all(path.parent().unwrap())?;
            write_frozen(&path, bytes)?;
            entries.insert(
                name.clone(),
                Entry {
                    digest: digest(bytes),
                    size: bytes.len() as u64,
                },
            );
        }
        let manifest = serde_json::to_vec(&Manifest {
            version: 1,
            entries,
        })?;
        let hash = digest(&manifest);
        write_frozen(&temporary.join("manifest.json"), &manifest)?;
        let target = archive_path(root, &hash)?;
        if target.exists() {
            verify(root, &hash)?;
            std::fs::remove_dir_all(&temporary)?;
        } else {
            std::fs::rename(&temporary, target)?;
        }
        Ok(hash)
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&temporary);
    }
    result
}

fn write_frozen(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    #[cfg(unix)]
    {
        let mut permissions = file.metadata()?.permissions();
        permissions.set_readonly(true);
        file.set_permissions(permissions)?;
    }
    Ok(())
}

fn read_regular(path: &Path, cap: u64) -> Result<Vec<u8>> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > cap {
        return Err(anyhow!("Archive entry is not a bounded regular file"));
    }
    Ok(std::fs::read(path)?)
}

pub fn verify(root: &Path, hash: &str) -> Result<BTreeMap<String, Vec<u8>>> {
    let directory = archive_path(root, hash)?;
    if std::fs::symlink_metadata(&directory)?
        .file_type()
        .is_symlink()
    {
        return Err(anyhow!("Archive directory cannot be a symlink"));
    }
    let bytes = read_regular(&directory.join("manifest.json"), 128 * 1024)?;
    if digest(&bytes) != hash {
        return Err(anyhow!("Archive manifest failed its digest check"));
    }
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|_| anyhow!("Invalid archive manifest"))?;
    if manifest.version != 1 || manifest.entries.len() > 256 {
        return Err(anyhow!("Unsupported or oversized archive manifest"));
    }
    let mut files = BTreeMap::new();
    for (name, entry) in manifest.entries {
        validate_relative_path(&name)?;
        // Reject symlink ancestors as well as symlink leaf entries.
        let mut path = directory.clone();
        for component in Path::new(&name).components() {
            path.push(component);
            if std::fs::symlink_metadata(&path)?.file_type().is_symlink() {
                return Err(anyhow!("Archive entries cannot traverse symlinks"));
            }
        }
        let value = read_regular(&path, 4 * 1024 * 1024)?;
        if value.len() as u64 != entry.size || digest(&value) != entry.digest {
            return Err(anyhow!("Archive entry failed its digest check"));
        }
        files.insert(name, value);
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn symlinked_archive_parent_is_rejected_without_touching_its_target() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let root = std::env::temp_dir().join(format!("growth-seal-link-{}", uuid::Uuid::new_v4()));
        let target = root.join("unrelated");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).unwrap();
        symlink(&target, root.join("growth-archives")).unwrap();
        assert!(seal(&root, &BTreeMap::from([("run.json".into(), vec![])])).is_err());
        assert_eq!(
            std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert!(std::fs::read_dir(&target).unwrap().next().is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn seal_is_idempotent_and_tampering_is_detected() {
        let root = std::env::temp_dir().join(format!("growth-seal-{}", uuid::Uuid::new_v4()));
        let files = BTreeMap::from([("run.json".into(), b"{\"status\":\"failed\"}".to_vec())]);
        let hash = seal(&root, &files).unwrap();
        assert_eq!(verify(&root, &hash).unwrap(), files);
        assert_eq!(seal(&root, &files).unwrap(), hash);
        let path = archive_path(&root, &hash).unwrap().join("run.json");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        #[cfg(windows)]
        {
            let mut permissions = std::fs::metadata(&path).unwrap().permissions();
            permissions.set_readonly(false);
            std::fs::set_permissions(&path, permissions).unwrap();
        }
        std::fs::write(path, "tampered").unwrap();
        assert!(verify(&root, &hash).is_err());
        assert!(seal(&root, &BTreeMap::from([("../escape".into(), vec![])])).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
