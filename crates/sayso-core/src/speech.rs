//! Speech providers: cloud services that run the final pass.
//!
//! This module holds the config types, the known providers with their
//! models, and the contract for one request. The HTTP code lives in
//! `sayso-transcribe`. API keys are in the Keychain, never in `config.toml`.

use crate::config::is_local_url;
use crate::languages;
use crate::models::{EngineKind, ModelId, ModelInfo};
use sayso_i18n::t;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Which API a speech provider speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechProviderKind {
    OpenAi,
    Groq,
    Mistral,
    ElevenLabs,
    Deepgram,
    AssemblyAi,
    /// Any server with `POST {base_url}/audio/transcriptions`: Together,
    /// speaches, LocalAI, vLLM, or a proxy.
    OpenAiCompatible,
}

/// One model a known provider offers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KnownModel {
    /// The id Sayso sends to the provider.
    pub id: &'static str,
    pub name: &'static str,
    /// The provider takes dictionary words as a hint for this model.
    pub vocabulary: bool,
    /// 1 (slow) to 5 (fast).
    pub speed: u8,
}

impl SpeechProviderKind {
    pub const ALL: [SpeechProviderKind; 7] = [
        SpeechProviderKind::OpenAi,
        SpeechProviderKind::Groq,
        SpeechProviderKind::ElevenLabs,
        SpeechProviderKind::Deepgram,
        SpeechProviderKind::AssemblyAi,
        SpeechProviderKind::Mistral,
        SpeechProviderKind::OpenAiCompatible,
    ];

    /// The name to show, and the default name of a new provider. "Custom
    /// endpoint" is in the interface language.
    pub fn label(self) -> String {
        match self {
            SpeechProviderKind::OpenAi => "OpenAI".into(),
            SpeechProviderKind::Groq => "Groq".into(),
            SpeechProviderKind::Mistral => "Mistral".into(),
            SpeechProviderKind::ElevenLabs => "ElevenLabs".into(),
            SpeechProviderKind::Deepgram => "Deepgram".into(),
            SpeechProviderKind::AssemblyAi => "AssemblyAI".into(),
            SpeechProviderKind::OpenAiCompatible => t!("core.speech.custom_endpoint"),
        }
    }

