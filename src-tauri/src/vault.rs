//! Holds the 32-byte data key that encrypts config.json (TorBox key + addon URLs) in the OS keychain
//! (macOS Keychain, Windows Credential Manager, iOS Keychain). Elsewhere there is no vault: the file
//! stays owner-only (0600) but unencrypted.
use companion_core::config::DataKey;

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "ios"))]
pub fn data_key(create: bool) -> Option<DataKey> {
    let e = keyring::Entry::new("app.companion.streamplaylist", "config-data-key").ok()?;
    match e.get_password() {
        Ok(h) => {
            let b: Vec<u8> = (0..h.len()).step_by(2).filter_map(|i| u8::from_str_radix(h.get(i..i + 2)?, 16).ok()).collect();
            b.try_into().ok()
        }
        Err(keyring::Error::NoEntry) if create => {
            let k = companion_core::config::new_data_key();
            e.set_password(&k.iter().map(|x| format!("{x:02x}")).collect::<String>()).ok()?;
            Some(k)
        }
        Err(_) => None,
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "ios")))]
pub fn data_key(_create: bool) -> Option<DataKey> {
    None
}
