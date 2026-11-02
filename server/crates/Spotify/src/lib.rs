//! Spotify application profile: the app-specific knowledge the generic Memory Sentry monitor needs to
//! find and classify the Spotify process tree. This crate is the seam a different desktop application
//! would replace - the executable to enumerate, and the Chromium/Electron command-line tokens that
//! identify each child process's role and stable identity. The policy engine (`memory-sentry-core`),
//! the Win32 layer and the runtime hold no Spotify knowledge of their own; they reach the managed app
//! only through the [`AppProfile`] trait implemented here, so adding a project means adding one crate
//! like this and wiring it in a single place.

use memory_sentry_core::{AppProfile, ProcessRole};
use std::collections::BTreeSet;

// The single executable base name the Spotify desktop client runs as. Lowercased so it matches the
// case-insensitive comparison the native Toolhelp enumeration performs; the native path filters on it
// and the PowerShell fallback derives the WMI `Name=` filter from [`SpotifyProfile::process_image_name`].
const TARGET_EXECUTABLES: &[&str] = &["spotify.exe"];

/// The Spotify desktop client (a Chromium/CEF application) treated as a managed process tree.
#[derive(Clone, Copy, Debug, Default)]
pub struct SpotifyProfile;

impl SpotifyProfile {
    /// A stateless profile; constructing it never allocates and carries no configuration.
    pub fn new() -> Self {
        SpotifyProfile
    }
}

/// Reads a single Chromium/Electron switch value from a raw command line. Accepts both `--key=value`
/// and `--key value` shapes; the lowercase haystack and the source have equal byte lengths (only
/// ASCII A-Z changes) so the byte index transfers directly.
fn extract_flag_value(cmd: &str, flag: &str) -> Option<String> {
    let lower = cmd.to_ascii_lowercase();
    let index = lower.find(flag)?;
    let after = cmd[index + flag.len()..].trim_start();
    let after = after.strip_prefix('=').unwrap_or(after).trim_start();
    let token = after.split_whitespace().next()?;
    let token = token.trim_matches('"');
    (!token.is_empty()).then(|| token.to_string())
}