    /// The API root. None for a custom endpoint, where the user gives it.
    pub fn default_base_url(self) -> Option<&'static str> {
        match self {
            SpeechProviderKind::OpenAi => Some("https://api.openai.com/v1"),
            SpeechProviderKind::Groq => Some("https://api.groq.com/openai/v1"),
            SpeechProviderKind::Mistral => Some("https://api.mistral.ai/v1"),
            SpeechProviderKind::ElevenLabs => Some("https://api.elevenlabs.io/v1"),
            SpeechProviderKind::Deepgram => Some("https://api.deepgram.com/v1"),
            SpeechProviderKind::AssemblyAi => Some("https://sync.assemblyai.com/v1"),
            SpeechProviderKind::OpenAiCompatible => None,
        }
    }

    /// Where the user creates an API key.
    pub fn key_url(self) -> Option<&'static str> {
        match self {
            SpeechProviderKind::OpenAi => Some("https://platform.openai.com/api-keys"),
            SpeechProviderKind::Groq => Some("https://console.groq.com/keys"),
            SpeechProviderKind::Mistral => Some("https://console.mistral.ai/api-keys"),
            SpeechProviderKind::ElevenLabs => Some("https://elevenlabs.io/app/settings/api-keys"),
            SpeechProviderKind::Deepgram => Some("https://console.deepgram.com"),
            SpeechProviderKind::AssemblyAi => Some("https://www.assemblyai.com/dashboard/api-keys"),
            SpeechProviderKind::OpenAiCompatible => None,
        }
    }

    /// The models Sayso offers for this provider. A provider's `models` list
    /// in `config.toml` adds more.
    pub fn known_models(self) -> &'static [KnownModel] {
        const fn m(id: &'static str, name: &'static str, vocabulary: bool, speed: u8) -> KnownModel {
            KnownModel { id, name, vocabulary, speed }
        }
        const OPEN_AI: &[KnownModel] = &[
            m("gpt-4o-transcribe", "GPT-4o Transcribe", true, 4),
            m("gpt-4o-mini-transcribe", "GPT-4o mini Transcribe", true, 5),
            m("gpt-transcribe", "GPT Transcribe", true, 4),
            m("whisper-1", "Whisper", true, 3),
        ];
        const GROQ: &[KnownModel] = &[
            m("whisper-large-v3-turbo", "Whisper large-v3 turbo", true, 5),
            m("whisper-large-v3", "Whisper large-v3", true, 5),
        ];
        const MISTRAL: &[KnownModel] = &[m("voxtral-mini-latest", "Voxtral Mini Transcribe", true, 4)];
        const ELEVEN_LABS: &[KnownModel] = &[m("scribe_v2", "Scribe v2", true, 3)];
        const DEEPGRAM: &[KnownModel] = &[
            m("nova-3", "Nova-3", true, 5),
            m("nova-2", "Nova-2", false, 5),
        ];
        const ASSEMBLY_AI: &[KnownModel] =
            &[m("universal-3-5-pro", "Universal 3.5 Pro", true, 4)];
        match self {
            SpeechProviderKind::OpenAi => OPEN_AI,
            SpeechProviderKind::Groq => GROQ,
            SpeechProviderKind::Mistral => MISTRAL,
            SpeechProviderKind::ElevenLabs => ELEVEN_LABS,
            SpeechProviderKind::Deepgram => DEEPGRAM,
            SpeechProviderKind::AssemblyAi => ASSEMBLY_AI,
            SpeechProviderKind::OpenAiCompatible => &[],
        }
    }
}

impl KnownModel {
    /// A short note about the model, in the interface language.
    pub fn description(&self) -> String {
        match self.id {
            "gpt-4o-transcribe" => t!("core.speech.model.gpt_4o_transcribe"),
            "gpt-4o-mini-transcribe" => t!("core.speech.model.gpt_4o_mini_transcribe"),
            "gpt-transcribe" => t!("core.speech.model.gpt_transcribe"),
            "whisper-1" => t!("core.speech.model.whisper_1"),
            "whisper-large-v3-turbo" => t!("core.speech.model.whisper_large_v3_turbo"),
            "whisper-large-v3" => t!("core.speech.model.whisper_large_v3"),
            "voxtral-mini-latest" => t!("core.speech.model.voxtral_mini_latest"),
            "scribe_v2" => t!("core.speech.model.scribe_v2"),
            "nova-3" => t!("core.speech.model.nova_3"),
            "nova-2" => t!("core.speech.model.nova_2"),
            "universal-3-5-pro" => t!("core.speech.model.universal_3_5_pro"),
            _ => String::new(),
        }
    }
}

/// A configured speech provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeechProvider {
    /// Used in model ids (`"<id>:<model>"`), so it has no colon.
    pub id: String,
    pub name: String,
    pub kind: SpeechProviderKind,
    /// The API root. Needed for a custom endpoint. For a known provider it
    /// replaces the default address (a proxy, a regional host).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Keychain account name. None for a server that needs no key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_account: Option<String>,
    /// Model ids to offer besides the known ones. A custom endpoint needs at least one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<String>,
}

impl SpeechProvider {
    /// The API root without a trailing slash. None when a custom endpoint has no address.
    pub fn base_url(&self) -> Option<String> {
        let url = self.base_url.as_deref().map(str::trim).filter(|u| !u.is_empty()).or(self.kind.default_base_url())?;
        Some(url.trim_end_matches('/').to_string())
    }

    /// True when audio leaves this Mac.
    pub fn is_cloud(&self) -> bool {
        self.base_url().is_none_or(|url| !is_local_url(&url))
    }

