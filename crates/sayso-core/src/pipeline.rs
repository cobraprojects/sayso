//! The text part of the pipeline: replacements, then the style (plan §3).
//!
//! The rule is that text is never lost. Any AI failure returns the text after
//! replacements, with the reason.

use crate::dictionary::{AppliedReplacement, Replacer};
use crate::enhance::{EnhanceError, EnhanceRequest, Enhancer};
use crate::history::{EnhanceAttempt, EnhanceOutcome, EnhanceTimings};
use crate::style::{Style, system_prompt};
use std::time::{Duration, Instant};

pub struct TextPipeline<'a> {
    pub replacer: &'a Replacer,
    pub style: &'a Style,
    /// The base prompt of the style library.
    pub base_prompt: &'a str,
    pub vocabulary: &'a [String],
    /// None when AI is off or no provider is set up.
    pub enhancer: Option<&'a dyn Enhancer>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PipelineOutput {
    pub text: String,
    pub replacements: Vec<AppliedReplacement>,
    pub enhance: EnhanceOutcome,
    /// Set when the style failed: the id of the provider and the error. The
    /// overlay shows it.
    pub enhance_error: Option<(String, EnhanceError)>,
    /// The attempts, when a provider ran. The caller knows the time of the
    /// whole step, the setup time, and the kind of the provider.
    pub timings: Option<EnhanceTimings>,
}

impl TextPipeline<'_> {
    /// True when [`run_style`](Self::run_style) will call a provider.
    pub fn will_enhance(&self) -> bool {
        self.style.uses_ai() && self.enhancer.is_some()
    }

    pub fn replace(&self, transcript: &str) -> (String, Vec<AppliedReplacement>) {
        self.replacer.apply(transcript.trim())
    }

    /// Run the style on text that already has replacements applied.
    pub fn run_style(&self, replaced: String, replacements: Vec<AppliedReplacement>) -> PipelineOutput {
        // A model answers an empty transcript with a remark about it.
        let Some(enhancer) = self.enhancer.filter(|_| self.style.uses_ai() && !replaced.trim().is_empty()) else {
            return PipelineOutput { text: replaced, replacements, enhance: EnhanceOutcome::NotUsed, enhance_error: None, timings: None };
        };
        // The model gets spelled words as words. A failure returns the text without this step.
        let spoken = crate::spelled::apply(&replaced);
        let request = EnhanceRequest {
            system_prompt: system_prompt(self.style, self.base_prompt, self.vocabulary, &spoken),
            transcript: spoken,
            model: self.style.model.clone(),
            temperature: self.style.temperature,
            timeout: self.style.timeout_ms.map(Duration::from_millis).unwrap_or(self.timeout),
        };
        let mut attempts = Vec::new();
        let mut attempt = || {
            let started = Instant::now();
            let result = enhancer.enhance(&request).and_then(|response| {
                if is_runaway(&replaced, &response.text) {
                    return Err(EnhanceError::InvalidOutput("the reply is much longer than the text".into()));
                }
                Ok(response)
            });
            attempts.push(EnhanceAttempt {
                elapsed_ms: started.elapsed().as_millis() as u64,
                error: result.as_ref().err().map(|e| e.to_string()),
                mode: result.as_ref().ok().map(|r| r.mode.clone()),
                report: result.as_ref().ok().and_then(|r| r.report.clone()),
            });
            result
        };
        // One retry when the reply has the wrong shape. Timeouts are not retried.
        let mut result = attempt();
        if matches!(result, Err(EnhanceError::InvalidOutput(_))) {
            result = attempt();
        }
        // The time the user waited: a retry counts too.
        let elapsed_ms = attempts.iter().map(|a| a.elapsed_ms).sum();
        let timings = Some(EnhanceTimings {
            timeout_ms: request.timeout.as_millis() as u64,
            prompt_chars: request.system_prompt.chars().count() + request.transcript.chars().count(),
            attempts,
            ..Default::default()
        });
        match result {
            Ok(resp) => PipelineOutput {
                text: resp.text.trim().to_string(),
                replacements,
                enhance: EnhanceOutcome::Applied {
                    provider_id: resp.provider_id,
                    model: resp.model,
                    elapsed_ms,
                },
                enhance_error: None,
                timings,
            },
            Err(err) => {
                log::warn!("style {} failed with {}: {err}", self.style.id, enhancer.id());
                PipelineOutput {
                    text: replaced,
                    replacements,
                    enhance: EnhanceOutcome::Failed { provider_id: enhancer.id().to_string(), reason: err.to_string() },
                    enhance_error: Some((enhancer.id().to_string(), err)),
                    timings,
                }
            }
        }
    }

    pub fn run(&self, transcript: &str) -> PipelineOutput {
        let (replaced, applied) = self.replace(transcript);
        self.run_style(replaced, applied)
    }
}

