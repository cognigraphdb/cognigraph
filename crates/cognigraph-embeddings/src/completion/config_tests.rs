use super::*;

fn config(values: &[(&str, &str)]) -> CompletionConfig {
    CompletionConfig::from_lookup(|name| {
        values
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).into())
    })
}

fn both_keys(extra: &[(&str, &str)]) -> CompletionConfig {
    let mut values = extra.to_vec();
    values.extend([
        ("OPENAI_API_KEY", "synthetic-openai"),
        ("GEMINI_API_KEY", "synthetic-gemini"),
    ]);
    config(&values)
}

#[test]
fn default_precedence_and_explicit_providers_with_both_keys() {
    for (explicit, expected) in [
        (None, ProviderKind::OpenAi),
        (Some("openai"), ProviderKind::OpenAi),
        (Some("gemini"), ProviderKind::Gemini),
    ] {
        let extra = explicit.map(|name| ("COGNIGRAPH_COMPLETION_PROVIDER", name));
        let settings = both_keys(&extra.into_iter().collect::<Vec<_>>());
        assert_eq!(settings.completion_spec().unwrap().unwrap().kind, expected);
        assert_eq!(settings.sideviews_spec().unwrap().unwrap().kind, expected);
    }
}

#[test]
fn each_key_alone_selects_its_provider() {
    for (key, expected) in [
        ("OPENAI_API_KEY", ProviderKind::OpenAi),
        ("GEMINI_API_KEY", ProviderKind::Gemini),
    ] {
        let settings = config(&[(key, "synthetic")]);
        assert_eq!(settings.completion_spec().unwrap().unwrap().kind, expected);
        assert_eq!(settings.sideviews_spec().unwrap().unwrap().kind, expected);
    }
}

#[test]
fn missing_or_blank_keys_leave_unconfigured_lanes_disabled() {
    let settings = config(&[("OPENAI_API_KEY", " \t"), ("GEMINI_API_KEY", "")]);
    assert!(settings.completion_provider().unwrap().is_none());
    assert!(settings.sideviews_provider().unwrap().is_none());
    let settings = config(&[("OPENAI_API_KEY", " "), ("GEMINI_API_KEY", "synthetic")]);
    assert_eq!(
        settings.completion_spec().unwrap().unwrap().kind,
        ProviderKind::Gemini
    );
}

#[test]
fn explicit_provider_requires_its_own_nonempty_key() {
    for (provider, missing_key, other_key) in [
        ("openai", "OPENAI_API_KEY", "GEMINI_API_KEY"),
        ("gemini", "GEMINI_API_KEY", "OPENAI_API_KEY"),
    ] {
        for lane in [
            "COGNIGRAPH_COMPLETION_PROVIDER",
            "COGNIGRAPH_SIDEVIEWS_PROVIDER",
        ] {
            let settings = config(&[
                (lane, provider),
                (missing_key, " "),
                (other_key, "synthetic"),
            ]);
            let error = if lane == "COGNIGRAPH_COMPLETION_PROVIDER" {
                settings.completion_provider().err().unwrap()
            } else {
                settings.sideviews_provider().err().unwrap()
            };
            let message = format!("{error:#}");
            assert!(
                message.contains(lane) && message.contains(missing_key),
                "{message}"
            );
        }
    }
}

#[test]
fn invalid_explicit_providers_are_errors_even_without_keys() {
    for name in ["", " ", "OpenAI", "unknown"] {
        let settings = config(&[("COGNIGRAPH_COMPLETION_PROVIDER", name)]);
        assert!(settings.completion_provider().is_err());
        assert!(settings.sideviews_provider().is_err());
        let settings = both_keys(&[("COGNIGRAPH_SIDEVIEWS_PROVIDER", name)]);
        assert!(settings.sideviews_provider().is_err());
    }
}

#[test]
fn inherited_sideviews_use_completion_model_and_base_url() {
    for (provider, base_env) in [("openai", "OPENAI_BASE_URL"), ("gemini", "GEMINI_BASE_URL")] {
        let settings = both_keys(&[
            ("COGNIGRAPH_COMPLETION_PROVIDER", provider),
            ("COGNIGRAPH_COMPLETION_MODEL", " custom-model "),
            (base_env, " http://127.0.0.1:12345/custom/// "),
        ]);
        let completion = settings.completion_spec().unwrap().unwrap();
        let sideviews = settings.sideviews_spec().unwrap().unwrap();
        assert_eq!(completion.model, "custom-model");
        assert_eq!(sideviews.model, completion.model);
        assert_eq!(completion.base, "http://127.0.0.1:12345/custom");
        assert_eq!(sideviews.base, completion.base);
    }
}

