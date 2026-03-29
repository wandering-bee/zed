use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "locales"]
#[include = "*.json"]
struct EmbeddedLocales;

/// Pre-interned translation string. Cheap to clone.
type I18nString = Arc<str>;

/// A single loaded language: key → translated string (pre-interned).
type TranslationMap = HashMap<String, I18nString>;

/// All loaded languages: language code → translation map.
type LanguageRegistry = HashMap<String, TranslationMap>;

/// Callback invoked when the active language changes.
/// The host application should use this to trigger UI refresh
/// (e.g. `cx.notify()` in GPUI).
/// Wrapped in Arc so callbacks can be cloned out of the lock for safe invocation.
type LanguageChangedCallback = Arc<dyn Fn(&str) + Send + Sync>;

static SERVICE: RwLock<Option<I18nService>> = RwLock::new(None);

struct I18nService {
    languages: LanguageRegistry,
    active_language: String,
    on_language_changed: Vec<LanguageChangedCallback>,
    external_locales_dir: Option<PathBuf>,
}

/// Result of loading a single locale file.
#[derive(Debug)]
pub struct LocaleLoadResult {
    pub language: String,
    pub entries: usize,
    pub source: LocaleSource,
}

#[derive(Debug)]
pub enum LocaleSource {
    Embedded,
    External,
}

/// Report returned by `init()` describing what was loaded.
#[derive(Debug)]
pub struct I18nInitReport {
    pub loaded: Vec<LocaleLoadResult>,
    pub errors: Vec<String>,
    pub active_language: String,
}

fn parse_locale_file(data: &[u8]) -> Result<TranslationMap, String> {
    let raw: HashMap<String, String> =
        serde_json::from_slice(data).map_err(|e| format!("JSON parse error: {e}"))?;

    Ok(raw
        .into_iter()
        .map(|(k, v)| (k, Arc::from(v.as_str())))
        .collect())
}

fn load_embedded_locales(
    languages: &mut LanguageRegistry,
    report: &mut I18nInitReport,
) {
    for filename in <EmbeddedLocales as rust_embed::Embed>::iter() {
        let filename_str: &str = &filename;
        let Some(lang_code) = filename_str.strip_suffix(".json") else {
            continue;
        };

        let Some(file) = <EmbeddedLocales as rust_embed::Embed>::get(filename_str) else {
            report
                .errors
                .push(format!("embedded: failed to read {filename_str}"));
            continue;
        };

        match parse_locale_file(&file.data) {
            Ok(map) => {
                let entries = map.len();
                languages.insert(lang_code.to_string(), map);
                report.loaded.push(LocaleLoadResult {
                    language: lang_code.to_string(),
                    entries,
                    source: LocaleSource::Embedded,
                });
            }
            Err(err) => {
                report
                    .errors
                    .push(format!("embedded: {filename_str}: {err}"));
            }
        }
    }
}

fn load_external_locales(
    dir: &Path,
    languages: &mut LanguageRegistry,
    report: &mut I18nInitReport,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            report
                .errors
                .push(format!("external: cannot read dir {}: {err}", dir.display()));
            return;
        }
    };

    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path = entry.path();

        let Some(ext) = path.extension() else {
            continue;
        };
        if ext != "json" {
            continue;
        }

        let Some(lang_code) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };

        match std::fs::read(&path) {
            Ok(data) => match parse_locale_file(&data) {
                Ok(map) => {
                    let entries = map.len();
                    // External overrides embedded
                    languages.insert(lang_code.to_string(), map);
                    report.loaded.push(LocaleLoadResult {
                        language: lang_code.to_string(),
                        entries,
                        source: LocaleSource::External,
                    });
                }
                Err(err) => {
                    report
                        .errors
                        .push(format!("external: {}: {err}", path.display()));
                }
            },
            Err(err) => {
                report
                    .errors
                    .push(format!("external: cannot read {}: {err}", path.display()));
            }
        }
    }
}