/// True when `reply` is too long to be an edit of `text`: the model answered
/// the text or did not stop. A greeting and a sign-off fit in the limit.
fn is_runaway(text: &str, reply: &str) -> bool {
    reply.chars().count() > text.chars().count() * 3 + 200
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::Replacement;
    use crate::enhance::EnhanceResponse;
    use crate::style::builtin_styles;
    use std::sync::Mutex;

    struct Fake {
        replies: Mutex<Vec<Result<EnhanceResponse, EnhanceError>>>,
        seen: Mutex<Vec<EnhanceRequest>>,
    }

    impl Fake {
        fn new(replies: Vec<Result<EnhanceResponse, EnhanceError>>) -> Self {
            Self { replies: Mutex::new(replies), seen: Mutex::new(vec![]) }
        }
    }

    impl Enhancer for Fake {
        fn id(&self) -> &str {
            "fake"
        }
        fn is_cloud(&self) -> bool {
            false
        }
        fn enhance(&self, request: &EnhanceRequest) -> Result<EnhanceResponse, EnhanceError> {
            self.seen.lock().unwrap().push(request.clone());
            self.replies.lock().unwrap().remove(0)
        }
    }

    fn ok(text: &str) -> Result<EnhanceResponse, EnhanceError> {
        Ok(EnhanceResponse { text: text.into(), provider_id: "fake".into(), model: "m".into(), elapsed_ms: 5, mode: "json_schema".into(), report: None })
    }

    fn style(id: &str) -> Style {
        builtin_styles().into_iter().find(|s| s.id == id).unwrap()
    }

    fn rules() -> Vec<Replacement> {
        vec![Replacement { id: 1, from: "git hub".into(), to: "GitHub".into(), case_sensitive: false, uses: 0 }]
    }

    #[test]
    fn replacements_run_before_the_style() {
        let fake = Fake::new(vec![ok("Open a GitHub issue.")]);
        let replacer = Replacer::new(&rules());
        let clean = style("clean");
        let p = TextPipeline { replacer: &replacer, style: &clean, base_prompt: "", vocabulary: &[], enhancer: Some(&fake), timeout: Duration::from_secs(4) };
        let out = p.run("open a git hub issue");
        assert_eq!(out.text, "Open a GitHub issue.");
        assert_eq!(fake.seen.lock().unwrap()[0].transcript, "open a GitHub issue");
        assert_eq!(out.replacements.len(), 1);
    }

    #[test]
    fn raw_style_never_calls_the_provider() {
        let fake = Fake::new(vec![]);
        let replacer = Replacer::new(&[]);
        let raw = style("raw");
        let p = TextPipeline { replacer: &replacer, style: &raw, base_prompt: "", vocabulary: &[], enhancer: Some(&fake), timeout: Duration::from_secs(4) };
        assert!(!p.will_enhance());
        assert_eq!(p.run("hi there").enhance, EnhanceOutcome::NotUsed);
    }

    #[test]
    fn failure_keeps_the_replaced_text() {
        let fake = Fake::new(vec![Err(EnhanceError::Timeout(4000))]);
        let replacer = Replacer::new(&rules());
        let clean = style("clean");
        let p = TextPipeline { replacer: &replacer, style: &clean, base_prompt: "", vocabulary: &[], enhancer: Some(&fake), timeout: Duration::from_secs(4) };
        let out = p.run("on git hub");
        assert_eq!(out.text, "on GitHub");
        assert_eq!(out.enhance_error, Some(("fake".to_string(), EnhanceError::Timeout(4000))));
        assert!(matches!(out.enhance, EnhanceOutcome::Failed { .. }));
    }

    #[test]
    fn invalid_output_is_retried_once() {
        let fake = Fake::new(vec![Err(EnhanceError::InvalidOutput("x".into())), ok("Fixed.")]);
        let replacer = Replacer::new(&[]);
        let clean = style("clean");
        let p = TextPipeline { replacer: &replacer, style: &clean, base_prompt: "", vocabulary: &[], enhancer: Some(&fake), timeout: Duration::from_secs(4) };
        let out = p.run("fixed");
        assert_eq!(out.text, "Fixed.");
        assert_eq!(fake.seen.lock().unwrap().len(), 2);
        // The diagnostics keep both attempts, with the reason for the retry.
        let timings = out.timings.unwrap();
        assert_eq!(timings.timeout_ms, 4000);
        assert_eq!(timings.attempts.len(), 2);
        assert!(timings.attempts[0].error.as_deref().is_some_and(|e| e.contains("schema")));
        assert_eq!((timings.attempts[1].error.as_deref(), timings.attempts[1].mode.as_deref()), (None, Some("json_schema")));
    }

    #[test]
    fn style_overrides_reach_the_request() {
        let fake = Fake::new(vec![ok("x")]);
        let replacer = Replacer::new(&[]);
        let mut s = style("email");
        s.model = Some("big".into());
        s.timeout_ms = Some(9000);
        let p = TextPipeline { replacer: &replacer, style: &s, base_prompt: "- Shared rule.", vocabulary: &["Dana".into()], enhancer: Some(&fake), timeout: Duration::from_secs(4) };
        p.run("hi dana");
        let req = &fake.seen.lock().unwrap()[0];
        assert_eq!(req.model.as_deref(), Some("big"));
        assert_eq!(req.timeout, Duration::from_millis(9000));
        assert!(req.system_prompt.contains("Dana"));
        assert!(req.system_prompt.contains("- Shared rule."));
    }

    #[test]
    fn the_model_gets_spelled_words_as_words() {
        let fake = Fake::new(vec![ok("Send it to Dana Katz."), Err(EnhanceError::Timeout(4000))]);
        let replacer = Replacer::new(&[]);
        let clean = style("clean");
        let p = TextPipeline { replacer: &replacer, style: &clean, base_prompt: "", vocabulary: &[], enhancer: Some(&fake), timeout: Duration::from_secs(4) };
        assert_eq!(p.run("send it to dana cats that's k a t z").text, "Send it to Dana Katz.");
        assert_eq!(fake.seen.lock().unwrap()[0].transcript, "send it to dana katz");
        // A failure returns what the speaker said.
        assert_eq!(p.run("send it to dana cats that's k a t z").text, "send it to dana cats that's k a t z");
    }

    #[test]
    fn an_empty_text_never_calls_the_provider() {
        let fake = Fake::new(vec![]);
        let replacer = Replacer::new(&[]);
        let clean = style("clean");
        let p = TextPipeline { replacer: &replacer, style: &clean, base_prompt: "", vocabulary: &[], enhancer: Some(&fake), timeout: Duration::from_secs(4) };
        assert_eq!(p.run("  ").enhance, EnhanceOutcome::NotUsed);
    }

    #[test]
    fn a_reply_much_longer_than_the_text_keeps_the_replaced_text() {
        let answer = "Tokyo is nine hours ahead of UTC. ".repeat(10);
        let fake = Fake::new(vec![ok(&answer), ok(&answer)]);
        let replacer = Replacer::new(&[]);
        let clean = style("clean");
        let p = TextPipeline { replacer: &replacer, style: &clean, base_prompt: "", vocabulary: &[], enhancer: Some(&fake), timeout: Duration::from_secs(4) };
        let out = p.run("what time is it in tokyo");
        assert_eq!(out.text, "what time is it in tokyo");
        assert!(matches!(out.enhance, EnhanceOutcome::Failed { .. }));
        assert_eq!(fake.seen.lock().unwrap().len(), 2, "one retry");
        // An email adds a greeting and a sign-off to a short text.
        assert!(!is_runaway("send dana the report", "Hi Dana,\n\nPlease send me the report.\n\nBest regards,"));
    }
}
