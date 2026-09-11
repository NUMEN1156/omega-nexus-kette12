use serde::Deserialize;

pub const BROWSER_REQUEST_TOPIC: &str = "skill.browser.request";
pub const BROWSER_RESULT_TOPIC: &str = "skill.browser.result";
pub const SCIENTIFIC_REQUEST_TOPIC: &str = "skill.scientific.request";
pub const SCIENTIFIC_RESULT_TOPIC: &str = "skill.scientific.result";
const MAX_REQUEST_TIMEOUT_SECS: u64 = 300;
const MAX_BROWSER_STEPS: u16 = 64;
const MAX_SCIENTIFIC_DOCUMENTS: u16 = 25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillKind {
    BrowserUse,
    Scientific,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillResultKind {
    Success,
    Failure,
    Timeout,
    Denied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillEvent {
    Request {
        kind: SkillKind,
    },
    Result {
        kind: SkillKind,
        outcome: SkillResultKind,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ScoutKind {
    Outreach,
    Label,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SkillStatus {
    Success,
    Error,
    Timeout,
    Denied,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowserUseRequest {
    scout: ScoutKind,
    task_id: String,
    target: String,
    allowed_targets: Vec<String>,
    output_topic: String,
    timeout_secs: Option<u64>,
    max_steps: Option<u16>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScientificRequest {
    scout: ScoutKind,
    task_id: String,
    source: String,
    query: String,
    allowed_sources: Vec<String>,
    output_topic: String,
    timeout_secs: Option<u64>,
    max_documents: Option<u16>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SkillResult {
    scout: ScoutKind,
    task_id: String,
    status: SkillStatus,
    summary: String,
    #[serde(default)]
    artifacts: Vec<SkillArtifact>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SkillArtifact {
    kind: String,
    uri: String,
}

pub fn validate_payload(topic: &str, data: &[u8]) -> Result<Option<SkillEvent>, String> {
    match topic {
        BROWSER_REQUEST_TOPIC => {
            let request: BrowserUseRequest = parse_json(data, "browser request")?;
            validate_browser_request(&request)?;
            Ok(Some(SkillEvent::Request {
                kind: SkillKind::BrowserUse,
            }))
        }
        SCIENTIFIC_REQUEST_TOPIC => {
            let request: ScientificRequest = parse_json(data, "scientific request")?;
            validate_scientific_request(&request)?;
            Ok(Some(SkillEvent::Request {
                kind: SkillKind::Scientific,
            }))
        }
        BROWSER_RESULT_TOPIC => {
            let result: SkillResult = parse_json(data, "browser result")?;
            validate_result(&result, SkillKind::BrowserUse)?;
            Ok(Some(SkillEvent::Result {
                kind: SkillKind::BrowserUse,
                outcome: result_kind(result.status),
            }))
        }
        SCIENTIFIC_RESULT_TOPIC => {
            let result: SkillResult = parse_json(data, "scientific result")?;
            validate_result(&result, SkillKind::Scientific)?;
            Ok(Some(SkillEvent::Result {
                kind: SkillKind::Scientific,
                outcome: result_kind(result.status),
            }))
        }
        _ => Ok(None),
    }
}

pub fn is_skill_topic(topic: &str) -> bool {
    matches!(
        topic,
        BROWSER_REQUEST_TOPIC
            | BROWSER_RESULT_TOPIC
            | SCIENTIFIC_REQUEST_TOPIC
            | SCIENTIFIC_RESULT_TOPIC
    )
}

fn parse_json<T: for<'de> Deserialize<'de>>(data: &[u8], label: &str) -> Result<T, String> {
    serde_json::from_slice(data).map_err(|error| format!("invalid {label} payload: {error}"))
}

fn validate_browser_request(request: &BrowserUseRequest) -> Result<(), String> {
    if !matches!(request.scout, ScoutKind::Outreach) {
        return Err("browser use is restricted to the outreach scout".into());
    }
    if request.task_id.trim().is_empty() {
        return Err("browser request task_id must not be empty".into());
    }
    if request.target.trim().is_empty() {
        return Err("browser request target must not be empty".into());
    }
    if request.output_topic != BROWSER_RESULT_TOPIC {
        return Err(format!(
            "browser request output_topic must be `{BROWSER_RESULT_TOPIC}`"
        ));
    }
    if request.allowed_targets.is_empty() {
        return Err("browser request requires an explicit allowed_targets policy".into());
    }
    if !request
        .allowed_targets
        .iter()
        .any(|allowed| pattern_matches(allowed, &request.target))
    {
        return Err("browser request target is outside allowed_targets".into());
    }
    if let Some(timeout_secs) = request.timeout_secs {
        validate_timeout(timeout_secs, "browser request")?;
    }
    if let Some(max_steps) = request.max_steps {
        if max_steps == 0 || max_steps > MAX_BROWSER_STEPS {
            return Err(format!(
                "browser request max_steps must be between 1 and {MAX_BROWSER_STEPS}"
            ));
        }
    }
    Ok(())
}

fn validate_scientific_request(request: &ScientificRequest) -> Result<(), String> {
    if !matches!(request.scout, ScoutKind::Label) {
        return Err("scientific skill is restricted to the label scout".into());
    }
    if request.task_id.trim().is_empty() {
        return Err("scientific request task_id must not be empty".into());
    }
    if request.source.trim().is_empty() {
        return Err("scientific request source must not be empty".into());
    }
    if request.query.trim().is_empty() {
        return Err("scientific request query must not be empty".into());
    }
    if request.output_topic != SCIENTIFIC_RESULT_TOPIC {
        return Err(format!(
            "scientific request output_topic must be `{SCIENTIFIC_RESULT_TOPIC}`"
        ));
    }
    if request.allowed_sources.is_empty() {
        return Err("scientific request requires an explicit allowed_sources policy".into());
    }
    if !request
        .allowed_sources
        .iter()
        .any(|allowed| pattern_matches(allowed, &request.source))
    {
        return Err("scientific request source is outside allowed_sources".into());
    }
    if let Some(timeout_secs) = request.timeout_secs {
        validate_timeout(timeout_secs, "scientific request")?;
    }
    if let Some(max_documents) = request.max_documents {
        if max_documents == 0 || max_documents > MAX_SCIENTIFIC_DOCUMENTS {
            return Err(format!(
                "scientific request max_documents must be between 1 and {MAX_SCIENTIFIC_DOCUMENTS}"
            ));
        }
    }
    Ok(())
}

fn validate_result(result: &SkillResult, kind: SkillKind) -> Result<(), String> {
    if result.task_id.trim().is_empty() {
        return Err("skill result task_id must not be empty".into());
    }
    if result.summary.trim().is_empty() {
        return Err("skill result summary must not be empty".into());
    }
    match kind {
        SkillKind::BrowserUse if !matches!(result.scout, ScoutKind::Outreach) => {
            return Err("browser result scout must be outreach".into());
        }
        SkillKind::Scientific if !matches!(result.scout, ScoutKind::Label) => {
            return Err("scientific result scout must be label".into());
        }
        _ => {}
    }
    match result.status {
        SkillStatus::Success => {
            if result.artifacts.is_empty() {
                return Err("successful skill result must include at least one artifact".into());
            }
            if result.error.is_some() {
                return Err("successful skill result must not include an error".into());
            }
        }
        SkillStatus::Error | SkillStatus::Timeout | SkillStatus::Denied => {
            if result
                .error
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
            {
                return Err("non-success skill result must include an error".into());
            }
        }
    }
    for artifact in &result.artifacts {
        if artifact.kind.trim().is_empty() || artifact.uri.trim().is_empty() {
            return Err("skill result artifacts require non-empty kind and uri".into());
        }
    }
    Ok(())
}

fn validate_timeout(timeout_secs: u64, label: &str) -> Result<(), String> {
    if timeout_secs == 0 || timeout_secs > MAX_REQUEST_TIMEOUT_SECS {
        return Err(format!(
            "{label} timeout_secs must be between 1 and {MAX_REQUEST_TIMEOUT_SECS}"
        ));
    }
    Ok(())
}

fn pattern_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return false;
    }
    pattern == "*"
        || pattern == value
        || pattern
            .strip_suffix('*')
            .map(|prefix| value.starts_with(prefix))
            .unwrap_or(false)
}

fn result_kind(status: SkillStatus) -> SkillResultKind {
    match status {
        SkillStatus::Success => SkillResultKind::Success,
        SkillStatus::Error => SkillResultKind::Failure,
        SkillStatus::Timeout => SkillResultKind::Timeout,
        SkillStatus::Denied => SkillResultKind::Denied,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn browser_request_requires_explicit_allowlist() {
        let payload = json!({
            "scout": "outreach",
            "task_id": "job-1",
            "target": "https://example.org/research",
            "allowed_targets": [],
            "output_topic": BROWSER_RESULT_TOPIC
        });
        let error = validate_payload(
            BROWSER_REQUEST_TOPIC,
            &serde_json::to_vec(&payload).unwrap(),
        )
        .unwrap_err();
        assert!(error.contains("allowed_targets"));
    }

    #[test]
    fn browser_request_accepts_matching_target() {
        let payload = json!({
            "scout": "outreach",
            "task_id": "job-1",
            "target": "https://example.org/research",
            "allowed_targets": ["https://example.org/*"],
            "output_topic": BROWSER_RESULT_TOPIC,
            "timeout_secs": 30,
            "max_steps": 4
        });
        assert_eq!(
            validate_payload(
                BROWSER_REQUEST_TOPIC,
                &serde_json::to_vec(&payload).unwrap()
            )
            .unwrap(),
            Some(SkillEvent::Request {
                kind: SkillKind::BrowserUse
            })
        );
    }

    #[test]
    fn scientific_request_is_restricted_to_label_scout() {
        let payload = json!({
            "scout": "outreach",
            "task_id": "job-2",
            "source": "arxiv",
            "query": "vector memory",
            "allowed_sources": ["arxiv"],
            "output_topic": SCIENTIFIC_RESULT_TOPIC
        });
        let error = validate_payload(
            SCIENTIFIC_REQUEST_TOPIC,
            &serde_json::to_vec(&payload).unwrap(),
        )
        .unwrap_err();
        assert!(error.contains("label scout"));
    }

    #[test]
    fn result_requires_error_for_fail_closed_states() {
        let payload = json!({
            "scout": "label",
            "task_id": "job-2",
            "status": "timeout",
            "summary": "timed out waiting for source"
        });
        let error = validate_payload(
            SCIENTIFIC_RESULT_TOPIC,
            &serde_json::to_vec(&payload).unwrap(),
        )
        .unwrap_err();
        assert!(error.contains("must include an error"));
    }

    #[test]
    fn successful_result_requires_artifact() {
        let payload = json!({
            "scout": "outreach",
            "task_id": "job-3",
            "status": "success",
            "summary": "captured page"
        });
        let error = validate_payload(BROWSER_RESULT_TOPIC, &serde_json::to_vec(&payload).unwrap())
            .unwrap_err();
        assert!(error.contains("artifact"));
    }
}