/// Initialize the i18n service.
///
/// Loads translations from two sources (external overrides embedded):
/// 1. Embedded locales compiled into the binary
/// 2. External JSON files from `external_dir` (if provided)
///
/// If the requested language is not available after loading,
/// falls back to "en".
pub fn init(language: &str, external_dir: Option<&Path>) -> I18nInitReport {
    let mut languages = LanguageRegistry::new();
    let mut report = I18nInitReport {
        loaded: vec![],
        errors: vec![],
        active_language: String::new(),
    };

    // Phase 1: embedded locales (baseline)
    load_embedded_locales(&mut languages, &mut report);

    // Phase 2: external locales (override)
    if let Some(dir) = external_dir {
        load_external_locales(dir, &mut languages, &mut report);
    }

    let active_language = if languages.contains_key(language) {
        language.to_string()
    } else {
        if language != "en" {
            report.errors.push(format!(
                "requested language '{language}' not found, falling back to 'en'"
            ));
        }
        "en".to_string()
    };

    report.active_language = active_language.clone();

    for result in &report.loaded {
        log::info!(
            "i18n: loaded {:?} '{}' ({} entries)",
            result.source,
            result.language,
            result.entries,
        );
    }
    for err in &report.errors {
        log::warn!("i18n: {err}");
    }

    let mut service = SERVICE.write();
    *service = Some(I18nService {
        languages,
        active_language,
        on_language_changed: vec![],
        external_locales_dir: external_dir.map(|p| p.to_path_buf()),
    });

    report
}

/// Reload locale files without losing registered callbacks.
///
/// Rebuilds translation tables from embedded + external sources,
/// preserving the active language, external dir path, and all
/// `on_language_changed` callbacks.
pub fn reload() -> I18nInitReport {
    let mut service = SERVICE.write();
    let Some(svc) = service.as_mut() else {
        return I18nInitReport {
            loaded: vec![],
            errors: vec!["i18n: reload called before init".to_string()],
            active_language: "en".to_string(),
        };
    };

    let mut languages = LanguageRegistry::new();
    let mut report = I18nInitReport {
        loaded: vec![],
        errors: vec![],
        active_language: String::new(),
    };

    load_embedded_locales(&mut languages, &mut report);

    if let Some(dir) = &svc.external_locales_dir {
        load_external_locales(dir, &mut languages, &mut report);
    }

    if !languages.contains_key(&svc.active_language) {
        report.errors.push(format!(
            "active language '{}' no longer available after reload, falling back to 'en'",
            svc.active_language
        ));
        svc.active_language = "en".to_string();
    }

    report.active_language = svc.active_language.clone();
    svc.languages = languages;

    for result in &report.loaded {
        log::info!(
            "i18n: reloaded {:?} '{}' ({} entries)",
            result.source,
            result.language,
            result.entries,
        );
    }
    for err in &report.errors {
        log::warn!("i18n: {err}");
    }

    report
}

/// Switch the active language at runtime.
///
/// Returns `true` if the language exists and was switched.
/// Invokes all registered `on_language_changed` callbacks so the
/// host application can trigger UI refresh.
pub fn set_language(language: &str) -> bool {
    // Update state and clone callbacks under the lock, then invoke
    // callbacks outside the lock to avoid deadlock if they call t() etc.
    let callbacks = {
        let mut service = SERVICE.write();
        let Some(svc) = service.as_mut() else {
            return false;
        };

        if !svc.languages.contains_key(language) {
            log::warn!("i18n: set_language('{language}') failed: language not loaded");
            return false;
        }

        if svc.active_language == language {
            return true;
        }

        svc.active_language = language.to_string();
        svc.on_language_changed.clone()
    };
    // Write lock released. Callbacks can safely call t() / active_language().

    for callback in &callbacks {
        callback(language);
    }

    true
}

/// Register a callback to be invoked when the active language changes.
///
/// The host application should use this to trigger UI refresh, e.g.:
/// ```ignore
/// zed_i18n::on_language_changed(Arc::new(move |_lang| {
///     // trigger GPUI global refresh
/// }));
/// ```
pub fn on_language_changed(callback: LanguageChangedCallback) {
    let mut service = SERVICE.write();
    if let Some(svc) = service.as_mut() {
        svc.on_language_changed.push(callback);
    }
}

