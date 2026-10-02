//! Public package registries beyond npm: pub.dev, Packagist, Bundlephobia,
//! and the Chrome Web Store. No token; every call is one bounded GET.

use serde_json::Value;

use super::upstream::{read_body, read_json, Call, Resource, Upstream, UpstreamError};
use crate::capabilities::live::domain::model::{
    BundleSize, ChromeItem, PackagistDownloads, PubScore,
};

pub(super) async fn json(up: &dyn Upstream, url: String) -> Result<Option<Value>, UpstreamError> {
    read_json(up, Call::read(Resource::Registry, url)).await
}

pub(super) fn num(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

/// A non-empty token of at most `max` characters, each `allowed`: the one
/// shape check behind every registry identifier in a badge path.
pub(crate) fn token(s: &str, max: usize, allowed: impl Fn(char) -> bool) -> bool {
    !s.is_empty() && s.len() <= max && s.chars().all(allowed)
}

/// A pub.dev package name: lowercase letters, digits, underscores.
pub(crate) fn valid_pub(name: &str) -> bool {
    token(name, 64, |c| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'
    })
}

/// One Packagist path part (`vendor` or `package`): npm's name charset
/// without `~`.
pub(crate) fn valid_packagist(part: &str) -> bool {
    part.len() <= 100 && !part.contains('~') && super::npm::valid_part(part)
}

/// A Chrome Web Store item id: 32 letters a–p.
pub(crate) fn valid_chrome_id(id: &str) -> bool {
    id.len() == 32 && token(id, 32, |c| ('a'..='p').contains(&c))
}

/// The string at JSON `pointer` in the document at `url`: the one shape of
/// every "latest version" read.
pub(super) async fn version_at(
    up: &dyn Upstream,
    url: String,
    pointer: &str,
) -> Result<Option<String>, UpstreamError> {
    let v = json(up, url).await?;
    Ok(v.and_then(|v| {
        v.pointer(pointer)
            .and_then(Value::as_str)
            .map(str::to_string)
    }))
}

/// The latest pub.dev version.
pub(crate) async fn pub_version(
    up: &dyn Upstream,
    name: &str,
) -> Result<Option<String>, UpstreamError> {
    version_at(
        up,
        format!("https://pub.dev/api/packages/{name}"),
        "/latest/version",
    )
    .await
}

/// Likes, pub points, and 30-day downloads.
pub(crate) async fn pub_score(
    up: &dyn Upstream,
    name: &str,
) -> Result<Option<PubScore>, UpstreamError> {
    let v = json(up, format!("https://pub.dev/api/packages/{name}/score")).await?;
    Ok(v.map(|v| PubScore {
        likes: num(&v, "likeCount"),
        points: num(&v, "grantedPoints"),
        max_points: num(&v, "maxPoints"),
        downloads_30d: num(&v, "downloadCount30Days"),
    }))
}

/// The newest stable Packagist version (tagged, not `dev-`).
pub(crate) async fn packagist_version(
    up: &dyn Upstream,
    vendor: &str,
    package: &str,
) -> Result<Option<String>, UpstreamError> {
    let name = format!("{vendor}/{package}");
    let v = json(up, format!("https://repo.packagist.org/p2/{name}.json")).await?;
    Ok(v.and_then(|v| {
        v.get("packages")
            .and_then(|p| p.get(&name))
            .and_then(Value::as_array)
            .and_then(|versions| {
                versions
                    .iter()
                    .filter_map(|r| r.get("version").and_then(Value::as_str))
                    .find(|ver| !ver.contains("dev") && !ver.contains('-'))
                    .or_else(|| {
                        versions
                            .first()
                            .and_then(|r| r.get("version").and_then(Value::as_str))
                    })
                    .map(str::to_string)
            })
    }))
}

/// `N` counters named `keys` inside the object `group` of the document at
/// `url`; a missing counter reads as zero.
pub(super) async fn counters<const N: usize>(
    up: &dyn Upstream,
    url: String,
    group: &str,
    keys: [&str; N],
) -> Result<Option<[u64; N]>, UpstreamError> {
    let v = json(up, url).await?;
    Ok(v.and_then(|v| {
        let g = v.get(group)?;
        Some(keys.map(|k| num(g, k)))
    }))
}

