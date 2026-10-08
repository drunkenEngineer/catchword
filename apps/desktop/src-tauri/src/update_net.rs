//! The network module (ADR-8, ADR-24, guarded area 3): the only code in
//! Catchword that uses the network. Compiled only into the GitHub build.
//!
//! It asks one address, `updates::MANIFEST`, whether a newer version
//! exists, and downloads it only from `updates::DOWNLOADS`. Every download
//! is checked against the public key in `tauri.conf.json` before it is run,
//! and must be signed for the version it claims (`requireSignedVersion`).
//! Nothing is sent but the request itself.

use anyhow::{anyhow, Result};
use tauri::AppHandle;
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::contract::UpdateOffer;
use crate::updates::{allowed_download, MANIFEST};

/// Ask whether a newer version exists. None if this one is the newest.
pub fn check(app: &AppHandle) -> Result<Option<UpdateOffer>> {
    Ok(newer(app)?.map(|update| UpdateOffer {
        version: update.version.clone(),
        notes: update.body.clone().unwrap_or_default(),
    }))
}

/// Download the newer version, check its signature, and run its installer,
/// which closes Catchword and starts it again. The user asked for this.
pub fn install(app: &AppHandle) -> Result<()> {
    let update = newer(app)?.ok_or_else(|| anyhow!("no newer version is available"))?;
    tauri::async_runtime::block_on(update.download_and_install(|_, _| {}, || {}))?;
    Ok(())
}

fn newer(app: &AppHandle) -> Result<Option<Update>> {
    let updater = app
        .updater_builder()
        .endpoints(vec![MANIFEST.parse()?])?
        .build()?;
    let Some(update) = tauri::async_runtime::block_on(updater.check())? else {
        return Ok(None);
    };
    if !allowed_download(update.download_url.as_str()) {
        return Err(anyhow!(
            "the update is not one of Catchword's releases, so it was not used"
        ));
    }
    Ok(Some(update))
}
