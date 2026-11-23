//! Source GitHub : une seule requête GraphQL `search` par Live Folder, qui
//! ramène titre, auteur, brouillon, état des checks, décision de review et
//! conflits. Coût : ~1 point de rate limit par rafraîchissement.

use serde::Deserialize;
use serde_json::{json, Value};
use voile_core::{Badge, LiveItem};

const ENDPOINT: &str = "https://api.github.com/graphql";

const QUERY: &str = r#"
query Voile($q: String!) {
  search(query: $q, type: ISSUE, first: 50) {
    nodes {
      __typename
      ... on PullRequest {
        number title url isDraft updatedAt mergeable reviewDecision
        repository { nameWithOwner }
        author { login avatarUrl(size: 40) }
        commits(last: 1) { nodes { commit { statusCheckRollup { state } } } }
      }
      ... on Issue {
        number title url updatedAt
        repository { nameWithOwner }
        author { login avatarUrl(size: 40) }
      }
    }
  }
}
"#;

#[derive(Debug, thiserror::Error)]
pub enum GithubError {
    #[error("token GitHub introuvable : définis GITHUB_TOKEN ou lance `gh auth login`")]
    NoToken,
    #[error("token GitHub refusé (401) : vérifie ses droits (repo, read:org)")]
    Unauthorized,
    #[error("limite d'API GitHub atteinte, nouvel essai plus tard")]
    RateLimited,
    #[error("GitHub : {0}")]
    Api(String),
    #[error("réseau : {0}")]
    Http(#[from] reqwest::Error),
}

#[derive(Clone)]
pub struct GithubClient {
    http: reqwest::Client,
    token: String,
    endpoint: String,
}

impl GithubClient {
    pub fn new(token: String) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("voile/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .expect("client HTTP");
        Self {
            http,
            token,
            endpoint: ENDPOINT.into(),
        }
    }

