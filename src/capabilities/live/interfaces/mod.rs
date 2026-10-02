//! Live inbound adapters: card and dynamic-badge routes.

mod badges;
mod cards;
mod registry_badges;
mod switch_badges;

pub(crate) use badges::respond as respond_face;
pub(crate) use badges::{github_badge, last_commit_branch, npm_badge, release_badge};
pub(crate) use cards::{
    card_handler, pin_handler, star_history_handler, stats_card, streak_card, streak_handler,
    top_langs_handler, trophy_card, trophy_handler, CardQuery,
};
pub(crate) use registry_badges::{
    bundlephobia_badge, chrome_badge, packagist_badge, pub_badge, workflow_badge,
};
pub(crate) use switch_badges::{crates_badge, docker_badge, endpoint_badge, pypi_badge};