    /// The id of one of this provider's models in the catalog.
    pub fn model_id(&self, model: &str) -> ModelId {
        ModelId::new(format!("{}:{model}", self.id))
    }

    /// This provider's models as catalog entries.
    pub fn catalog(&self) -> Vec<ModelInfo> {
        let entry = |id: &str, name: String, description: String, vocabulary: bool, speed: u8| ModelInfo {
            id: self.model_id(id),
            name,
            vendor: self.name.clone(),
            description,
            engine: EngineKind::Remote { provider: self.id.clone(), model: id.to_string() },
            size_bytes: 0,
            languages: languages::all(),
            final_pass: true,
            live_preview: false,
            vocabulary,
            speed,
            wer_percent: None,
            preview_wer_percent: None,
            punctuation: true,
            rtfx: None,
            card_url: None,
            credit: None,
            recommended: false,
            min_macos: 14,
        };
        let known = self.kind.known_models();
        let mut list: Vec<ModelInfo> =
            known.iter().map(|k| entry(k.id, k.name.to_string(), k.description(), k.vocabulary, k.speed)).collect();
        for id in &self.models {
            let id = id.trim();
            if id.is_empty() || known.iter().any(|k| k.id == id) || list.iter().any(|m| m.id == self.model_id(id)) {
                continue;
            }
            let place = if self.is_cloud() { t!("core.speech.runs_on_provider") } else { t!("core.speech.runs_on_own_server") };
            // The OpenAI-compatible API takes a prompt, so the words go there.
            let vocabulary = matches!(self.kind, SpeechProviderKind::OpenAiCompatible | SpeechProviderKind::OpenAi | SpeechProviderKind::Groq);
            let name = id.rsplit('/').next().unwrap_or(id).to_string();
            list.push(entry(id, name, place, vocabulary, 3));
        }
        list
    }
}

/// The `[speech]` section of `config.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Speech {
    pub providers: Vec<SpeechProvider>,
    /// The longest a cloud final pass may take. The upload is part of it.
    pub timeout_ms: u64,
}

impl Default for Speech {
    fn default() -> Self {
        Self { providers: Vec::new(), timeout_ms: 30_000 }
    }
}

impl Speech {
    pub fn provider(&self, id: &str) -> Option<&SpeechProvider> {
        self.providers.iter().find(|p| p.id == id)
    }

    /// The models of every configured provider.
    pub fn catalog(&self) -> Vec<ModelInfo> {
        self.providers.iter().flat_map(SpeechProvider::catalog).collect()
    }
}

/// One final pass for a speech provider.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeechRequest<'a> {
    /// The provider's model id, for example "gpt-4o-transcribe".
    pub model: &'a str,
    /// A complete 16 kHz mono 16-bit WAV file.
    pub wav: &'a [u8],
    /// ISO 639-1 code. None lets the provider detect the language.
    pub language: Option<&'a str>,
    /// Dictionary words. Sent as the provider's vocabulary hint, if it has one.
    pub vocabulary: &'a [String],
    /// A deadline for the whole call.
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SpeechError {
    #[error("timed out after {0} ms")]
    Timeout(u64),
    #[error("the provider is not set up: {0}")]
    NotConfigured(MissingSetup),
    #[error("HTTP {status}: {message}")]
    Http { status: u16, message: String },
    #[error("network error: {0}")]
    Network(String),
    #[error("the response had no transcript: {0}")]
    InvalidResponse(String),
}

/// What a speech provider needs before Sayso can use it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MissingSetup {
    #[error("no address is set")]
    Address,
    #[error("no API key is saved")]
    ApiKey,
}

