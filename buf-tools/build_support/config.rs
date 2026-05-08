//! Resolve buf-tools config from env vars and Cargo.toml metadata.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigSource {
    Env,
    PackageMetadata,
    WorkspaceMetadata,
    Default,
}

#[derive(Clone, Debug)]
pub struct ResolvedConfig {
    pub layout_mode: Option<String>,
    pub layout_mode_source: ConfigSource,
    pub build_log: Option<String>,
    pub build_log_source: ConfigSource,
    pub cache_dir: Option<String>,
    pub cache_dir_source: ConfigSource,
    pub release_base_url: Option<String>,
    pub release_base_url_source: ConfigSource,
    pub source_base_url: Option<String>,
    pub source_base_url_source: ConfigSource,
    pub workspace_manifest: Option<PathBuf>,
    pub package_manifest: Option<PathBuf>,
}

impl ResolvedConfig {
    fn new() -> Self {
        Self {
            layout_mode: None,
            layout_mode_source: ConfigSource::Default,
            build_log: None,
            build_log_source: ConfigSource::Default,
            cache_dir: None,
            cache_dir_source: ConfigSource::Default,
            release_base_url: None,
            release_base_url_source: ConfigSource::Default,
            source_base_url: None,
            source_base_url_source: ConfigSource::Default,
            workspace_manifest: None,
            package_manifest: None,
        }
    }
}

pub fn resolve(out_dir: &Path) -> ResolvedConfig {
    let mut cfg = ResolvedConfig::new();
    if let Some(manifest) = discover_root_manifest(out_dir) {
        cfg.workspace_manifest = Some(manifest.clone());
        cfg.package_manifest = Some(manifest.clone());
        if let Ok(text) = fs::read_to_string(&manifest)
            && let Ok(doc) = text.parse::<toml::Value>()
        {
            apply_metadata(
                &mut cfg,
                &doc,
                &["workspace", "metadata", "buf-tools", "config"],
                ConfigSource::WorkspaceMetadata,
            );
            apply_metadata(
                &mut cfg,
                &doc,
                &["package", "metadata", "buf-tools", "config"],
                ConfigSource::PackageMetadata,
            );
        }
    }
    apply_env_overrides(&mut cfg);
    cfg
}

fn discover_root_manifest(out_dir: &Path) -> Option<PathBuf> {
    let target_dir = out_dir
        .ancestors()
        .find(|p| p.file_name().is_some_and(|n| n == "target"))?;
    let root = target_dir.parent()?;
    let manifest = root.join("Cargo.toml");
    manifest.is_file().then_some(manifest)
}

fn apply_metadata(
    cfg: &mut ResolvedConfig,
    doc: &toml::Value,
    path: &[&str],
    source: ConfigSource,
) {
    let mut cur = doc;
    for key in path {
        cur = match cur.get(*key) {
            Some(v) => v,
            None => return,
        };
    }
    let Some(table) = cur.as_table() else {
        return;
    };
    if let Some(v) = parse_string_or_bool(table.get("layout_mode")) {
        cfg.layout_mode = Some(v);
        cfg.layout_mode_source = source;
    }
    if let Some(v) = parse_string_or_bool(table.get("build_log")) {
        cfg.build_log = Some(v);
        cfg.build_log_source = source;
    }
    if let Some(v) = parse_string_or_bool(table.get("cache_dir")) {
        cfg.cache_dir = Some(v);
        cfg.cache_dir_source = source;
    }
    if let Some(v) = parse_string_or_bool(table.get("release_base_url")) {
        cfg.release_base_url = Some(v);
        cfg.release_base_url_source = source;
    }
    if let Some(v) = parse_string_or_bool(table.get("source_base_url")) {
        cfg.source_base_url = Some(v);
        cfg.source_base_url_source = source;
    }
}

fn parse_string_or_bool(v: Option<&toml::Value>) -> Option<String> {
    match v {
        Some(toml::Value::String(s)) => Some(s.clone()),
        Some(toml::Value::Boolean(b)) => Some(b.to_string()),
        _ => None,
    }
}

fn apply_env_overrides(cfg: &mut ResolvedConfig) {
    apply_env(
        &mut cfg.layout_mode,
        &mut cfg.layout_mode_source,
        "BUF_RS_LAYOUT_MODE",
    );
    apply_env(
        &mut cfg.build_log,
        &mut cfg.build_log_source,
        "BUF_RS_BUILD_LOG",
    );
    apply_env(
        &mut cfg.cache_dir,
        &mut cfg.cache_dir_source,
        "BUF_RS_CACHE_DIR",
    );
    apply_env(
        &mut cfg.release_base_url,
        &mut cfg.release_base_url_source,
        "BUF_RS_RELEASE_BASE_URL",
    );
    apply_env(
        &mut cfg.source_base_url,
        &mut cfg.source_base_url_source,
        "BUF_RS_SOURCE_BASE_URL",
    );
}

fn apply_env(dest: &mut Option<String>, source: &mut ConfigSource, key: &str) {
    if let Ok(v) = std::env::var(key) {
        *dest = Some(v);
        *source = ConfigSource::Env;
    }
}
