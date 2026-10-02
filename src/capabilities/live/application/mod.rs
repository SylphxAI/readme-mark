//! Live application: upstream port and adapters, caches, and the service.

pub(crate) mod budget;
pub(crate) mod cache;
mod fixtures;
mod github;
pub(crate) mod metrics;
pub(crate) mod npm;
pub(crate) mod registries;
mod service;
pub(crate) mod switch_registries;
pub(crate) mod switch_service;
mod upstream;

pub use service::LiveService;