#[test]
fn sideview_model_override_keeps_inherited_provider() {
    let settings = both_keys(&[
        ("COGNIGRAPH_COMPLETION_PROVIDER", "gemini"),
        ("COGNIGRAPH_COMPLETION_MODEL", "main-model"),
        ("COGNIGRAPH_SIDEVIEWS_MODEL", " side-model "),
    ]);
    assert_eq!(
        settings.completion_spec().unwrap().unwrap().model,
        "main-model"
    );
    let sideviews = settings.sideviews_spec().unwrap().unwrap();
    assert_eq!(sideviews.kind, ProviderKind::Gemini);
    assert_eq!(sideviews.model, "side-model");
}

#[test]
fn explicit_sideview_provider_uses_own_default_model_and_base_url() {
    for (provider, expected_model, expected_base) in [
        (
            "openai",
            DEFAULT_OPENAI_COMPLETION_MODEL,
            "http://127.0.0.1:12345/openai",
        ),
        (
            "gemini",
            "gemini-3.8-flash",
            "http://127.0.0.1:12345/gemini",
        ),
    ] {
        let settings = both_keys(&[
            ("COGNIGRAPH_COMPLETION_MODEL", "unrelated-model"),
            ("COGNIGRAPH_SIDEVIEWS_PROVIDER", provider),
            ("OPENAI_BASE_URL", "http://127.0.0.1:12345/openai"),
            ("GEMINI_BASE_URL", "http://127.0.0.1:12345/gemini"),
        ]);
        let spec = settings.sideviews_spec().unwrap().unwrap();
        assert_eq!(spec.base, expected_base);
        assert_eq!(spec.model, expected_model);
        assert_eq!(
            settings.sideviews_provider().unwrap().unwrap().model_name(),
            expected_model
        );
    }
}

#[test]
fn explicit_sideview_provider_and_model_are_independent() {
    let settings = both_keys(&[
        ("COGNIGRAPH_COMPLETION_PROVIDER", "gemini"),
        ("COGNIGRAPH_COMPLETION_MODEL", "main-model"),
        ("COGNIGRAPH_SIDEVIEWS_PROVIDER", "openai"),
        ("COGNIGRAPH_SIDEVIEWS_MODEL", "side-model"),
    ]);
    assert_eq!(
        settings.completion_spec().unwrap().unwrap().kind,
        ProviderKind::Gemini
    );
    let spec = settings.sideviews_spec().unwrap().unwrap();
    assert_eq!(spec.kind, ProviderKind::OpenAi);
    assert_eq!(spec.model, "side-model");
}

#[test]
fn named_provider_default_does_not_inherit_completion_model() {
    let settings = both_keys(&[("COGNIGRAPH_COMPLETION_MODEL", "unrelated-model")]);
    for (name, model) in [
        ("openai", DEFAULT_OPENAI_COMPLETION_MODEL),
        ("gemini", "gemini-3.8-flash"),
    ] {
        assert_eq!(
            settings
                .named_spec(name, None)
                .unwrap()
                .build()
                .unwrap()
                .model_name(),
            model
        );
        assert_eq!(
            settings
                .named_spec(name, Some(" benchmark-model ".into()))
                .unwrap()
                .build()
                .unwrap()
                .model_name(),
            "benchmark-model"
        );
    }
}

#[test]
fn blank_models_and_base_urls_use_provider_defaults() {
    let settings = both_keys(&[
        ("COGNIGRAPH_COMPLETION_MODEL", " "),
        ("OPENAI_BASE_URL", ""),
        ("GEMINI_BASE_URL", " "),
    ]);
    let spec = settings.completion_spec().unwrap().unwrap();
    assert_eq!(spec.model, DEFAULT_OPENAI_COMPLETION_MODEL);
    assert_eq!(spec.base, "https://api.openai.com/v1");
    let gemini = settings.named_spec("gemini", Some(" ".into())).unwrap();
    assert_eq!(gemini.model, "gemini-3.8-flash");
    assert_eq!(
        gemini.base,
        "https://generativelanguage.googleapis.com/v1beta"
    );
}

#[test]
fn invalid_selected_base_urls_fail_without_exposing_their_value() {
    for base in [
        "invalid-secret",
        "file:///private/secret",
        "https://example.test/v1?key=secret",
        "https://example.test/v1#secret",
    ] {
        let settings = both_keys(&[("OPENAI_BASE_URL", base)]);
        let error = format!("{:#}", settings.completion_provider().err().unwrap());
        assert!(error.contains("OPENAI_BASE_URL"));
        assert!(!error.contains("secret"));
    }
    // An unused provider's URL does not break a correctly selected lane.
    assert!(
        both_keys(&[("GEMINI_BASE_URL", "invalid")])
            .completion_provider()
            .is_ok()
    );
}

