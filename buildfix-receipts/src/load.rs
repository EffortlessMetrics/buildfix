use anyhow::Context;
use buildfix_types::receipt::ReceiptEnvelope;
use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use glob::glob;
use thiserror::Error;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct LoadedReceipt {
    pub path: Utf8PathBuf,
    /// Directory name under artifacts/... (best effort).
    pub sensor_id: String,
    pub receipt: Result<ReceiptEnvelope, ReceiptLoadError>,
}

#[derive(Debug, Error, Clone)]
pub enum ReceiptLoadError {
    #[error("io error: {message}")]
    Io { message: String },

    #[error("json parse error: {message}")]
    Json { message: String },
}

pub fn load_receipts(artifacts_dir: &Utf8Path) -> anyhow::Result<Vec<LoadedReceipt>> {
    let pattern = artifacts_dir.join("*/report.json");
    let pattern_str = pattern.as_str();

    debug!(pattern = %pattern_str, "scanning artifacts for receipts");

    let mut out = Vec::new();
    for entry in glob(pattern_str).context("glob artifacts/*/report.json")? {
        let path = entry
            .map_err(|e| anyhow::anyhow!("glob error: {e}"))?
            .to_string_lossy()
            .to_string();

        let utf8_path = Utf8PathBuf::from(path);
        let sensor_id = utf8_path
            .parent()
            .and_then(|p| p.file_name())
            .unwrap_or("unknown")
            .to_string();

        // Skip buildfix's own output directory - it's not a sensor receipt.
        if sensor_id == "buildfix" {
            debug!(path = %utf8_path, "skipping buildfix's own report");
            continue;
        }

        let receipt = match fs::read_to_string(&utf8_path) {
            Ok(s) => {
                serde_json::from_str::<ReceiptEnvelope>(&s).map_err(|e| ReceiptLoadError::Json {
                    message: e.to_string(),
                })
            }
            Err(e) => Err(ReceiptLoadError::Io {
                message: e.to_string(),
            }),
        };

        out.push(LoadedReceipt {
            path: utf8_path,
            sensor_id,
            receipt,
        });
    }

    // Deterministic order matters.
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}
