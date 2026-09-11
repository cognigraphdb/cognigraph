//! Shared, snapshot-based completion configuration for servers and harnesses.

use anyhow::{Context, Result, bail};

use super::{
    CompletionProvider, DEFAULT_GEMINI_COMPLETION_MODEL, DEFAULT_OPENAI_COMPLETION_MODEL,
    GeminiCompletion, OpenAiCompletion,
};

/// Completion, side-view, and dedicated judge settings captured together. An absent provider and
/// absent keys disable the optional lane; an explicit provider must be valid
/// and have its own nonempty key. Never include credentials in diagnostics.
pub struct CompletionConfig {
    provider: Option<String>,
    model: Option<String>,
    sideviews_provider: Option<String>,
    sideviews_model: Option<String>,
    judge_model: Option<String>,
    judge_partner_model: Option<String>,
    openai_key: Option<String>,
    openai_base: Option<String>,
    gemini_key: Option<String>,
    gemini_base: Option<String>,
}

impl CompletionConfig {
    pub fn from_env() -> Self {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    fn from_lookup(mut lookup: impl FnMut(&str) -> Option<String>) -> Self {
        Self {
            provider: lookup("COGNIGRAPH_COMPLETION_PROVIDER"),
            model: nonempty(lookup("COGNIGRAPH_COMPLETION_MODEL")),
            sideviews_provider: lookup("COGNIGRAPH_SIDEVIEWS_PROVIDER"),
            sideviews_model: nonempty(lookup("COGNIGRAPH_SIDEVIEWS_MODEL")),
            judge_model: nonempty(lookup("COGNIGRAPH_JUDGE_MODEL")),
            judge_partner_model: nonempty(lookup("COGNIGRAPH_JUDGE_PARTNER_MODEL")),
            openai_key: nonempty(lookup("OPENAI_API_KEY")),
            openai_base: nonempty(lookup("OPENAI_BASE_URL")),
            gemini_key: nonempty(lookup("GEMINI_API_KEY")),
            gemini_base: nonempty(lookup("GEMINI_BASE_URL")),
        }
    }

    fn completion_spec(&self) -> Result<Option<ProviderSpec>> {
        let provider = self.provider.as_deref().or_else(|| {
            if self.openai_key.is_some() {
                Some("openai")
            } else if self.gemini_key.is_some() {
                Some("gemini")
            } else {
                None
            }
        });
        provider
            .map(|name| self.named_spec(name, self.model.clone()))
            .transpose()
            .context("COGNIGRAPH_COMPLETION_PROVIDER")
    }

    fn sideviews_spec(&self) -> Result<Option<ProviderSpec>> {
        if let Some(provider) = &self.sideviews_provider {
            return self
                .named_spec(provider, self.sideviews_model.clone())
                .map(Some)
                .context("COGNIGRAPH_SIDEVIEWS_PROVIDER");
        }
        let mut spec = self.completion_spec()?;
        if let (Some(spec), Some(model)) = (&mut spec, &self.sideviews_model) {
            spec.model.clone_from(model);
        }
        Ok(spec)
    }

    /// Resolve the main construction lane. No keys and no explicit provider
    /// return `None`; configuration errors must not be treated as disabled.
    pub fn completion_provider(&self) -> Result<Option<Box<dyn CompletionProvider>>> {
        self.completion_spec()?.map(ProviderSpec::build).transpose()
    }

    /// Inherit completion settings unless a separate side-view provider is
    /// explicit. That provider uses its own default model when none is supplied.
    pub fn sideviews_provider(&self) -> Result<Option<Box<dyn CompletionProvider>>> {
        self.sideviews_spec()?.map(ProviderSpec::build).transpose()
    }

    /// Dedicated OpenAI judge, independent of the main provider/model. None
    /// leaves the review route's completion-provider fallback in effect.
    pub fn judge_provider(&self) -> Result<Option<Box<dyn CompletionProvider>>> {
        self.judge_spec(self.judge_model.as_ref(), "COGNIGRAPH_JUDGE_MODEL")?
            .map(ProviderSpec::build)
            .transpose()
    }

    /// Dedicated OpenAI agreement partner. None leaves Lane A+ unavailable;
    /// qualification and pair matching remain the review policy's concern.
    pub fn judge_partner_provider(&self) -> Result<Option<Box<dyn CompletionProvider>>> {
        self.judge_spec(
            self.judge_partner_model.as_ref(),
            "COGNIGRAPH_JUDGE_PARTNER_MODEL",
        )?
        .map(ProviderSpec::build)
        .transpose()
    }

    fn judge_spec(&self, model: Option<&String>, setting: &str) -> Result<Option<ProviderSpec>> {
        model
            .map(|model| self.named_spec("openai", Some(model.clone())))
            .transpose()
            .with_context(|| setting.to_owned())
    }

    fn named_spec(&self, provider: &str, model: Option<String>) -> Result<ProviderSpec> {
        let (kind, key, base, base_env, default_model) = match provider.trim() {
            "openai" => (
                ProviderKind::OpenAi,
                self.openai_key
                    .as_ref()
                    .context("OPENAI_API_KEY required for the openai completion provider")?,
                self.openai_base
                    .as_deref()
                    .unwrap_or("https://api.openai.com/v1"),
                "OPENAI_BASE_URL",
                DEFAULT_OPENAI_COMPLETION_MODEL,
            ),
            "gemini" => (
                ProviderKind::Gemini,
                self.gemini_key
                    .as_ref()
                    .context("GEMINI_API_KEY required for the gemini completion provider")?,
                self.gemini_base
                    .as_deref()
                    .unwrap_or("https://generativelanguage.googleapis.com/v1beta"),
                "GEMINI_BASE_URL",
                DEFAULT_GEMINI_COMPLETION_MODEL,
            ),
            other => bail!("unknown completion provider `{other}` (expected openai|gemini)"),
        };
        let url = reqwest::Url::parse(base)
            .map_err(|_| anyhow::anyhow!("{base_env} must be an absolute HTTP(S) base URL"))?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            bail!("{base_env} must be an absolute HTTP(S) base URL without a query or fragment");
        }
        Ok(ProviderSpec {
            kind,
            key: key.clone(),
            base: url.as_str().trim_end_matches('/').to_owned(),
            // Always pass a resolved model: OpenAiCompletion's legacy env
            // fallback must not leak into explicitly separate side-view lanes.
            model: nonempty(model).unwrap_or_else(|| default_model.into()),
        })
    }
}