    /// `GITHUB_TOKEN` / `GH_TOKEN`, sinon le token de la CLI `gh`.
    pub async fn from_env() -> Result<Self, GithubError> {
        for var in ["VOILE_GITHUB_TOKEN", "GITHUB_TOKEN", "GH_TOKEN"] {
            if let Ok(t) = std::env::var(var) {
                if !t.trim().is_empty() {
                    return Ok(Self::new(t.trim().to_string()));
                }
            }
        }
        let out = tokio::process::Command::new("gh")
            .args(["auth", "token"])
            .output()
            .await
            .map_err(|_| GithubError::NoToken)?;
        let token = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if out.status.success() && !token.is_empty() {
            Ok(Self::new(token))
        } else {
            Err(GithubError::NoToken)
        }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<LiveItem>, GithubError> {
        let res = self
            .http
            .post(&self.endpoint)
            .bearer_auth(&self.token)
            .json(&json!({ "query": QUERY, "variables": { "q": query } }))
            .send()
            .await?;
        match res.status().as_u16() {
            401 => return Err(GithubError::Unauthorized),
            403 | 429 => return Err(GithubError::RateLimited),
            _ => {}
        }
        let body: Value = res.error_for_status()?.json().await?;
        parse_search(&body)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawNode {
    #[serde(rename = "__typename")]
    typename: Option<String>,
    number: Option<u64>,
    title: Option<String>,
    url: Option<String>,
    #[serde(default)]
    is_draft: bool,
    updated_at: Option<String>,
    mergeable: Option<String>,
    review_decision: Option<String>,
    repository: Option<Repo>,
    author: Option<Author>,
    commits: Option<Commits>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Repo {
    name_with_owner: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Author {
    login: String,
    avatar_url: Option<String>,
}

#[derive(Deserialize)]
struct Commits {
    nodes: Vec<CommitNode>,
}

#[derive(Deserialize)]
struct CommitNode {
    commit: Commit,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Commit {
    status_check_rollup: Option<Rollup>,
}

#[derive(Deserialize)]
struct Rollup {
    state: String,
}

/// Convertit la réponse GraphQL en éléments de Live Folder.
pub fn parse_search(body: &Value) -> Result<Vec<LiveItem>, GithubError> {
    if let Some(errors) = body.get("errors").and_then(Value::as_array) {
        let msg = errors
            .iter()
            .filter_map(|e| e.get("message").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(" ; ");
        if errors
            .iter()
            .any(|e| e.get("type").and_then(Value::as_str) == Some("RATE_LIMITED"))
        {
            return Err(GithubError::RateLimited);
        }
        if body.pointer("/data/search").is_none() {
            return Err(GithubError::Api(msg));
        }
        tracing::warn!("réponse GitHub partielle : {msg}");
    }
    let nodes = body
        .pointer("/data/search/nodes")
        .cloned()
        .unwrap_or(Value::Array(vec![]));
    let nodes: Vec<Option<RawNode>> =
        serde_json::from_value(nodes).map_err(|e| GithubError::Api(e.to_string()))?;

    Ok(nodes.into_iter().flatten().filter_map(to_item).collect())
}

fn to_item(n: RawNode) -> Option<LiveItem> {
    let repo = n.repository?.name_with_owner;
    let number = n.number?;
    let author = n.author;
    let mut badges = Vec::new();
    let is_pr = n.typename.as_deref() == Some("PullRequest");
    if is_pr {
        if n.is_draft {
            badges.push(Badge::Draft);
        }
        let rollup = n
            .commits
            .and_then(|c| c.nodes.into_iter().last())
            .and_then(|c| c.commit.status_check_rollup)
            .map(|r| r.state);
        match rollup.as_deref() {
            Some("SUCCESS") => badges.push(Badge::ChecksPassing),
            Some("FAILURE" | "ERROR") => badges.push(Badge::ChecksFailing),
            Some("PENDING" | "EXPECTED") => badges.push(Badge::ChecksPending),
            _ => {}
        }
        match n.review_decision.as_deref() {
            Some("APPROVED") => badges.push(Badge::Approved),
            Some("CHANGES_REQUESTED") => badges.push(Badge::ChangesRequested),
            Some("REVIEW_REQUIRED") => badges.push(Badge::ReviewRequired),
            _ => {}
        }
        if n.mergeable.as_deref() == Some("CONFLICTING") {
            badges.push(Badge::Conflicts);
        }
    }
    let subtitle = match &author {
        Some(a) => format!("{repo} #{number} · @{}", a.login),
        None => format!("{repo} #{number}"),
    };
    Some(LiveItem {
        key: format!("{repo}#{number}"),
        title: n.title.unwrap_or_default(),
        url: n.url?,
        subtitle,
        updated_at: n.updated_at.unwrap_or_default(),
        badges,
        avatar: author.and_then(|a| a.avatar_url),
        unread: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_prs_and_badges() {
        let body: Value =
            serde_json::from_str(include_str!("../tests/fixtures/search.json")).unwrap();
        let items = parse_search(&body).unwrap();
        assert_eq!(items.len(), 3);

        assert_eq!(items[0].key, "mathieuletyrant/cozytouch#58");
        assert_eq!(
            items[0].subtitle,
            "mathieuletyrant/cozytouch #58 · @julie-dev"
        );
        assert_eq!(
            items[0].badges,
            vec![Badge::ChecksPassing, Badge::ReviewRequired]
        );

        assert_eq!(
            items[1].badges,
            vec![Badge::Draft, Badge::ChecksFailing, Badge::Conflicts]
        );

        // Une issue n'a pas de badges de PR.
        assert!(items[2].badges.is_empty());
        assert_eq!(
            items[2].url,
            "https://github.com/bardesss/arr-mcp/issues/12"
        );
    }

    #[test]
    fn surfaces_graphql_errors() {
        let body = json!({ "errors": [{ "message": "Bad credentials" }] });
        assert!(matches!(parse_search(&body), Err(GithubError::Api(m)) if m == "Bad credentials"));
        let body = json!({ "errors": [{ "type": "RATE_LIMITED", "message": "slow down" }] });
        assert!(matches!(parse_search(&body), Err(GithubError::RateLimited)));
    }
}
