//! Checks of the catalogs. See "Tests" in `docs/i18n.md`.

use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

fn catalog(locale: &str) -> BTreeMap<String, String> {
    sayso_i18n::__messages(locale).unwrap_or_else(|| panic!("no catalog for {locale}")).into_iter().collect()
}

fn placeholders(text: &str) -> BTreeSet<String> {
    let re = Regex::new(r"%\{(\w+)\}").unwrap();
    re.captures_iter(text).map(|c| c[1].to_string()).collect()
}

/// The locale folders under `locales/`, without English.
fn translated_locales() -> Vec<String> {
    let locales = Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
    let mut out: Vec<String> = std::fs::read_dir(&locales)
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|l| l != "en")
        .collect();
    out.sort();
    out
}

/// Each locale in `LOCALES` has a catalog folder, and each folder is a locale
/// in `LOCALES`.
#[test]
fn locales_and_folders_match() {
    let mut folders = translated_locales();
    folders.push("en".into());
    folders.sort();
    let mut codes: Vec<String> = sayso_i18n::LOCALES.iter().map(|l| l.code.to_string()).collect();
    codes.sort();
    assert_eq!(folders, codes, "the folders in locales/ and LOCALES differ");
}

/// Each translated catalog has the keys of English, no others, and the same
/// arguments in each text. A plural can have other categories than English
/// ("many" in Spanish, only "other" in Japanese), so plurals compare by their
/// base key. Each plural needs a text for each category that the counts of its
/// language give, and for "other".
#[test]
fn every_locale_matches_english() {
    let en = catalog("en");
    let plural = Regex::new(r"^(.+)\.(zero|one|two|few|many|other)$").unwrap();
    let base = |key: &str| plural.captures(key).map_or(key.to_string(), |c| c[1].to_string());
    let en_plurals: BTreeSet<String> = en.keys().filter(|k| plural.is_match(k)).map(|k| base(k)).collect();
    let mut failures = Vec::new();
    for locale in translated_locales() {
        let locale = locale.as_str();
        let other = catalog(locale);
        let mut problems = Vec::new();
        let keys = |c: &BTreeMap<String, String>| c.keys().map(|k| base(k)).collect::<BTreeSet<_>>();
        let (en_keys, keys) = (keys(&en), keys(&other));
        for missing in en_keys.difference(&keys) {
            problems.push(format!("missing {missing}"));
        }
        for extra in keys.difference(&en_keys) {
            problems.push(format!("not in en.yml: {extra}"));
        }
        for key in &en_plurals {
            for category in sayso_i18n::__plural_categories(locale) {
                if !other.contains_key(&format!("{key}.{category}")) {
                    problems.push(format!("{key} has no \"{category}\""));
                }
            }
        }
        for (key, text) in &other {
            let en_text = en.get(key).or_else(|| en.get(&format!("{}.other", base(key))));
            if let Some(en_text) = en_text
                && placeholders(en_text) != placeholders(text)
            {
                problems.push(format!("{key} has other arguments than English"));
            }
        }
        if !problems.is_empty() {
            failures.push(format!("{locale}:\n  {}", problems.join("\n  ")));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Each `t!` key in the workspace is in `en.yml`. A typo in a key would
/// otherwise show the key in the interface.
#[test]
fn every_key_in_the_code_is_in_english() {
    let en = catalog("en");
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    // `t!("key"` and `t!(os_text!("key", "key")`, with or without a path.
    let call = Regex::new(r#"\bt!\(\s*(?:(?:crate::)?(?:shell::)?os_text!\(([^)]*)\)|"([^"]+)")(\s*,\s*count\s*=)?"#).unwrap();
    let literal = Regex::new(r#""([^"]+)""#).unwrap();
    let mut missing = Vec::new();
    let mut found = 0;
    for file in rust_files(crates) {
        let source = code(&file);
        for c in call.captures_iter(&source) {
            let keys: Vec<String> = match (c.get(1), c.get(2)) {
                (Some(arms), _) => literal.captures_iter(arms.as_str()).map(|l| l[1].to_string()).collect(),
                (_, Some(key)) => vec![key.as_str().to_string()],
                _ => continue,
            };
            let is_plural = c.get(3).is_some();
            for key in keys {
                found += 1;
                let known = if is_plural { en.contains_key(&format!("{key}.other")) } else { en.contains_key(&key) };
                if !known {
                    missing.push(format!("{}: {key}", file.strip_prefix(crates).unwrap_or(&file).display()));
                }
            }
        }
    }
    // A key that is not a literal cannot be checked, so the code has none.
    let any_call = Regex::new(r"\bt!\(").unwrap();
    for file in rust_files(crates) {
        let source = code(&file);
        let checked = call.find_iter(&source).count();
        let all = any_call.find_iter(&source).count();
        if all != checked {
            missing.push(format!("{}: {} t! calls without a literal key", file.strip_prefix(crates).unwrap_or(&file).display(), all - checked));
        }
    }
    assert!(found > 0, "the scan found no t! calls");
    assert!(missing.is_empty(), "keys not in en.yml:\n  {}", missing.join("\n  "));
}

/// Each English text is used: a key that no code reads is dead text that
/// translators would still translate.
#[test]
fn every_english_key_is_used() {
    let en = catalog("en");
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut sources = String::new();
    for file in rust_files(crates) {
        sources.push_str(&code(&file));
    }
    let plural = Regex::new(r"^(.+)\.(zero|one|two|few|many|other)$").unwrap();
    let unused: Vec<&String> = en
        .keys()
        .filter(|key| {
            let base = plural.captures(key).map_or(key.as_str(), |c| c.get(1).unwrap().as_str());
            // scripts/i18n-system-files.py reads the system texts.
            !base.starts_with("system.") && !sources.contains(&format!("\"{base}\""))
        })
        .collect();
    assert!(unused.is_empty(), "keys in en.yml that no code uses:\n  {}", unused.iter().map(|k| k.as_str()).collect::<Vec<_>>().join("\n  "));
}

/// The source of a file without its comment lines, so that examples in docs
/// do not count.
fn code(file: &Path) -> String {
    let source = std::fs::read_to_string(file).unwrap();
    source.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n")
}

fn rust_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target" || n == "workspace-hack") {
                continue;
            }
            out.extend(rust_files(&path));
        } else if path.extension().is_some_and(|e| e == "rs") && !path.ends_with("tests/catalog.rs") {
            out.push(path);
        }
    }
    out
}

#[test]
fn a_missing_key_shows_the_key() {
    assert_eq!(sayso_i18n::t!("no.such.key"), "no.such.key");
}

#[test]
fn plural_and_arguments() {
    sayso_i18n::set_locale("en");
    assert!(sayso_i18n::t!("settings.general.config_problems", count = 1usize).starts_with("config.toml has 1 problem."));
    assert!(sayso_i18n::t!("settings.general.config_problems", count = 3usize).starts_with("config.toml has 3 problems."));
    assert_eq!(sayso_i18n::t!("settings.general.version", version = "1.2.3"), "Version 1.2.3");
}

/// The full keys of a catalog file. The files use a small part of YAML:
/// two spaces for each level, and `key: "text"` or `key:` with children.
fn file_keys(text: &str) -> Vec<String> {
    let mut path: Vec<String> = Vec::new();
    let mut keys = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = (line.len() - trimmed.len()) / 2;
        let Some((key, value)) = trimmed.split_once(':') else { continue };
        path.truncate(indent);
        if value.trim().is_empty() {
            path.push(key.to_string());
        } else if key != "_version" {
            keys.push(path.iter().chain([&key.to_string()]).cloned().collect::<Vec<_>>().join("."));
        }
    }
    keys
}

/// Files are named for their locale, and no two files of a locale have the
/// same key: rust-i18n merges the files, so a second text for a key would
/// replace the first without an error.
#[test]
fn files_do_not_share_keys() {
    let locales = Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
    for dir in std::fs::read_dir(&locales).unwrap().flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        let locale = dir.file_name().unwrap().to_string_lossy().to_string();
        let mut seen: BTreeMap<String, String> = BTreeMap::new();
        for file in std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()) {
            let name = file.file_name().unwrap().to_string_lossy().to_string();
            assert!(name.ends_with(&format!(".{locale}.yml")), "{name} is in {locale}/ but is not named <surface>.{locale}.yml");
            for key in file_keys(&std::fs::read_to_string(&file).unwrap()) {
                if let Some(other) = seen.insert(key.clone(), name.clone()) {
                    panic!("{locale}/{name} and {locale}/{other} both have the key {key}");
                }
            }
        }
        // The parser must see what rust-i18n sees.
        if let Some(messages) = sayso_i18n::__messages(&locale) {
            assert_eq!(seen.len(), messages.len(), "{locale}: the test parser and rust-i18n find a different number of keys");
        }
    }
}