/// Get the current active language code.
pub fn active_language() -> String {
    let service = SERVICE.read();
    match service.as_ref() {
        Some(svc) => svc.active_language.clone(),
        None => "en".to_string(),
    }
}

/// List all available language codes.
pub fn available_languages() -> Vec<String> {
    let service = SERVICE.read();
    match service.as_ref() {
        Some(svc) => svc.languages.keys().cloned().collect(),
        None => vec![],
    }
}

/// Translate a key using the active language.
///
/// Returns a pre-interned `Arc<str>` — cheap to clone, no allocation
/// on the hot path when the key exists in the translation table.
///
/// If the service is not initialized or the key has no translation,
/// returns the key itself (allocated once). This ensures graceful
/// degradation: untranslated strings display as English.
pub fn t(key: &str) -> Arc<str> {
    let service = SERVICE.read();
    let Some(svc) = service.as_ref() else {
        return Arc::from(key);
    };

    svc.languages
        .get(&svc.active_language)
        .and_then(|map| map.get(key))
        .cloned()
        .unwrap_or_else(|| Arc::from(key))
}

/// Translate with format arguments.
///
/// Placeholders use `{0}`, `{1}`, etc.
/// Note: this allocates a new String for the formatted result.
/// Use `t()` for static strings in the render hot path.
pub fn t_fmt(key: &str, args: &[&str]) -> String {
    let template = t(key);
    let mut result = template.to_string();
    for (i, arg) in args.iter().enumerate() {
        result = result.replace(&format!("{{{i}}}"), arg);
    }
    result
}

/// Convenience macro for i18n string lookup.
///
/// Usage:
/// ```ignore
/// use zed_i18n::i18n;
///
/// let label = i18n!("New Thread");
/// let msg = i18n!("Found {0} results", &count.to_string());
/// ```
#[macro_export]
macro_rules! i18n {
    ($key:expr) => {
        $crate::t($key)
    };
    ($key:expr, $($arg:expr),+ $(,)?) => {
        $crate::t_fmt($key, &[$($arg),+])
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Tests share global SERVICE state, so they must run serially.
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_fallback_without_init() {
        let _lock = TEST_MUTEX.lock();
        // Reset to uninitialized state
        *SERVICE.write() = None;
        let s = t("Hello");
        assert_eq!(&*s, "Hello");
    }

    #[test]
    fn test_init_report() {
        let _lock = TEST_MUTEX.lock();
        let report = init("en", None);
        assert!(report.errors.is_empty() || report.active_language == "en");
        assert!(!report.loaded.is_empty());
    }

    #[test]
    fn test_fallback_missing_key() {
        let _lock = TEST_MUTEX.lock();
        init("en", None);
        let s = t("nonexistent key");
        assert_eq!(&*s, "nonexistent key");
    }

    #[test]
    fn test_format() {
        let _lock = TEST_MUTEX.lock();
        init("en", None);
        let result = t_fmt("Found {0} results in {1}", &["42", "src/"]);
        assert_eq!(result, "Found 42 results in src/");
    }

    #[test]
    fn test_set_language_callback() {
        let _lock = TEST_MUTEX.lock();
        init("en", None);
        let called = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called_clone = called.clone();
        on_language_changed(Arc::new(move |_| {
            called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
        }));

        let ok = set_language("zh-CN");
        assert!(ok);
        assert!(called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_translation_zh_cn() {
        let _lock = TEST_MUTEX.lock();
        let report = init("zh-CN", None);
        assert_eq!(report.active_language, "zh-CN");
        let s = t("New Thread");
        assert_eq!(&*s, "新建会话");
    }

    #[test]
    fn test_reload_preserves_callbacks() {
        let _lock = TEST_MUTEX.lock();
        init("en", None);
        let called = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called_clone = called.clone();
        on_language_changed(Arc::new(move |_| {
            called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
        }));

        let report = reload();
        assert!(report.errors.is_empty());

        set_language("zh-CN");
        assert!(called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_arc_is_cheap() {
        let _lock = TEST_MUTEX.lock();
        init("zh-CN", None);
        let s1 = t("New Thread");
        let s2 = t("New Thread");
        assert!(Arc::ptr_eq(&s1, &s2));
    }
}
