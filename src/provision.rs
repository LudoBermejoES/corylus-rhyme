use std::sync::{Arc, Mutex};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

use crate::{
    Inner, RhymeError, RhymeState, Result,
    state::{self, VersionFile, SCHEMA_VERSION, FST_FORMAT_VERSION},
    map::LoadedMap,
};

pub async fn run(
    inner: Arc<Mutex<Inner>>,
    on_progress: impl Fn(RhymeState) + Send + 'static,
) -> Result<()> {
    {
        let guard = inner.lock().unwrap();
        if state::is_installed_for(&guard.config) {
            info!("[rhyme] already installed for {}", guard.config.lang);
            drop(guard);
            return try_load_map(inner);
        }
        // Guard: do not start if already downloading/indexing.
        match &guard.state {
            RhymeState::Downloading { .. } | RhymeState::Indexing => {
                info!("[rhyme] provision already in flight for {}", guard.config.lang);
                return Ok(());
            }
            _ => {}
        }
        std::fs::create_dir_all(&guard.config.data_dir)?;
    }

    let (url, sha256_expected, lang) = {
        let g = inner.lock().unwrap();
        (
            g.config.source_url.clone(),
            g.config.source_sha256.clone(),
            g.config.lang.clone(),
        )
    };

    let part_path = state::part_path(&inner.lock().unwrap().config);

    // --- Download ---
    info!("[rhyme] downloading {} from {}", lang, url);
    let client = reqwest::Client::new();
    let resp = client.get(&url).send().await.map_err(RhymeError::Http)?;
    let total = resp.content_length();

    set_state(&inner, RhymeState::Downloading { downloaded: 0, total });
    on_progress(RhymeState::Downloading { downloaded: 0, total });

    let mut file = tokio::fs::File::create(&part_path).await?;
    let mut hasher = Sha256::new();
    let mut downloaded: u64 = 0;
    let mut buf: Vec<u8> = Vec::new();

    use futures_util::StreamExt;
    let mut byte_stream = resp.bytes_stream();

    while let Some(chunk) = byte_stream.next().await {
        let chunk = chunk.map_err(RhymeError::Http)?;
        hasher.update(&chunk);
        downloaded += chunk.len() as u64;
        buf.extend_from_slice(&chunk);
        file.write_all(&chunk).await?;
        let s = RhymeState::Downloading { downloaded, total };
        set_state(&inner, s.clone());
        on_progress(s);
    }
    file.flush().await?;
    drop(file);

    // --- Verify checksum ---
    let actual = format!("{:x}", hasher.finalize());
    if actual != sha256_expected {
        let _ = std::fs::remove_file(&part_path);
        warn!("[rhyme] checksum mismatch for {}: expected {} got {}", lang, sha256_expected, actual);
        let err = RhymeError::ChecksumMismatch {
            expected: sha256_expected,
            actual,
        };
        set_state(&inner, RhymeState::Error { message: err.to_string() });
        return Err(err);
    }
    info!("[rhyme] checksum ok for {}", lang);

    // --- Index: unpack tarball containing {lang}.rhyme.fst and {lang}.rhyme.json ---
    set_state(&inner, RhymeState::Indexing);
    on_progress(RhymeState::Indexing);

    let dest = inner.lock().unwrap().config.data_dir.clone();
    unpack_tar_gz(&buf, &dest).map_err(|e| RhymeError::Fst(e.to_string()))?;

    // --- Write version file ---
    let ver_path = state::version_path(&inner.lock().unwrap().config);
    let version = VersionFile {
        lang: lang.clone(),
        source_sha256: sha256_expected,
        schema_version: SCHEMA_VERSION,
        fst_format_version: FST_FORMAT_VERSION,
    };
    std::fs::write(&ver_path, serde_json::to_string_pretty(&version).unwrap())?;

    let _ = std::fs::remove_file(&part_path);

    // --- Load the map into memory ---
    try_load_map(inner.clone())?;
    on_progress(RhymeState::Ready);
    info!("[rhyme] provision complete for {}", lang);
    Ok(())
}

fn unpack_tar_gz(data: &[u8], dest_dir: &std::path::Path) -> std::io::Result<()> {
    let cursor = std::io::Cursor::new(data);
    let gz = flate2::read::GzDecoder::new(cursor);
    let mut archive = tar::Archive::new(gz);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        let filename = path.file_name().unwrap_or_default().to_os_string();
        let dest = dest_dir.join(filename);
        let mut out = std::fs::File::create(&dest)?;
        std::io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}

pub fn try_load_map(inner: Arc<Mutex<Inner>>) -> Result<()> {
    let (fst_path, tails_path, lang) = {
        let g = inner.lock().unwrap();
        (
            state::fst_path(&g.config),
            state::tails_path(&g.config),
            g.config.lang.clone(),
        )
    };

    match LoadedMap::load(&fst_path, &tails_path) {
        Ok(loaded) => {
            let mut g = inner.lock().unwrap();
            g.loaded_map = Some(Arc::new(loaded));
            g.state = RhymeState::Ready;
            info!("[rhyme] map loaded for {}", lang);
            Ok(())
        }
        Err(e) => {
            let mut g = inner.lock().unwrap();
            g.state = RhymeState::Error { message: e.to_string() };
            Err(e)
        }
    }
}

fn set_state(inner: &Arc<Mutex<Inner>>, state: RhymeState) {
    inner.lock().unwrap().state = state;
}
