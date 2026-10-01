// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

/// Encodes the complete version so a new patch cannot be captured by an older tray process.
pub fn semver_instance_key(version: &str) -> String {
    let mut key = String::with_capacity(version.len() * 2);
    for byte in version.bytes() {
        use std::fmt::Write as _;
        write!(&mut key, "{byte:02x}").expect("writing to String is infallible");
    }
    key
}

#[cfg(test)]
mod tests {
    use super::semver_instance_key;
    #[test]
    fn singleton_identity_changes_for_every_release_version() {
        let current = "0.6.2";

        for version in ["0.6.0", "0.6.1", "0.6.3", "0.6.4"] {
            assert_ne!(semver_instance_key(current), semver_instance_key(version));
        }
    }
}
