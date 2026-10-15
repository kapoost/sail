//! The three things about sailing together that a person should not have to type twice.
//!
//! Your name, the address of the game you sail on, and its word. None of them is a
//! setting in the sense the other settings are — they are not preferences about how the
//! boat behaves, they are *where your friends are*, and retyping an address and a word
//! every time the game starts is the kind of friction that quietly ends a thing like
//! this. It was the last piece missing from joining a server rather than a laptop on the
//! wifi.
//!
//! One small file in the place the operating system keeps settings, written when the
//! panel is closed and read once at startup. No format, no versioning, no library: three
//! lines of `key=value`, because this is three strings and a parser for three strings
//! should be shorter than the data.
//!
//! **The word is in it**, which is a decision rather than an oversight. It is a doorbell
//! and not a credential — it crosses the network in the clear and the server treats it
//! as the price of entry rather than as proof of anything — so it belongs with the
//! address it goes with, the way a door code lives beside the address in a notebook. The
//! file is written 0600 on anything that has permissions. What it must never become is a
//! thing that travels: it is in the user's own config directory, not in the repository,
//! not in a save file, and not on a screenshot.

use bevy::prelude::*;

/// Where the file goes: the usual place for the platform, and nowhere clever.
fn path() -> Option<std::path::PathBuf> {
    let dir = if cfg!(target_os = "windows") {
        std::path::PathBuf::from(std::env::var("APPDATA").ok()?).join("sail")
    } else {
        std::path::PathBuf::from(std::env::var("HOME").ok()?)
            .join(".config")
            .join("sail")
    };
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("together"))
}

/// Read at startup. A missing file is the ordinary case and not a fault.
pub fn read() -> (String, String, String) {
    let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) else {
        return (String::new(), String::new(), String::new());
    };
    let mut name = String::new();
    let mut address = String::new();
    let mut word = String::new();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().to_string();
        match key.trim() {
            "name" => name = value,
            "address" => address = value,
            "word" => word = value,
            _ => {}
        }
    }
    (name, address, word)
}

/// Written when the panel closes, which is the moment the player has finished changing
/// their mind about it.
pub fn write(name: &str, address: &str, word: &str) {
    let Some(path) = path() else {
        return;
    };
    let text = format!(
        "# Where your friends are. Written by the game; safe to delete.\n\
         name={}\naddress={}\nword={}\n",
        name.trim(),
        address.trim(),
        word.trim()
    );
    if std::fs::write(&path, text).is_err() {
        warn!("could not remember the game at {}", path.display());
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
}