// duplicate-exception: one counters() call mapped onto a typed record, like PyPI's.
pub(crate) async fn packagist_downloads(
    up: &dyn Upstream,
    vendor: &str,
    package: &str,
) -> Result<Option<PackagistDownloads>, UpstreamError> {
    let url = format!("https://packagist.org/packages/{vendor}/{package}/stats.json");
    let got = counters(up, url, "downloads", ["total", "monthly", "daily"]).await?;
    Ok(got.map(|[total, monthly, daily]| PackagistDownloads {
        total,
        monthly,
        daily,
    }))
}

pub(crate) async fn bundle_size(
    up: &dyn Upstream,
    package: &str,
) -> Result<Option<BundleSize>, UpstreamError> {
    let url = format!(
        "https://bundlephobia.com/api/size?package={}",
        urlencoding::encode(package)
    );
    let v = json(up, url).await?;
    Ok(v.and_then(|v| {
        let min = v.get("size")?.as_u64()?;
        Some(BundleSize {
            min,
            gzip: num(&v, "gzip"),
        })
    }))
}

/// The listing page's embedded item record (`ds:0`); there is no public API.
pub(crate) async fn chrome_item(
    up: &dyn Upstream,
    id: &str,
) -> Result<Option<ChromeItem>, UpstreamError> {
    let url = format!("https://chromewebstore.google.com/detail/{id}");
    let Some(html) = read_body(up, Call::read(Resource::Registry, url)).await? else {
        return Ok(None);
    };
    Ok(parse_chrome_item(&html))
}

/// `[id, icon, name, rating, ratingCount, …, users (14), …, manifest (18)]`.
pub(crate) fn parse_chrome_item(html: &str) -> Option<ChromeItem> {
    let start = html.find("key: 'ds:0'")?;
    let data = &html[start..];
    let open = data.find("data:")? + "data:".len();
    let array = balanced_array(&data[open..])?;
    let v: Value = serde_json::from_str(array).ok()?;
    let item = v.get(0)?;
    let version = item
        .get(18)
        .and_then(Value::as_str)
        .and_then(|m| serde_json::from_str::<Value>(m).ok())
        .and_then(|m| m.get("version").and_then(Value::as_str).map(str::to_string));
    Some(ChromeItem {
        version,
        users: item.get(14).and_then(Value::as_u64)?,
        rating: item.get(3).and_then(Value::as_f64).unwrap_or(0.0),
        rating_count: item.get(4).and_then(Value::as_u64).unwrap_or(0),
    })
}

/// The first balanced `[...]` at the start of `s`, skipping string contents.
fn balanced_array(s: &str) -> Option<&str> {
    let s = s.trim_start();
    if !s.starts_with('[') {
        return None;
    }
    let (mut depth, mut in_str, mut escaped) = (0usize, false, false);
    for (i, c) in s.char_indices() {
        if in_str {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_str = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_validated() {
        assert!(valid_pub("http") && valid_pub("flutter_bloc"));
        assert!(!valid_pub("Http") && !valid_pub("a/b") && !valid_pub(""));
        assert!(valid_packagist("monolog") && !valid_packagist("../x"));
        assert!(valid_chrome_id("gighmmpiobklfepjocnamgkkbiglidom"));
        assert!(!valid_chrome_id("gighmmpiobklfepjocnamgkkbiglidoz"));
    }

    #[test]
    fn chrome_listing_record_parses() {
        let html = r#"<script>AF_initDataCallback({key: 'ds:0', hash: '2', data:[["gighmmpiobklfepjocnamgkkbiglidom","i","AdBlock ] [",4.47,290290,"x","s","u",1,null,null,["c",null,4],1,1,64000000,1,"i",[1,2],"{\"version\": \"6.29.0\", \"name\": \"x\"}"]], sideChannel: {}});</script>"#;
        let item = parse_chrome_item(html).expect("parsed");
        assert_eq!(item.users, 64_000_000);
        assert_eq!(item.rating_count, 290_290);
        assert_eq!(item.version.as_deref(), Some("6.29.0"));
        assert!(parse_chrome_item("<html></html>").is_none());
    }
}