fn nonempty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProviderKind {
    OpenAi,
    Gemini,
}

struct ProviderSpec {
    kind: ProviderKind,
    key: String,
    base: String,
    model: String,
}

impl ProviderSpec {
    fn build(self) -> Result<Box<dyn CompletionProvider>> {
        match self.kind {
            ProviderKind::OpenAi => Ok(Box::new(OpenAiCompletion::new(
                self.key,
                Some(self.base),
                Some(self.model),
            )?)),
            ProviderKind::Gemini => Ok(Box::new(GeminiCompletion::new(
                self.key,
                Some(self.base),
                Some(self.model),
            )?)),
        }
    }
}

/// Required main provider for measurement harnesses and examples, using the
/// same settings as the server (including provider-specific base URLs).
pub fn completion_from_env() -> Result<Box<dyn CompletionProvider>> {
    CompletionConfig::from_env()
        .completion_provider()?
        .context("completion provider requires OPENAI_API_KEY or GEMINI_API_KEY")
}

/// Build an explicit benchmark provider/model pair. An omitted model uses the
/// provider default, independently of COGNIGRAPH_COMPLETION_MODEL.
pub fn provider_named(
    provider: &str,
    model: Option<String>,
) -> Result<Box<dyn CompletionProvider>> {
    CompletionConfig::from_env()
        .named_spec(provider, model)?
        .build()
}

/// Required side-view provider for harnesses, sharing server inheritance rules.
pub fn sideviews_provider_from_env() -> Result<Box<dyn CompletionProvider>> {
    CompletionConfig::from_env()
        .sideviews_provider()?
        .context("side-view provider requires OPENAI_API_KEY or GEMINI_API_KEY")
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
