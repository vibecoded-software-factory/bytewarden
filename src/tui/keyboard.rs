use std::borrow::Cow;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyboard {
    Mac,

    Pc,
}

pub fn host() -> Keyboard {
    static HOST: OnceLock<Keyboard> = OnceLock::new();
    *HOST.get_or_init(|| classify(std::env::var("BYTEWARDEN_KEYS").ok().as_deref()))
}

fn classify(override_var: Option<&str>) -> Keyboard {
    match override_var.map(str::trim) {
        Some("mac") | Some("macos") | Some("apple") => Keyboard::Mac,
        Some("pc") | Some("linux") | Some("windows") => Keyboard::Pc,
        _ => {
            if cfg!(target_os = "macos") {
                Keyboard::Mac
            } else {
                Keyboard::Pc
            }
        }
    }
}

const MODIFIERS: [(&str, &str); 4] = [
    ("Ctrl+", "⌃"),
    ("Alt+", "⌥"),
    ("Shift+", "⇧"),
    ("Super+", "⌘"),
];

pub fn label(key: &str) -> Cow<'_, str> {
    label_on(host(), key)
}

pub fn label_on(kb: Keyboard, key: &str) -> Cow<'_, str> {
    if kb == Keyboard::Pc || !key.contains('+') {
        return Cow::Borrowed(key);
    }
    let mut out = key.to_string();
    for (spelled, glyph) in MODIFIERS {
        if out.contains(spelled) {
            out = out.replace(spelled, glyph);
        }
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_wins_over_the_compile_target() {
        assert_eq!(classify(Some("mac")), Keyboard::Mac);
        assert_eq!(classify(Some(" macos ")), Keyboard::Mac);
        assert_eq!(classify(Some("pc")), Keyboard::Pc);
        assert_eq!(classify(Some("linux")), Keyboard::Pc);
    }

    #[test]
    fn unknown_override_falls_through_to_the_target() {
        let native = classify(None);
        assert_eq!(classify(Some("garbage")), native);
        if cfg!(target_os = "macos") {
            assert_eq!(native, Keyboard::Mac);
        } else {
            assert_eq!(native, Keyboard::Pc);
        }
    }

    #[test]
    fn pc_labels_are_left_alone() {
        for key in ["Alt+S", "Ctrl+P", "Enter", "j/k", "Alt+Del"] {
            assert_eq!(label_on(Keyboard::Pc, key), key);
        }
    }

    #[test]
    fn mac_labels_use_the_apple_glyphs_without_a_separator() {
        assert_eq!(label_on(Keyboard::Mac, "Alt+S"), "⌥S");
        assert_eq!(label_on(Keyboard::Mac, "Ctrl+P"), "⌃P");
        assert_eq!(label_on(Keyboard::Mac, "Alt+Del"), "⌥Del");
        assert_eq!(label_on(Keyboard::Mac, "Shift+Tab"), "⇧Tab");
    }

    #[test]
    fn mac_leaves_unmodified_keys_untouched_and_unallocated() {
        for key in ["Enter", "Esc", "j/k", "↑↓", "F1", "Space"] {
            let out = label_on(Keyboard::Mac, key);
            assert_eq!(out, key);
            assert!(matches!(out, Cow::Borrowed(_)), "{key} should not allocate");
        }
    }

    #[test]
    fn mac_handles_a_multi_modifier_label() {
        assert_eq!(label_on(Keyboard::Mac, "Ctrl+Shift+P"), "⌃⇧P");
    }
}
