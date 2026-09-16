use std::io::{Read, Write};
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

use crate::error::{anyhow, Result};

pub const CONFIG_FILE: &str = "growthlab.yaml";
const MAX_CONFIG_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    #[serde(alias = "analysis")]
    AnalyzeOnly,
    Draft,
    #[serde(rename = "implementation", alias = "implement")]
    #[value(name = "implementation", alias = "implement")]
    Implement,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Product {
    pub name: String,
    pub audience: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Goal {
    pub primary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    pub mode: PermissionMode,
    #[serde(default)]
    pub allowed_paths: Vec<String>,
    #[serde(default)]
    pub denied_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Validation {
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

impl Default for Validation {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
            timeout_seconds: default_timeout(),
        }
    }
}

fn default_timeout() -> u64 {
    120
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Metrics {
    pub primary: String,
    #[serde(default)]
    pub guardrails: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Agents {
    pub parallelism: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GrowthConfig {
    pub version: u32,
    pub product: Product,
    pub goal: Goal,
    pub permissions: Permissions,
    pub validation: Validation,
    pub metrics: Metrics,
    pub agents: Agents,
}

impl GrowthConfig {
    pub fn parse(value: &str) -> Result<Self> {
        if value.len() as u64 > MAX_CONFIG_BYTES {
            return Err(anyhow!("growthlab.yaml exceeds the 64 KiB limit"));
        }
        if super::redaction::contains_secret(value) {
            return Err(anyhow!("growthlab.yaml contains a possible credential; remove secrets, including from comments"));
        }
        let config: Self = serde_yaml_ng::from_str(value).map_err(|error| {
            // YAML errors may echo a credential or arbitrary user value. Report
            // only the location; never copy parser content into logs/archives.
            if let Some(location) = error.location() {
                anyhow!("Invalid growthlab.yaml schema at line {}, column {} (unknown fields and invalid values are rejected)", location.line(), location.column())
            } else {
                anyhow!("Invalid growthlab.yaml schema")
            }
        })?;
        config.validate()?;
        Ok(config)
    }

    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join(CONFIG_FILE);
        let metadata = std::fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(anyhow!("growthlab.yaml must be a regular file"));
        }
        let file = std::fs::File::open(path)?;
        let mut value = String::new();
        file.take(MAX_CONFIG_BYTES + 1).read_to_string(&mut value)?;
        Self::parse(&value)
    }

    pub fn write_new(&self, root: &Path) -> Result<()> {
        self.validate()?;
        let value = serde_yaml_ng::to_string(self)?;
        if value.len() as u64 > MAX_CONFIG_BYTES {
            return Err(anyhow!(
                "Serialized growthlab.yaml exceeds the 64 KiB limit"
            ));
        }
        let temporary = root.join(format!(".growthlab-config-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| -> Result<()> {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temporary)?;
            file.write_all(value.as_bytes())?;
            file.sync_all()?;
            // Same-directory hard link atomically installs a complete file and
            // refuses an existing target, including an existing symlink.
            std::fs::hard_link(&temporary, root.join(CONFIG_FILE))
                .map_err(|error| anyhow!("Cannot create growthlab.yaml: {error}. Existing configuration is never overwritten."))?;
            Ok(())
        })();
        let cleanup = std::fs::remove_file(&temporary);
        match result {
            Err(error) => Err(error),
            Ok(()) => {
                cleanup?;
                Ok(())
            }
        }
    }

    pub fn validate(&self) -> Result<()> {
        let serialized = serde_json::to_string(self)?;
        if serialized.len() as u64 > MAX_CONFIG_BYTES {
            return Err(anyhow!("Product configuration exceeds the 64 KiB limit"));
        }
        if super::redaction::contains_secret(&serialized) {
            return Err(anyhow!(
                "Configuration contains a possible credential; remove secrets from growthlab.yaml"
            ));
        }
        if self.version != 1 {
            return Err(anyhow!("Unsupported growthlab.yaml version; expected 1"));
        }
        for (label, value) in [
            ("product.name", &self.product.name),
            ("product.audience", &self.product.audience),
            ("goal.primary", &self.goal.primary),
            ("metrics.primary", &self.metrics.primary),
        ] {
            if value.trim().is_empty() || value.len() > 4096 {
                return Err(anyhow!("{label} must contain 1–4096 bytes"));
            }
        }
        if !(1..=8).contains(&self.agents.parallelism) {
            return Err(anyhow!("agents.parallelism must be between 1 and 8"));
        }
        if !(1..=3600).contains(&self.validation.timeout_seconds) {
            return Err(anyhow!(
                "validation.timeout_seconds must be between 1 and 3600"
            ));
        }
        if self.validation.commands.len() > 16
            || self
                .validation
                .commands
                .iter()
                .any(|command| command.trim().is_empty() || command.len() > 4096)
        {
            return Err(anyhow!(
                "validation.commands accepts up to 16 nonempty commands of at most 4096 bytes"
            ));
        }
        if self.permissions.mode == PermissionMode::Implement
            && (self.permissions.allowed_paths.is_empty() || self.validation.commands.is_empty())
        {
            return Err(anyhow!(
                "implementation mode requires allowed_paths and validation.commands"
            ));
        }
        for path in self
            .permissions
            .allowed_paths
            .iter()
            .chain(&self.permissions.denied_paths)
        {
            validate_relative_path(path.trim_end_matches('/'))?;
        }
        for path in &self.permissions.allowed_paths {
            if protected_path(path) {
                return Err(anyhow!(
                    "allowed_paths includes a protected credential or internal path"
                ));
            }
        }
        Ok(())
    }

    /// Permission check for implementation in an isolated worktree. Draft
    /// artifacts are written to the lab's artifact store, never the product.
    pub fn check_permission(&self, relative: &str) -> Result<()> {
        validate_relative_path(relative)?;
        if self.permissions.mode != PermissionMode::Implement {
            return Err(anyhow!("Product file changes require implementation mode"));
        }
        if protected_path(relative)
            || self
                .permissions
                .denied_paths
                .iter()
                .any(|prefix| under(&relative.to_lowercase(), &prefix.to_lowercase()))
            || !self
                .permissions
                .allowed_paths
                .iter()
                .any(|prefix| under(relative, prefix))
        {
            return Err(anyhow!("File change is outside the allowed product paths"));
        }
        Ok(())
    }

    pub fn check_write(&self, root: &Path, relative: &str) -> Result<()> {
        self.check_permission(relative)?;
        let root = crate::paths::canonicalize(root)?;
        let mut current = root.clone();
        for component in Path::new(relative).components() {
            current.push(component);
            match std::fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(anyhow!("File changes through symlinks are prohibited"));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
}

fn under(path: &str, prefix: &str) -> bool {
    let prefix = prefix.trim_end_matches('/');
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

pub(crate) fn validate_relative_path(path: &str) -> Result<()> {
    if path.is_empty()
        || path.len() > 512
        || path.contains([
            '\\', ':', '*', '?', '<', '>', '|', '"', '\0', '\n', '\r', '\t',
        ])
        || !Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
        || path.split('/').any(|part| {
            part == "." || part == ".." || part.is_empty() || part.ends_with(['.', ' '])
        })
    {
        return Err(anyhow!(
            "Paths must be portable relative file/directory prefixes, without globs or traversal"
        ));
    }
    Ok(())
}

pub(crate) fn protected_path(path: &str) -> bool {
    path.split('/').any(|part| {
        let part = part.to_ascii_lowercase();
        matches!(
            part.as_str(),
            ".git" | ".growthlab" | ".ssh" | ".aws" | "credentials" | "secrets" | "growthlab.yaml"
        ) || part == ".env"
            || part.starts_with(".env.")
            || part.ends_with(".pem")
            || part.ends_with(".key")
            || part.starts_with("credentials.")
            || part.starts_with("secrets.")
    })
}

#[cfg(test)]
pub(crate) fn fixture() -> GrowthConfig {
    GrowthConfig {
        version: 1,
        product: Product {
            name: "Fixture".into(),
            audience: "Developers".into(),
            description: "Synthetic public test product".into(),
        },
        goal: Goal {
            primary: "Improve qualified activation".into(),
        },
        permissions: Permissions {
            mode: PermissionMode::Implement,
            allowed_paths: vec!["website".into()],
            denied_paths: vec!["website/private".into()],
        },
        validation: Validation {
            commands: vec!["node website/check.mjs".into()],
            timeout_seconds: 120,
        },
        metrics: Metrics {
            primary: "qualified_signup".into(),
            guardrails: vec!["page_load_time".into()],
        },
        agents: Agents { parallelism: 3 },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_credentials_and_unsupported_versions_without_echoing_values() {
        let mut config = fixture();
        config.product.description = "API_KEY=fixture-private-value".into();
        let error = config.validate().unwrap_err().to_string();
        assert!(!error.contains("fixture-private-value"));
        config.product.description.clear();
        let yaml = serde_yaml_ng::to_string(&config).unwrap();
        assert!(GrowthConfig::parse(&(yaml + "\n# API_KEY=fixture-private-value\n")).is_err());
        config.version = 2;
        assert!(config.validate().is_err());
        config.version = 1;
        config.permissions.allowed_paths = vec!["website/".into()];
        assert!(
            config.validate().is_ok(),
            "documented directory prefixes accept a trailing slash"
        );
        config.product.description = "x".repeat(65537);
        assert!(config.validate().is_err());
    }

    #[test]
    fn schema_roundtrip_and_missing_provenance_sensitive_fields() {
        let config = fixture();
        let yaml = serde_yaml_ng::to_string(&config).unwrap();
        assert_eq!(GrowthConfig::parse(&yaml).unwrap(), config);
        for suffix in ["\napi_key: secret-value", "\nversion: 2", "\nunknown: true"] {
            let error = GrowthConfig::parse(&(yaml.clone() + suffix))
                .unwrap_err()
                .to_string();
            assert!(!error.contains("secret-value"));
        }
        assert!(GrowthConfig::parse(&"x".repeat(65537)).is_err());
    }

    #[test]
    fn rejects_bad_contracts_and_portable_traversal() {
        for path in [
            "",
            ".",
            "../website",
            "website/../private",
            "/tmp",
            "website//a",
            "C:/a",
            "website/private./a",
            "website/private /a",
            "website/<a>",
            "website\\a",
            "website/*",
            "website/./a",
        ] {
            assert!(validate_relative_path(path).is_err(), "{path}");
        }
        let mut config = fixture();
        config.agents.parallelism = 0;
        assert!(config.validate().is_err());
        config.agents.parallelism = 3;
        config.permissions.allowed_paths.clear();
        assert!(config.validate().is_err());
        config.permissions.mode = PermissionMode::AnalyzeOnly;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn writes_new_config_once_and_enforces_denials_and_modes() {
        let dir = std::env::temp_dir().join(format!("growthlab-config-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut config = fixture();
        config.write_new(&dir).unwrap();
        assert!(config.write_new(&dir).is_err());
        assert_eq!(GrowthConfig::load(&dir).unwrap(), config);
        assert!(config.check_write(&dir, "website/index.html").is_ok());
        for path in [
            "website-other/a",
            "website/private/a",
            "website/PRIVATE/a",
            "website/.env",
            "website/.env.local",
            "website/key.pem",
            "website/credentials.json",
            ".git/config",
            "growthlab.yaml",
        ] {
            assert!(config.check_write(&dir, path).is_err(), "{path}");
        }
        for mode in [PermissionMode::Draft, PermissionMode::AnalyzeOnly] {
            config.permissions.mode = mode;
            assert!(config.check_write(&dir, "website/index.html").is_err());
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_files_and_parent_directories() {
        let dir = std::env::temp_dir().join(format!("growthlab-symlink-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("website")).unwrap();
        std::os::unix::fs::symlink(std::env::temp_dir(), dir.join("website/link")).unwrap();
        assert!(fixture()
            .check_write(&dir, "website/link/new.html")
            .is_err());
        std::os::unix::fs::symlink("/etc/passwd", dir.join("website/index.html")).unwrap();
        assert!(fixture().check_write(&dir, "website/index.html").is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
