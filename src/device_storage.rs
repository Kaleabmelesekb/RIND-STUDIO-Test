//! The phone's internal storage, which the Browser lists like a music player: its root, whether the app may read it,
//! the request for that, and the folders a listener keeps music in. On the desktop (and in the tests) there is none:
//! the Browser lists the app's own library only.

use std::path::{Path, PathBuf};

use crate::project_files::is_audio;

/// The folders of the root offered as places when they exist, in this order.
const PLACES: [&str; 5] = ["Music", "Download", "Documents", "Recordings", "Samples"];

/// The internal storage's root (`/storage/emulated/0`), where there is one.
pub fn root() -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    {
        ["/storage/emulated/0", "/sdcard"]
            .into_iter()
            .map(PathBuf::from)
            .find(|p| p.is_dir())
    }
    #[cfg(not(target_os = "android"))]
    {
        None
    }
}

/// Whether the app may read the storage now. A JNI call: ask it about once a second, not every frame.
pub fn granted() -> bool {
    #[cfg(target_os = "android")]
    {
        let mut yes = false;
        let _ = crate::android::with_activity(|env, activity| {
            yes = env.call_method(activity, "hasStorage", "()Z", &[])?.z()?;
            Ok(())
        });
        yes
    }
    #[cfg(not(target_os = "android"))]
    {
        false
    }
}

/// Opens the system's page (or question) that gives the app the storage; [`granted`] turns true once it does.
pub fn ask() {
    #[cfg(target_os = "android")]
    {
        let _ = crate::android::with_activity(|env, activity| {
            env.call_method(activity, "askStorage", "()V", &[])?;
            Ok(())
        });
    }
}

/// `path` lies on the internal storage.
pub fn on_storage(path: &Path) -> bool {
    root().is_some_and(|root| path.starts_with(root))
}

/// The root's own folders worth a shortcut, as `(name, path)`.
pub fn places() -> Vec<(String, PathBuf)> {
    let Some(root) = root() else {
        return Vec::new();
    };
    (PLACES.into_iter())
        .map(|name| (name.to_owned(), root.join(name)))
        .filter(|(_, path)| path.is_dir())
        .collect()
}

/// One folder's content: `(name, path, is_folder)`, folders first, each group by name. Dot files and files that are not
/// audio are left out.
pub fn list(dir: &Path) -> Vec<(String, PathBuf, bool)> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<(String, PathBuf, bool)> = read
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            let folder = path.is_dir();
            (!name.starts_with('.') && (folder || is_audio(&path))).then_some((name, path, folder))
        })
        .collect();
    out.sort_by(|a, b| (b.2.cmp(&a.2)).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase())));
    out
}

/// Keeps a copy of the audio file `from` in the app's `My Samples/Device` folder (the project reads files from the
/// user folder only) and returns it; a copy of the same size already there stays.
pub fn bring_in(user: &Path, from: &Path) -> Option<PathBuf> {
    let dir = user
        .join(crate::project_files::SAMPLES_FOLDER)
        .join("Device");
    std::fs::create_dir_all(&dir).ok()?;
    let to = dir.join(from.file_name()?);
    let len = |p: &Path| std::fs::metadata(p).ok().map(|m| m.len());
    if len(&to).is_none() || len(&to) != len(from) {
        std::fs::copy(from, &to).ok()?;
    }
    Some(to)
}

/// The folders pinned as places, one per line of the user folder's `browser-places.txt`.
pub fn pins(user: &Path) -> Vec<PathBuf> {
    let text = std::fs::read_to_string(user.join("browser-places.txt")).unwrap_or_default();
    (text.lines())
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// Writes the pinned places.
pub fn save_pins(user: &Path, pins: &[PathBuf]) {
    let text: Vec<_> = pins.iter().map(|p| p.to_string_lossy()).collect();
    let _ = std::fs::write(user.join("browser-places.txt"), text.join("\n"));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rind-storage-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_listing_has_folders_first_then_audio_by_name_and_no_dot_files() {
        let dir = temp("list");
        std::fs::create_dir(dir.join("Zebra")).unwrap();
        std::fs::create_dir(dir.join("apple")).unwrap();
        std::fs::create_dir(dir.join(".hidden")).unwrap();
        std::fs::write(dir.join("b.wav"), b"x").unwrap();
        std::fs::write(dir.join("A.wav"), b"x").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        let names: Vec<_> = list(&dir).into_iter().map(|(n, _, f)| (n, f)).collect();
        assert_eq!(
            names,
            [
                ("apple".to_owned(), true),
                ("Zebra".to_owned(), true),
                ("A.wav".to_owned(), false),
                ("b.wav".to_owned(), false),
            ]
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_brought_in_file_lands_in_my_samples_device_and_keeps_its_copy() {
        let user = temp("user");
        let from = temp("from").join("kick.wav");
        std::fs::write(&from, b"abc").unwrap();
        let to = bring_in(&user, &from).unwrap();
        assert!(to.starts_with(user.join(crate::project_files::SAMPLES_FOLDER)));
        assert_eq!(std::fs::read(&to).unwrap(), b"abc");
        assert_eq!(bring_in(&user, &from), Some(to));
        let _ = std::fs::remove_dir_all(user);
    }

    #[test]
    fn pins_round_trip_through_the_user_folder() {
        let user = temp("pins");
        assert!(pins(&user).is_empty());
        let list = [PathBuf::from("/a/b"), PathBuf::from("/c")];
        save_pins(&user, &list);
        assert_eq!(pins(&user), list);
        let _ = std::fs::remove_dir_all(user);
    }

    #[test]
    fn there_is_no_internal_storage_off_android() {
        assert_eq!(root(), None);
        assert!(!granted());
        assert!(!on_storage(Path::new("/storage/emulated/0/Music")));
        assert!(places().is_empty());
    }
}
