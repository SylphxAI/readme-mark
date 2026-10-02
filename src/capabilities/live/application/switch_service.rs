//! Cached reads for the shields-switch sources (PyPI, crates.io, Docker Hub,
//! `endpoint`). Kept apart from [`super::service`] so the core service stays
//! small; one `SwitchCaches` field is all it carries.

use std::time::Duration;

use super::cache::{Lookup, Ttl, TtlCache};
use super::service::LiveService;
use super::switch_registries as read;
use crate::capabilities::live::domain::endpoint::EndpointBadge;
use crate::capabilities::live::domain::model::{CrateInfo, DockerRepo, PypiDownloads};

/// An endpoint answers for its owner's own CI or script: minutes fresh, a day
/// stale, and a bad document is retried soon (the owner is fixing it).
const ENDPOINT_TTL: Ttl = Ttl {
    fresh: Duration::from_secs(300),
    stale: Duration::from_secs(24 * 3600),
    not_found: Duration::from_secs(300),
    retry: Duration::from_secs(60),
};

pub(crate) struct SwitchCaches {
    versions: TtlCache<String>,
    pypi: TtlCache<PypiDownloads>,
    crates: TtlCache<CrateInfo>,
    docker: TtlCache<DockerRepo>,
    endpoints: TtlCache<EndpointBadge>,
}

impl SwitchCaches {
    pub(crate) fn new() -> Self {
        Self {
            versions: TtlCache::new("switch-version", 2000, Ttl::BADGE),
            pypi: TtlCache::new("pypi-dl", 1000, Ttl::BADGE),
            crates: TtlCache::new("crate", 2000, Ttl::BADGE),
            docker: TtlCache::new("docker", 2000, Ttl::BADGE),
            endpoints: TtlCache::new("endpoint", 2000, ENDPOINT_TTL),
        }
    }
}

impl LiveService {
    // duplicate-exception: cache-read wrapper binding one TtlCache to one registry reader.
    pub(crate) async fn pypi_version(&self, name: &str) -> Lookup<String> {
        let load = || read::pypi_version(self.up(), name);
        let key = format!("pypi:{}", name.to_ascii_lowercase());
        self.switch.versions.fetch(key, load).await
    }

    // duplicate-exception: cache-read wrapper binding one TtlCache to one registry reader.
    pub(crate) async fn pypi_downloads(&self, name: &str) -> Lookup<PypiDownloads> {
        let load = || read::pypi_downloads(self.up(), name);
        let key = name.to_ascii_lowercase();
        self.switch.pypi.fetch(key, load).await
    }

    // duplicate-exception: cache-read wrapper binding one TtlCache to one registry reader.
    pub(crate) async fn crate_info(&self, name: &str) -> Lookup<CrateInfo> {
        let load = || read::crate_info(self.up(), name);
        let key = name.to_ascii_lowercase();
        self.switch.crates.fetch(key, load).await
    }

    // duplicate-exception: cache-read wrapper binding one TtlCache to one registry reader.
    pub(crate) async fn docker_repo(&self, user: &str, image: &str) -> Lookup<DockerRepo> {
        let load = || read::docker_repo(self.up(), user, image);
        self.switch
            .docker
            .fetch(format!("{user}/{image}"), load)
            .await
    }

    // duplicate-exception: cache-read wrapper binding one TtlCache to one registry reader.
    pub(crate) async fn docker_version(&self, user: &str, image: &str) -> Lookup<String> {
        let load = || read::docker_version(self.up(), user, image);
        let key = format!("docker:{user}/{image}");
        self.switch.versions.fetch(key, load).await
    }

    /// The badge an already vetted `https` endpoint URL serves.
    // duplicate-exception: cache-read wrapper binding one TtlCache to one registry reader.
    pub(crate) async fn endpoint(&self, url: &str) -> Lookup<EndpointBadge> {
        let load = || read::endpoint(self.up(), url.to_string());
        self.switch.endpoints.fetch(url.to_string(), load).await
    }
}
