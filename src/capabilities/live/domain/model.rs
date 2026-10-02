//! Live data shapes: what the upstream readers produce and the cards consume.

use super::languages::LangShare;

/// How the stats card's commit number was counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommitSource {
    /// Commit contributions in the last year (GraphQL, server token).
    CommitsLastYear,
    /// All contributions in the last year (public calendar, no token).
    ContributionsLastYear,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UserStats {
    pub login: String,
    pub name: Option<String>,
    pub stars: Option<u64>,
    pub followers: u64,
    /// Public (REST) or non-fork owned (GraphQL) repositories.
    pub repos: Option<u64>,
    /// Account creation timestamp (`YYYY-MM-DDTHH:MM:SSZ`).
    pub created_at: Option<String>,
    pub commits: Option<(u64, CommitSource)>,
    pub prs: Option<u64>,
    pub issues: Option<u64>,
    pub reviews: Option<u64>,
    /// Repositories contributed to last year (GraphQL only).
    pub contributed_to: Option<u64>,
}

/// How top-language weights were measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LangSource {
    /// Bytes per language across owned repositories (GraphQL, server token).
    Bytes,
    /// Each owned repository's primary language weighted by its size (REST).
    PrimaryBySize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TopLangs {
    pub login: String,
    pub langs: Vec<LangShare>,
    pub source: LangSource,
}

/// One owned repository, as the REST listing or GraphQL returns it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RepoInfo {
    pub owner: String,
    pub name: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub language_color: Option<String>,
    pub stars: u64,
    pub forks: u64,
    /// Repository size in KB (the REST `size` field).
    pub size: u64,
    pub fork: bool,
    pub archived: bool,
    pub template: bool,
    pub license: Option<String>,
    pub pushed_at: Option<String>,
}

/// npm package facts for badges.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NpmPackage {
    pub version: String,
    pub license: Option<String>,
}

/// A pub.dev package's score document.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PubScore {
    pub likes: u64,
    pub points: u64,
    pub max_points: u64,
    pub downloads_30d: u64,
}

/// Packagist download counters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PackagistDownloads {
    pub total: u64,
    pub monthly: u64,
    pub daily: u64,
}

/// A bundle's size in bytes, minified and minified + gzipped.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BundleSize {
    pub min: u64,
    pub gzip: u64,
}

/// A Chrome Web Store listing.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChromeItem {
    pub version: Option<String>,
    pub users: u64,
    pub rating: f64,
    pub rating_count: u64,
}

/// The newest run of a GitHub Actions workflow.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WorkflowRun {
    /// The workflow's display name (shields' default label).
    pub name: String,
    /// `queued`, `in_progress`, `completed`, …
    pub status: String,
    /// `success`, `failure`, `cancelled`, … once completed.
    pub conclusion: Option<String>,
}

/// Sampled star counts over time: `(unix seconds, stars)`, oldest first.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StarHistory {
    pub owner: String,
    pub repo: String,
    pub points: Vec<(i64, u64)>,
}

/// PyPI download counters (pypistats' `recent` document).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PypiDownloads {
    pub day: u64,
    pub week: u64,
    pub month: u64,
}

/// A crates.io crate's headline facts.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CrateInfo {
    pub version: String,
    pub downloads: u64,
    /// Downloads over the last 90 days (crates.io's "recent").
    pub recent: u64,
}

/// A Docker Hub repository's counters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockerRepo {
    pub pulls: u64,
    pub stars: u64,
}
