//! Exercise the actual automerge shell with GitHub API calls replaced by fixtures.

use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BASE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const REQUIRED: [&str; 4] = [
    "ci-gate",
    "workflow-security-gate",
    "security / codeql (rust)",
    "security / codeql (actions)",
];

const FAKE_GH: &str = r#"#!/usr/bin/env bash
set -u
root="$TEST_DIRECTORY"
command_name="$1"
shift
printf '%s' "$command_name" >> "$root/calls.tsv"
for argument in "$@"; do
  printf '\t%s' "$argument" >> "$root/calls.tsv"
done
printf '\n' >> "$root/calls.tsv"

if [ "$command_name" = pr ] && [ "${1:-}" = view ]; then
  jq -c '.info' "$root/case.json"
  exit 0
fi
if [ "$command_name" = pr ] && [ "${1:-}" = merge ]; then
  exit "$(jq -r '.merge_exit // 0' "$root/case.json")"
fi
if [ "$command_name" = run ] && [ "${1:-}" = list ]; then
  jq -c '.appsec' "$root/case.json"
  exit 0
fi
if [ "$command_name" = api ] && [ "${1:-}" = "repos/test/repo/git/ref/heads/main" ]; then
  jq -r '.base' "$root/case.json"
  exit 0
fi
if [ "$command_name" = api ] && [[ "${1:-}" == repos/test/repo/compare/* ]]; then
  if [ "$(jq -r '.compare_error // false' "$root/case.json")" = true ]; then
    exit 1
  fi
  jq -c '.comparison' "$root/case.json"
  exit 0
fi
echo "Unexpected GitHub call: $command_name $*" >&2
exit 99
"#;

fn ready_case() -> Value {
    json!({
        "info": {
            "author": {"login": "app/dependabot"},
            "isCrossRepository": false,
            "headRefOid": HEAD,
            "baseRefName": "main",
            "isDraft": false,
            "state": "OPEN",
            "mergeStateStatus": "CLEAN",
            "statusCheckRollup": REQUIRED.iter().map(|name| json!({
                "name": name,
                "status": "COMPLETED",
                "conclusion": "SUCCESS",
            })).collect::<Vec<_>>(),
        },
        "base": BASE,
        "appsec": [{"headSha": HEAD, "status": "completed", "conclusion": "success"}],
        "comparison": {"behind_by": 0, "merge_base_commit": {"sha": BASE}},
    })
}

fn run_workflow(case: &Value, associated_pr: Option<u64>) -> (Output, Vec<Vec<String>>) {
    let directory = TempDir::new().unwrap();
    let path = directory.path();
    fs::write(path.join("case.json"), case.to_string()).unwrap();
    let pull_requests = match associated_pr {
        Some(number) => json!([{"number": number}]),
        None => json!([]),
    };
    fs::write(
        path.join("event.json"),
        json!({"workflow_run": {"pull_requests": pull_requests}}).to_string(),
    )
    .unwrap();

    let gh_path = path.join("gh");
    fs::write(&gh_path, FAKE_GH).unwrap();
    fs::set_permissions(&gh_path, fs::Permissions::from_mode(0o700)).unwrap();

    let workflow = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/dependabot-automerge.yml"),
    )
    .unwrap();
    let script = workflow
        .split_once("        run: |")
        .expect("missing automerge run block")
        .1
        .trim_start_matches(['\r', '\n'])
        .replace("\r\n", "\n");

    let path_value = format!(
        "{}:{}",
        path.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = Command::new("bash")
        .arg("-c")
        .arg(script)
        .env("PATH", path_value)
        .env("TEST_DIRECTORY", path)
        .env("REPO", "test/repo")
        .env("CHECKED_SHA", HEAD)
        .env("GITHUB_EVENT_PATH", path.join("event.json"))
        .output()
        .unwrap();

    let calls_path = path.join("calls.tsv");
    let calls = if calls_path.exists() {
        fs::read_to_string(calls_path)
            .unwrap()
            .lines()
            .map(|line| line.split('\t').map(str::to_owned).collect())
            .collect()
    } else {
        Vec::new()
    };
    (output, calls)
}

fn assert_waits(case: &Value) -> (Output, Vec<Vec<String>>) {
    let (output, calls) = run_workflow(case, Some(24));
    assert!(
        output.status.success(),
        "workflow failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !calls
            .iter()
            .any(|call| call.first().is_some_and(|c| c == "pr")
                && call.get(1).is_some_and(|c| c == "merge")),
        "unexpected merge call: {calls:?}"
    );
    (output, calls)
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn current_base_and_successful_checks_merge_only_checked_head() {
    let (output, calls) = run_workflow(&ready_case(), Some(24));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        calls.last().unwrap(),
        &vec![
            "pr".to_owned(),
            "merge".to_owned(),
            "24".to_owned(),
            "--repo".to_owned(),
            "test/repo".to_owned(),
            "--squash".to_owned(),
            "--match-head-commit".to_owned(),
            HEAD.to_owned(),
        ]
    );
}

#[test]
fn stale_branch_waits_for_signed_dependabot_refresh() {
    let mut case = ready_case();
    case["comparison"]["behind_by"] = json!(1);
    let (output, _) = assert_waits(&case);
    assert!(stdout(&output).contains("Dependabot must refresh"));
}

#[test]
fn main_advanced_after_pr_lookup_does_not_merge() {
    let mut case = ready_case();
    case["base"] = json!("cccccccccccccccccccccccccccccccccccccccc");
    let (output, _) = assert_waits(&case);
    assert!(stdout(&output).contains("Dependabot must refresh"));
}

#[test]
fn unknown_or_blocked_native_merge_state_waits() {
    for state in ["BEHIND", "BLOCKED", "DIRTY", "DRAFT", "UNKNOWN", "UNSTABLE"] {
        let mut case = ready_case();
        case["info"]["mergeStateStatus"] = json!(state);
        assert_waits(&case);
    }
    let mut case = ready_case();
    case["info"]["mergeStateStatus"] = Value::Null;
    assert_waits(&case);
}

#[test]
fn head_moved_since_trigger_does_not_merge() {
    let mut case = ready_case();
    case["info"]["headRefOid"] = json!("dddddddddddddddddddddddddddddddddddddddd");
    let (_, calls) = assert_waits(&case);
    assert_eq!(calls.len(), 1);
}

#[test]
fn each_required_check_must_finish_successfully() {
    let outcomes: [(&str, Option<&str>); 8] = [
        ("IN_PROGRESS", None),
        ("QUEUED", None),
        ("COMPLETED", Some("FAILURE")),
        ("COMPLETED", Some("CANCELLED")),
        ("COMPLETED", Some("NEUTRAL")),
        ("COMPLETED", Some("SKIPPED")),
        ("COMPLETED", Some("TIMED_OUT")),
        ("COMPLETED", Some("ACTION_REQUIRED")),
    ];
    for name in REQUIRED {
        for (status, conclusion) in outcomes {
            let mut case = ready_case();
            let checks = case["info"]["statusCheckRollup"].as_array_mut().unwrap();
            let check = checks
                .iter_mut()
                .find(|check| check["name"] == name)
                .unwrap();
            check["status"] = json!(status);
            check["conclusion"] = match conclusion {
                Some(value) => json!(value),
                None => Value::Null,
            };
            assert_waits(&case);
        }
    }
}

#[test]
fn missing_analysis_or_gate_does_not_merge() {
    for name in REQUIRED {
        let mut case = ready_case();
        let checks = case["info"]["statusCheckRollup"].as_array_mut().unwrap();
        checks.retain(|check| check["name"] != name);
        assert_waits(&case);
    }
}

#[test]
fn old_successful_checks_do_not_hide_latest_appsec_failure() {
    for (status, conclusion) in [
        ("queued", None),
        ("in_progress", None),
        ("completed", Some("failure")),
        ("completed", Some("cancelled")),
        ("completed", Some("neutral")),
        ("completed", Some("skipped")),
    ] {
        let mut case = ready_case();
        case["appsec"][0]["status"] = json!(status);
        case["appsec"][0]["conclusion"] = match conclusion {
            Some(value) => json!(value),
            None => Value::Null,
        };
        assert_waits(&case);
    }
}

#[test]
fn appsec_from_a_different_commit_does_not_merge() {
    let mut case = ready_case();
    case["appsec"][0]["headSha"] = json!("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
    assert_waits(&case);
}

#[test]
fn missing_appsec_run_does_not_merge() {
    let mut case = ready_case();
    case["appsec"] = json!([]);
    assert_waits(&case);
}

#[test]
fn ineligible_pr_does_not_merge() {
    let mutations = [
        json!({"author": {"login": "contributor"}}),
        json!({"isCrossRepository": true}),
        json!({"baseRefName": "other"}),
        json!({"isDraft": true}),
        json!({"state": "CLOSED"}),
    ];
    for mutation in mutations {
        let mut case = ready_case();
        for (key, value) in mutation.as_object().unwrap() {
            case["info"][key] = value.clone();
        }
        assert_waits(&case);
    }
}

#[test]
fn no_associated_pr_does_not_call_github() {
    let (output, calls) = run_workflow(&ready_case(), None);
    assert!(output.status.success());
    assert!(calls.is_empty());
}

#[test]
fn compare_api_failure_is_not_treated_as_readiness() {
    let mut case = ready_case();
    case["compare_error"] = json!(true);
    let (output, calls) = run_workflow(&case, Some(24));
    assert!(!output.status.success());
    assert!(
        !calls
            .iter()
            .any(|call| call.first().is_some_and(|c| c == "pr")
                && call.get(1).is_some_and(|c| c == "merge"))
    );
}

#[test]
fn native_merge_rejection_is_not_hidden() {
    let mut case = ready_case();
    case["merge_exit"] = json!(1);
    let (output, _) = run_workflow(&case, Some(24));
    assert!(!output.status.success());
}

#[test]
fn appsec_completion_triggers_readiness_again() {
    let workflow = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/dependabot-automerge.yml"),
    )
    .unwrap();
    let trigger = workflow
        .split_once("workflows: [")
        .unwrap()
        .1
        .split_once(']')
        .unwrap()
        .0;
    let names: Vec<&str> = trigger.split(',').map(str::trim).collect();
    assert!(names.contains(&"AppSec"));
}