#[test]
fn dedicated_judges_share_openai_resolution_without_main_lane_inheritance() {
    let settings = both_keys(&[
        ("COGNIGRAPH_COMPLETION_PROVIDER", "gemini"),
        ("COGNIGRAPH_COMPLETION_MODEL", "construction"),
        ("COGNIGRAPH_JUDGE_MODEL", " primary "),
        ("COGNIGRAPH_JUDGE_PARTNER_MODEL", " partner "),
        ("OPENAI_BASE_URL", " http://127.0.0.1:12345/review/// "),
    ]);
    assert_eq!(
        settings.completion_spec().unwrap().unwrap().kind,
        ProviderKind::Gemini
    );
    for (model, expected) in [
        (&settings.judge_model, "primary"),
        (&settings.judge_partner_model, "partner"),
    ] {
        let spec = settings
            .judge_spec(model.as_ref(), "judge")
            .unwrap()
            .unwrap();
        assert_eq!(spec.kind, ProviderKind::OpenAi);
        assert_eq!(spec.key, "synthetic-openai");
        assert_eq!(spec.base, "http://127.0.0.1:12345/review");
        assert_eq!(spec.model, expected);
    }
    assert_eq!(
        settings.judge_provider().unwrap().unwrap().model_name(),
        "primary"
    );
    assert_eq!(
        settings
            .judge_partner_provider()
            .unwrap()
            .unwrap()
            .model_name(),
        "partner"
    );
}

#[test]
fn absent_judges_remain_optional_and_dedicated_judges_use_endpoint_defaults() {
    for models in [
        vec![],
        vec![
            ("COGNIGRAPH_JUDGE_MODEL", " \t"),
            ("COGNIGRAPH_JUDGE_PARTNER_MODEL", ""),
        ],
    ] {
        let settings = config(&models);
        assert!(settings.judge_provider().unwrap().is_none());
        assert!(settings.judge_partner_provider().unwrap().is_none());
    }
    for base in [vec![], vec![("OPENAI_BASE_URL", " \t")]] {
        let settings = both_keys(&base);
        let model = "dedicated".to_owned();
        assert_eq!(
            settings
                .judge_spec(Some(&model), "judge")
                .unwrap()
                .unwrap()
                .base,
            "https://api.openai.com/v1"
        );
    }
}

#[test]
fn explicit_judges_require_their_key_and_validate_only_their_selected_endpoint() {
    for lane in ["COGNIGRAPH_JUDGE_MODEL", "COGNIGRAPH_JUDGE_PARTNER_MODEL"] {
        for key in [None, Some(" \t")] {
            let mut values = vec![(lane, "dedicated"), ("GEMINI_API_KEY", "synthetic-gemini")];
            if let Some(key) = key {
                values.push(("OPENAI_API_KEY", key));
            }
            let settings = config(&values);
            let result = if lane == "COGNIGRAPH_JUDGE_MODEL" {
                settings.judge_provider()
            } else {
                settings.judge_partner_provider()
            };
            let error = format!("{:#}", result.err().unwrap());
            assert!(
                error.contains(lane) && error.contains("OPENAI_API_KEY"),
                "{error}"
            );
            assert!(!error.contains("synthetic"));
        }
        for base in [
            "invalid-secret",
            "file:///secret",
            "https://example.test/v1?secret=key",
            "https://example.test/v1#secret",
        ] {
            let settings = both_keys(&[(lane, "dedicated"), ("OPENAI_BASE_URL", base)]);
            let result = if lane == "COGNIGRAPH_JUDGE_MODEL" {
                settings.judge_provider()
            } else {
                settings.judge_partner_provider()
            };
            let error = format!("{:#}", result.err().unwrap());
            assert!(
                error.contains(lane) && error.contains("OPENAI_BASE_URL"),
                "{error}"
            );
            assert!(!error.contains("secret"), "{error}");
        }
    }
    let settings = both_keys(&[
        ("COGNIGRAPH_JUDGE_MODEL", "primary"),
        ("COGNIGRAPH_JUDGE_PARTNER_MODEL", "partner"),
        ("GEMINI_BASE_URL", "invalid"),
    ]);
    assert!(settings.judge_provider().is_ok());
    assert!(settings.judge_partner_provider().is_ok());
}
