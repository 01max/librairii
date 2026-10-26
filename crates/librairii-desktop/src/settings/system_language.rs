use librairii_app::localization::Language;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LanguagePlatform {
    MacOs,
    Linux,
}
impl LanguagePlatform {
    pub const fn current() -> Self {
        #[cfg(target_os = "macos")]
        {
            Self::MacOs
        }
        #[cfg(target_os = "linux")]
        {
            Self::Linux
        }
    }
}
/// Variable OS effects, independent of library/profile discovery. Test callers
/// supply observations without changing process-global environment or defaults.
pub trait SystemLanguageSource {
    fn apple_languages(&self) -> Option<String>;
    fn locale(&self, variable: &str) -> Option<String>;
}
pub fn detect_language(platform: LanguagePlatform, source: &impl SystemLanguageSource) -> Language {
    if platform == LanguagePlatform::MacOs
        && let Some(languages) = source.apple_languages()
    {
        let first = languages
            .trim_matches(['(', ')', '\n', ' ', '\t'])
            .split(',')
            .next()
            .unwrap_or_default()
            .trim_matches(['"', ' ', '\n', '\t']);
        return Language::from_system([first]);
    }
    for variable in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Some(value) = source.locale(variable).filter(|value| !value.is_empty()) {
            return Language::from_system([value.as_str()]);
        }
    }
    Language::English
}

#[derive(Debug)]
pub struct NativeLanguageSource;
impl SystemLanguageSource for NativeLanguageSource {
    fn apple_languages(&self) -> Option<String> {
        #[cfg(target_os = "macos")]
        {
            apple_languages()
        }
        #[cfg(target_os = "linux")]
        {
            None
        }
    }
    fn locale(&self, variable: &str) -> Option<String> {
        std::env::var(variable).ok()
    }
}
pub fn system_language() -> Language {
    detect_language(LanguagePlatform::current(), &NativeLanguageSource)
}

#[cfg(target_os = "macos")]
fn apple_languages() -> Option<String> {
    use std::{
        io::Read,
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    };
    let mut child = Command::new("/usr/bin/defaults")
        .args(["read", "-g", "AppleLanguages"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    };
    let reader = match thread::Builder::new()
        .name("system-language".into())
        .spawn(move || {
            let mut bytes = Vec::new();
            stdout.take(8193).read_to_end(&mut bytes).map(|_| bytes)
        }) {
        Ok(reader) => reader,
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
    };
    let deadline = Instant::now() + Duration::from_secs(2);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let bytes = reader.join().ok()?.ok()?;
    if !status?.success() || bytes.len() > 8192 {
        return None;
    }
    String::from_utf8(bytes).ok()
}
