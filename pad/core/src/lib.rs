//! Pure logic for the reMarkable Profiles PIN pad.
//!
//! This is convenience and basic privacy only, NOT a security boundary. Anyone
//! with USB or SSH access can read every profile's data and this file.
//!
//! The hashing scheme here MUST stay byte-for-byte identical to the shell
//! engine's hash helper (`bin/rm-profile`), because the pad and the engine both
//! read the same `pins.conf`. The shared canonical test vector guards against
//! drift between the two implementations.

use sha2::{Digest, Sha256};

/// Lowercase hex SHA-256 of `salt` concatenated with `pin`.
///
/// Canonical: `hash_pin("cafebabe", "1234")` ==
/// `"e35ced642ebca92d6bef21d6581b8d6f9a2a60037cc5689e7127006a8104a977"`.
pub fn hash_pin(salt: &str, pin: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(pin.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// One record from `pins.conf`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinEntry {
    pub name: String,
    pub salt: String,
    pub hash: String,
}

/// Parse a `pins.conf`: one `name:salt:hash` record per line. Blank lines and
/// lines beginning with `#` are ignored. Malformed lines (not exactly three
/// non-empty colon-separated fields) are dropped.
pub fn parse_pins(contents: &str) -> Vec<PinEntry> {
    contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let mut parts = line.splitn(3, ':');
            let name = parts.next()?;
            let salt = parts.next()?;
            let hash = parts.next()?;
            if name.is_empty() || salt.is_empty() || hash.is_empty() {
                return None;
            }
            Some(PinEntry {
                name: name.to_string(),
                salt: salt.to_string(),
                hash: hash.to_string(),
            })
        })
        .collect()
}

/// Return the name of the first profile whose stored hash matches `pin`.
pub fn match_pin<'a>(entries: &'a [PinEntry], pin: &str) -> Option<&'a str> {
    entries
        .iter()
        .find(|entry| hash_pin(&entry.salt, pin) == entry.hash)
        .map(|entry| entry.name.as_str())
}

/// What the pad should do after a PIN is entered. Keeping this decision in the
/// host-testable core (rather than in the framebuffer binary) means the pad's
/// behavior is covered by ordinary `cargo test`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// PIN matched no profile — clear the entry and let the user retry.
    Reject,
    /// PIN matched the already-active profile — just start xochitl, no switch.
    StartCurrent,
    /// PIN matched a different profile — switch to it, then start xochitl.
    SwitchTo(String),
}

/// Decide what to do given the known profiles, the currently active profile,
/// and the entered PIN.
pub fn decide(entries: &[PinEntry], active: &str, pin: &str) -> Action {
    match match_pin(entries, pin) {
        None => Action::Reject,
        Some(name) if name == active => Action::StartCurrent,
        Some(name) => Action::SwitchTo(name.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shared with `tests/hash-consistency.bats`. If this constant changes, the
    /// shell engine and the pad have diverged.
    const CANONICAL: &str =
        "e35ced642ebca92d6bef21d6581b8d6f9a2a60037cc5689e7127006a8104a977";

    #[test]
    fn hash_matches_canonical_vector() {
        assert_eq!(hash_pin("cafebabe", "1234"), CANONICAL);
    }

    #[test]
    fn hash_is_deterministic() {
        assert_eq!(hash_pin("cafebabe", "1234"), hash_pin("cafebabe", "1234"));
    }

    #[test]
    fn hash_changes_with_pin() {
        assert_ne!(hash_pin("cafebabe", "1234"), hash_pin("cafebabe", "0000"));
    }

    #[test]
    fn hash_changes_with_salt() {
        assert_ne!(hash_pin("aaaa", "1234"), hash_pin("bbbb", "1234"));
    }

    #[test]
    fn parse_skips_comments_and_blanks() {
        let contents = "# comment\n\nduncan:aaaa:hhhh\nkid:bbbb:gggg\n";
        let entries = parse_pins(contents);
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0],
            PinEntry {
                name: "duncan".into(),
                salt: "aaaa".into(),
                hash: "hhhh".into()
            }
        );
    }

    #[test]
    fn parse_rejects_malformed() {
        assert!(parse_pins("only:two\n").is_empty());
        assert!(parse_pins("::\n").is_empty());
        assert!(parse_pins("name::hash\n").is_empty());
    }

    #[test]
    fn match_returns_name_for_correct_pin() {
        let salt = "cafebabe";
        let entries = vec![PinEntry {
            name: "kid".into(),
            salt: salt.into(),
            hash: hash_pin(salt, "4321"),
        }];
        assert_eq!(match_pin(&entries, "4321"), Some("kid"));
    }

    #[test]
    fn match_returns_none_for_wrong_pin() {
        let salt = "cafebabe";
        let entries = vec![PinEntry {
            name: "kid".into(),
            salt: salt.into(),
            hash: hash_pin(salt, "4321"),
        }];
        assert_eq!(match_pin(&entries, "0000"), None);
    }

    #[test]
    fn match_picks_correct_profile_among_many() {
        let entries = vec![
            PinEntry { name: "duncan".into(), salt: "s1".into(), hash: hash_pin("s1", "1111") },
            PinEntry { name: "kid".into(), salt: "s2".into(), hash: hash_pin("s2", "2222") },
        ];
        assert_eq!(match_pin(&entries, "2222"), Some("kid"));
        assert_eq!(match_pin(&entries, "1111"), Some("duncan"));
    }

    fn two_profiles() -> Vec<PinEntry> {
        vec![
            PinEntry { name: "duncan".into(), salt: "s1".into(), hash: hash_pin("s1", "1111") },
            PinEntry { name: "kid".into(), salt: "s2".into(), hash: hash_pin("s2", "2222") },
        ]
    }

    #[test]
    fn decide_rejects_unknown_pin() {
        assert_eq!(decide(&two_profiles(), "duncan", "9999"), Action::Reject);
    }

    #[test]
    fn decide_starts_current_when_active_pin_entered() {
        assert_eq!(decide(&two_profiles(), "duncan", "1111"), Action::StartCurrent);
    }

    #[test]
    fn decide_switches_when_other_pin_entered() {
        assert_eq!(
            decide(&two_profiles(), "duncan", "2222"),
            Action::SwitchTo("kid".into())
        );
    }
}