impl AppProfile for SpotifyProfile {
    fn target_executables(&self) -> &'static [&'static str] {
        TARGET_EXECUTABLES
    }

    fn process_image_name(&self) -> &'static str {
        "Spotify.exe"
    }

    fn is_child_process(&self, cmd: &str) -> bool {
        let lower = cmd.to_ascii_lowercase();
        lower.contains("--type=") || lower.contains("--type =")
    }

    fn classify_role(
        &self,
        cmd: &str,
        pid: u32,
        root_pid: Option<u32>,
        audio_pids: &BTreeSet<u32>,
    ) -> ProcessRole {
        // The audio-session set comes from the OS (WASAPI), independent of the command line, so it is
        // the authoritative signal and wins over any token the process happens to carry.
        if audio_pids.contains(&pid) {
            return ProcessRole::Audio;
        }
        let lower = cmd.to_ascii_lowercase();
        if lower.contains("audio.mojom.audioservice")
            || lower.contains("--utility-sub-type=audio")
            || lower.contains("--utility-sub-type = audio")
            || lower.contains("audio-service")
        {
            return ProcessRole::Audio;
        }
        if Some(pid) == root_pid {
            return ProcessRole::Main;
        }
        if lower.contains("--type=renderer") || lower.contains("--type = renderer") {
            return ProcessRole::Renderer;
        }
        if lower.contains("--type=gpu-process") || lower.contains("--type = gpu-process") {
            return ProcessRole::Gpu;
        }
        if lower.contains("--type=utility") || lower.contains("--type = utility") {
            return ProcessRole::Utility;
        }
        if lower.contains("crashpad") {
            return ProcessRole::Crashpad;
        }
        ProcessRole::Helper
    }

    fn sub_label(&self, cmd: &str, role: ProcessRole) -> Option<String> {
        match role {
            ProcessRole::Renderer => {
                if let Some(id) = extract_flag_value(cmd, "--renderer-client-id") {
                    return Some(format!("#{id}"));
                }
                if cmd.to_ascii_lowercase().contains("--extension-process") {
                    return Some("ext".to_string());
                }
                None
            }
            ProcessRole::Utility => {
                let sub = extract_flag_value(cmd, "--utility-sub-type")?;
                let head = sub.split('.').next().unwrap_or("").trim();
                (!head.is_empty()).then(|| head.to_ascii_lowercase())
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAIN: &str = "spotify.exe";
    const RENDERER: &str = "spotify.exe --type=renderer --renderer-client-id=7";
    const GPU: &str = "spotify.exe --type=gpu-process";
    const UTILITY: &str = "spotify.exe --type=utility --utility-sub-type=network.mojom.NetworkService";

    fn audio_none() -> BTreeSet<u32> {
        BTreeSet::new()
    }

    #[test]
    fn target_names_are_the_spotify_executable() {
        let profile = SpotifyProfile::new();
        assert_eq!(profile.target_executables(), ["spotify.exe"]);
        assert_eq!(profile.process_image_name(), "Spotify.exe");
    }

    #[test]
    fn child_processes_are_recognized_by_the_type_switch() {
        let profile = SpotifyProfile::new();
        assert!(profile.is_child_process(RENDERER));
        assert!(profile.is_child_process("Spotify.exe --type = gpu-process"));
        assert!(!profile.is_child_process(MAIN));
    }

    #[test]
    fn roles_follow_the_chromium_command_line_tokens() {
        let profile = SpotifyProfile::new();
        assert_eq!(profile.classify_role(MAIN, 1, Some(1), &audio_none()), ProcessRole::Main);
        assert_eq!(
            profile.classify_role(RENDERER, 2, Some(1), &audio_none()),
            ProcessRole::Renderer
        );
        assert_eq!(profile.classify_role(GPU, 3, Some(1), &audio_none()), ProcessRole::Gpu);
        assert_eq!(
            profile.classify_role(UTILITY, 4, Some(1), &audio_none()),
            ProcessRole::Utility
        );
        assert_eq!(
            profile.classify_role("spotify.exe --type=renderer --crashpad-handler", 5, Some(1), &audio_none()),
            ProcessRole::Renderer,
            "the renderer type token wins over a stray crashpad substring"
        );
        assert_eq!(
            profile.classify_role("spotify.exe --crashpad-handler", 6, Some(1), &audio_none()),
            ProcessRole::Crashpad
        );
        assert_eq!(
            profile.classify_role("spotify.exe --no-capture-audio", 7, Some(1), &audio_none()),
            ProcessRole::Helper,
            "an unclassified child falls back to Helper"
        );
    }

    #[test]
    fn an_audio_session_pid_overrides_everything_else() {
        let profile = SpotifyProfile::new();
        let mut audio = BTreeSet::new();
        audio.insert(9);
        // Even a command line that would otherwise classify as a renderer is Audio when the OS reports
        // it owning the live audio session.
        assert_eq!(profile.classify_role(RENDERER, 9, Some(1), &audio), ProcessRole::Audio);
    }

    #[test]
    fn sub_labels_disambiguate_same_role_processes() {
        let profile = SpotifyProfile::new();
        assert_eq!(profile.sub_label(RENDERER, ProcessRole::Renderer).as_deref(), Some("#7"));
        assert_eq!(
            profile.sub_label("spotify.exe --type=renderer --extension-process", ProcessRole::Renderer).as_deref(),
            Some("ext")
        );
        assert_eq!(profile.sub_label("spotify.exe --type=renderer", ProcessRole::Renderer), None);
        assert_eq!(profile.sub_label(UTILITY, ProcessRole::Utility).as_deref(), Some("network"));
        assert_eq!(profile.sub_label(MAIN, ProcessRole::Main), None);
    }

    #[test]
    fn identity_combines_role_label_and_sub_label() {
        let profile = SpotifyProfile::new();
        assert_eq!(profile.identity(RENDERER, ProcessRole::Renderer), "Renderer:#7");
        assert_eq!(profile.identity(UTILITY, ProcessRole::Utility), "Utility:network");
        assert_eq!(profile.identity(MAIN, ProcessRole::Main), "Main");
    }
}
