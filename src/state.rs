use std::path::PathBuf;
use crate::EngineConfig;

pub fn fst_path(config: &EngineConfig) -> PathBuf {
    config.data_dir.join(format!("{}.rhyme.fst", config.lang))
}

pub fn tails_path(config: &EngineConfig) -> PathBuf {
    config.data_dir.join(format!("{}.rhyme.json", config.lang))
}

pub fn version_path(config: &EngineConfig) -> PathBuf {
    config.data_dir.join(format!("{}.rhyme.version.json", config.lang))
}

pub fn part_path(config: &EngineConfig) -> PathBuf {
    config.data_dir.join(format!("{}.rhyme.tar.gz.part", config.lang))
}

#[allow(dead_code)]
pub fn lock_path(config: &EngineConfig) -> PathBuf {
    config.data_dir.join(format!("{}.rhyme.lock", config.lang))
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct VersionFile {
    pub lang: String,
    pub source_sha256: String,
    pub schema_version: u32,
    pub fst_format_version: u32,
}

pub const SCHEMA_VERSION: u32 = 1;
pub const FST_FORMAT_VERSION: u32 = 1;

pub fn is_installed_for(config: &EngineConfig) -> bool {
    let fst = fst_path(config);
    let tails = tails_path(config);
    let ver = version_path(config);
    if !fst.exists() || !tails.exists() || !ver.exists() {
        return false;
    }
    let Ok(data) = std::fs::read_to_string(&ver) else { return false; };
    let Ok(v) = serde_json::from_str::<VersionFile>(&data) else { return false; };
    v.lang == config.lang
        && v.source_sha256 == config.source_sha256
        && v.schema_version == SCHEMA_VERSION
        && v.fst_format_version == FST_FORMAT_VERSION
}
