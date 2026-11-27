//! Read-only Dependabot provenance check; successful CI decides update eligibility.

use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::{env, fs, process::Command};

/// A batch qualifies only when every commit is a verified Dependabot commit and
/// the final commit still matches the checked event head.
pub fn verified_bot_commits(commits: &[Value], head: &str) -> bool {
    if commits.is_empty() || commits.last().and_then(|c| c["sha"].as_str()) != Some(head) {
        return false;
    }
    commits.iter().all(|commit| {
        commit["author"]["login"].as_str() == Some("dependabot[bot]")
            && commit["commit"]["verification"]["verified"].as_bool() == Some(true)
    })
}

fn main() -> Result<()> {
    let event_path = env::var("GITHUB_EVENT_PATH").context("GITHUB_EVENT_PATH is not set")?;
    let event: Value = serde_json::from_slice(&fs::read(event_path)?)?;
    let pr = &event["pull_request"];
    let number = pr["number"]
        .as_u64()
        .context("event has no pull request number")?;
    let repository = event["repository"]["full_name"]
        .as_str()
        .context("event has no repository name")?;
    let head = pr["head"]["sha"]
        .as_str()
        .context("event has no pull request head SHA")?;

    let endpoint = format!("repos/{repository}/pulls/{number}/commits?per_page=100");
    let output = Command::new("gh")
        .args(["api", &endpoint, "--paginate", "--slurp"])
        .output()
        .context("cannot run gh api for Dependabot provenance")?;
    if !output.status.success() {
        bail!("Dependabot provenance request failed");
    }
    let pages: Vec<Vec<Value>> = serde_json::from_slice(&output.stdout)?;
    let commits: Vec<Value> = pages.into_iter().flatten().collect();

    if !verified_bot_commits(&commits, head) {
        bail!(
            "The checked batch must contain only verified Dependabot commits and still match the event's head commit."
        );
    }
    println!("Dependabot provenance verified. All update types use the required CI gates.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn commit(sha: &str, author: &str, verified: bool) -> Value {
        json!({
            "sha": sha,
            "author": {"login": author},
            "commit": {"verification": {"verified": verified}},
        })
    }

    #[test]
    fn signed_bot_batch_needs_no_version_metadata_or_review() {
        let commits = vec![
            commit("first", "dependabot[bot]", true),
            commit("head", "dependabot[bot]", true),
        ];
        assert!(verified_bot_commits(&commits, "head"));
    }

    #[test]
    fn empty_batch_fails_closed() {
        assert!(!verified_bot_commits(&[], "head"));
    }

    #[test]
    fn changed_head_fails_closed() {
        assert!(!verified_bot_commits(
            &[commit("new", "dependabot[bot]", true)],
            "head"
        ));
    }

    #[test]
    fn every_commit_must_be_from_dependabot() {
        for author in ["contributor", "github-actions[bot]", "dependabot"] {
            let commits = vec![
                commit("first", author, true),
                commit("head", "dependabot[bot]", true),
            ];
            assert!(!verified_bot_commits(&commits, "head"));
        }
    }

    #[test]
    fn every_commit_must_have_a_verified_signature() {
        let unsigned = vec![
            commit("first", "dependabot[bot]", false),
            commit("head", "dependabot[bot]", true),
        ];
        assert!(!verified_bot_commits(&unsigned, "head"));
        let incomplete = vec![
            json!({"sha": "first"}),
            commit("head", "dependabot[bot]", true),
        ];
        assert!(!verified_bot_commits(&incomplete, "head"));
        let null_author = vec![
            json!({"sha": "first", "author": null, "commit": {"verification": {"verified": true}}}),
            commit("head", "dependabot[bot]", true),
        ];
        assert!(!verified_bot_commits(&null_author, "head"));
    }

    #[test]
    fn missing_provenance_fails_closed() {
        for incomplete in [
            json!({}),
            json!({"author": null}),
            json!({"author": {"login": "dependabot[bot]"}}),
        ] {
            let commits = vec![incomplete, commit("head", "dependabot[bot]", true)];
            assert!(!verified_bot_commits(&commits, "head"));
        }
    }
}