impl SpeechError {
    /// A sentence for the overlay and the Models page, in the interface
    /// language: what went wrong and what to do. `Display` gives the English
    /// text for the log.
    pub fn user_message(&self, provider: &str) -> String {
        match self {
            SpeechError::Timeout(ms) => t!("core.speech.error.timeout", provider = provider, seconds = (*ms as f64 / 1000.0).round() as u64),
            SpeechError::NotConfigured(MissingSetup::Address) => t!("core.speech.error.no_address", provider = provider),
            SpeechError::NotConfigured(MissingSetup::ApiKey) => t!("core.speech.error.no_api_key", provider = provider),
            SpeechError::Http { status: 401 | 403, .. } => t!("core.speech.error.key_refused", provider = provider),
            SpeechError::Http { status: 429, .. } => t!("core.speech.error.rate_limit", provider = provider),
            SpeechError::Http { status, message } if message.is_empty() => t!("core.speech.error.http", provider = provider, status = status),
            // The text of the provider stays as it is.
            SpeechError::Http { status, message } => t!("core.speech.error.http_message", provider = provider, status = status, message = message),
            SpeechError::Network(_) => t!("core.speech.error.network", provider = provider),
            SpeechError::InvalidResponse(_) => t!("core.speech.error.invalid_response", provider = provider),
        }
    }
}

/// Split a cloud model id into the provider id and the provider's model id.
pub fn split_model_id(id: &ModelId) -> Option<(&str, &str)> {
    id.as_str().split_once(':')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(kind: SpeechProviderKind) -> SpeechProvider {
        SpeechProvider { id: "p".into(), name: "P".into(), kind, base_url: None, api_key_account: Some("speech.p".into()), models: vec![] }
    }

    #[test]
    fn known_providers_have_an_address_and_models() {
        for kind in SpeechProviderKind::ALL {
            let p = provider(kind);
            if kind == SpeechProviderKind::OpenAiCompatible {
                assert_eq!(p.base_url(), None);
                assert!(p.catalog().is_empty());
            } else {
                assert!(p.base_url().unwrap().starts_with("https://"));
                assert!(!p.catalog().is_empty(), "{kind:?} offers no model");
                assert!(p.is_cloud());
            }
        }
    }

    #[test]
    fn catalog_ids_carry_the_provider_and_split_back() {
        let p = provider(SpeechProviderKind::Groq);
        let first = &p.catalog()[0];
        assert_eq!(first.id.as_str(), "p:whisper-large-v3-turbo");
        assert_eq!(split_model_id(&first.id), Some(("p", "whisper-large-v3-turbo")));
        assert_eq!(first.engine, EngineKind::Remote { provider: "p".into(), model: "whisper-large-v3-turbo".into() });
        assert!(first.is_remote() && first.final_pass && !first.live_preview);
        assert_eq!(split_model_id(&ModelId::new("parakeet-unified-en")), None);
    }

    #[test]
    fn extra_models_are_added_once() {
        let mut p = provider(SpeechProviderKind::OpenAiCompatible);
        p.base_url = Some("http://localhost:8000/v1/".into());
        p.models = vec!["Systran/faster-whisper-large-v3".into(), " ".into(), "Systran/faster-whisper-large-v3".into()];
        let list = p.catalog();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "faster-whisper-large-v3");
        assert_eq!(p.base_url().as_deref(), Some("http://localhost:8000/v1"));
        assert!(!p.is_cloud(), "a local server is not cloud");

        let mut openai = provider(SpeechProviderKind::OpenAi);
        openai.models = vec!["whisper-1".into(), "gpt-5-transcribe".into()];
        let ids: Vec<_> = openai.catalog().into_iter().map(|m| m.id.0).collect();
        assert_eq!(ids.iter().filter(|i| *i == "p:whisper-1").count(), 1, "a known model is not repeated");
        assert!(ids.contains(&"p:gpt-5-transcribe".to_string()));
    }

    #[test]
    fn errors_say_what_to_do() {
        let e = SpeechError::Http { status: 401, message: "bad key".into() };
        assert!(e.user_message("OpenAI").contains("API key"));
        assert!(SpeechError::Timeout(30_000).user_message("Groq").contains("30 s"));
    }
}
