//! Asking GitHub — the latest release and its `SHA256SUMS` — on the person's
//! pass, through `ronoctua_http::web::request_let`.
//!
//! Every request takes a [`Pass`] for the host of its FIRST address; a
//! redirect is that host's answer (`ronoctua_http::web`'s rule). The
//! transport is a parameter of the inner functions so the tests answer with
//! fixtures and the sandbox's missing network cannot make one pass.

use ronoctua_consent::Pass;

use crate::release::{ANSWER_MOST_BYTES, SUMS_MOST_BYTES, parse_latest};
use crate::{API_HOST, Refusal, Release, Sums, Why};

/// Who asks: GitHub's API refuses a request with no User-Agent.
const USER_AGENT: &str = "ronoctua-stick";

/// The address of the latest release.
fn latest_address() -> String {
    format!(
        "https://{API_HOST}/repos/{}/releases/latest",
        crate::REPOSITORY
    )
}

/// One GET, the body when the answer is a 200.
type Get<'a> = dyn FnMut(&str, u64) -> Result<Vec<u8>, Why> + 'a;

pub(crate) fn latest(pass: &Pass) -> Result<Release, Refusal> {
    latest_by(pass, &mut |url, most| get(pass, url, most))
}

pub(crate) fn sums(pass: &Pass, release: &Release) -> Result<Sums, Refusal> {
    sums_by(pass, release, &mut |url, most| get(pass, url, most))
}

fn latest_by(pass: &Pass, get: &mut Get<'_>) -> Result<Release, Refusal> {
    if !pass.covers(API_HOST) {
        return Err(Refusal::of(Why::NotLet));
    }
    let body = get(&latest_address(), ANSWER_MOST_BYTES).map_err(Refusal::of)?;
    parse_latest(&body)
}

fn sums_by(pass: &Pass, release: &Release, get: &mut Get<'_>) -> Result<Sums, Refusal> {
    let host = crate::release::host_of(&release.sums.url).ok_or(Refusal::of(Why::Failed))?;
    if !pass.covers(host) {
        return Err(Refusal::of(Why::NotLet));
    }
    let body = get(&release.sums.url, SUMS_MOST_BYTES).map_err(Refusal::of)?;
    let text = String::from_utf8(body).map_err(|_| Refusal::of(Why::Failed))?;
    let sums = Sums::parse(&text)?;
    if !sums.covers(release) {
        return Err(Refusal::of(Why::NoRelease));
    }
    Ok(sums)
}

/// The request, over the person's pass.
fn get(pass: &Pass, url: &str, most: u64) -> Result<Vec<u8>, Why> {
    let headers = [("Accept", "application/vnd.github+json")];
    let ask = ronoctua_http::web::Ask::get(url, USER_AGENT)
        .with_headers(&headers)
        .most(most);
    match ronoctua_http::web::request_let(pass, &ask) {
        Ok(got) if got.reply.status == 200 => Ok(got.reply.body),
        // NO RELEASE AT ALL is GitHub's 404 on `latest`.
        Ok(got) if got.reply.status == 404 => Err(Why::NoRelease),
        Ok(_) => Err(Why::Failed),
        Err(e) if e.kind == ronoctua_http::web::Kind::Offline => Err(Why::Offline),
        Err(_) => Err(Why::Failed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pass_for(host: &str, name: &str) -> (Pass, std::path::PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-stick-net-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = ronoctua_consent::store_at(Some(dir.clone()));
        ronoctua_consent::let_hosts_in(&store, &[host], true).unwrap();
        (ronoctua_consent::pass_in(&store, host).unwrap(), dir)
    }

    const ISO: &str = "ronoctua-distro-1.0-x86_64.iso";

    fn answer() -> Vec<u8> {
        let url = |n: &str| {
            format!("https://github.com/ronoctua/ronoctua-distro/releases/download/v1.0/{n}")
        };
        serde_json::to_vec(&serde_json::json!({
            "tag_name": "v1.0",
            "published_at": "2026-10-05T12:00:00Z",
            "assets": [
                { "name": ISO, "size": 10, "browser_download_url": url(ISO) },
                { "name": "SHA256SUMS", "size": 90, "browser_download_url": url("SHA256SUMS") },
            ],
        }))
        .unwrap()
    }

    #[test]
    fn the_latest_is_asked_on_the_apis_pass_and_no_other() {
        let (api, a) = pass_for(API_HOST, "api");
        let mut asked = Vec::new();
        let r = latest_by(&api, &mut |url, _| {
            asked.push(url.to_owned());
            Ok(answer())
        })
        .unwrap();
        assert_eq!(r.label, "1.0");
        assert_eq!(
            asked,
            ["https://api.github.com/repos/ronoctua/ronoctua-distro/releases/latest"]
        );
        // A PASS FOR ANOTHER HOST asks nothing.
        let (other, b) = pass_for("github.com", "other");
        let mut called = false;
        let refused = latest_by(&other, &mut |_, _| {
            called = true;
            Ok(answer())
        });
        assert_eq!(refused.unwrap_err().why, Why::NotLet);
        assert!(!called, "a request went out without its pass");
        let offline = latest_by(&api, &mut |_, _| Err(Why::Offline));
        assert_eq!(offline.unwrap_err().why, Why::Offline);
        let _ = std::fs::remove_dir_all(a);
        let _ = std::fs::remove_dir_all(b);
    }

    #[test]
    fn the_sums_must_name_the_whole_iso() {
        let (api, a) = pass_for(API_HOST, "sums-api");
        let release = latest_by(&api, &mut |_, _| Ok(answer())).unwrap();
        let (dl, b) = pass_for("github.com", "sums-dl");
        let good = format!("{}  {ISO}\n", "1".repeat(64));
        let s = sums_by(&dl, &release, &mut |_, _| Ok(good.clone().into_bytes())).unwrap();
        assert_eq!(s.of(ISO), Some("1".repeat(64).as_str()));
        let missing = format!("{}  other.iso\n", "1".repeat(64));
        let r = sums_by(&dl, &release, &mut |_, _| Ok(missing.clone().into_bytes()));
        assert_eq!(r.unwrap_err().why, Why::NoRelease);
        let r = sums_by(&api, &release, &mut |_, _| Ok(good.clone().into_bytes()));
        assert_eq!(
            r.unwrap_err().why,
            Why::NotLet,
            "the API's pass is not the file's"
        );
        let _ = std::fs::remove_dir_all(a);
        let _ = std::fs::remove_dir_all(b);
    }
}
