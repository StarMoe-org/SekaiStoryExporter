//! A SekaiStoryRipper library in S3 (`--library s3://bucket/prefix`, ADR-0015).
//!
//! Everything sse reads stays a local file: the episode's bundles are mirrored into a cache
//! directory before parsing, and the rest of sse reads the mirror. Per bundle, the remote
//! `_ripper.json` decides whether the cached copy is current:
//!
//! - `sound/…` voice and SE bundles contribute only the waveforms the index references (BGM
//!   bundles come whole: interactive BGMs play every block's waveform and read the `.acb`);
//! - every other bundle is mirrored whole (models, motions and effect prefabs are read by
//!   directory, and backgrounds, scenarios and movies are small next to the SE packs);
//! - a bundle whose record changed is dropped and fetched again; its record is written last, so a
//!   cached record means a complete mirror.
//!
//! Requests are SigV4-presigned with `rusty-s3` and sent with blocking reqwest over rustls/ring.
//! Credentials come from `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` (+ `AWS_SESSION_TOKEN`);
//! the endpoint from `AWS_ENDPOINT_URL_S3` / `AWS_ENDPOINT_URL` (unset: AWS), the region from
//! `AWS_REGION` / `AWS_DEFAULT_REGION` (unset: `us-east-1`), the addressing style from
//! `S3_ADDRESSING_STYLE` (`auto`: path style for a custom endpoint).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ripper_format::episode::EpisodeIndex;
use ripper_format::unpack::{RECORD_FILE, UnpackRecord};
use rusty_s3::{Bucket, Credentials, S3Action, UrlStyle};

use crate::{AssetError, Result};

const SIGNATURE_TTL: Duration = Duration::from_secs(15 * 60);
const PARALLEL: usize = 16;

fn remote_error(what: impl std::fmt::Display) -> AssetError {
    AssetError::Remote(what.to_string())
}

/// `s3://bucket/prefix` split into its parts; the prefix has no leading or trailing `/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3Location {
    pub bucket: String,
    pub prefix: String,
}

impl S3Location {
    /// `Some` for an `s3://` URL, `None` for anything else (a local path).
    pub fn parse(text: &str) -> Option<Result<Self>> {
        let rest = text.strip_prefix("s3://")?;
        let (bucket, prefix) = rest.split_once('/').unwrap_or((rest, ""));
        Some(if bucket.is_empty() {
            Err(remote_error(format!("{text}: missing bucket name")))
        } else {
            Ok(Self {
                bucket: bucket.to_owned(),
                prefix: prefix.trim_matches('/').to_owned(),
            })
        })
    }

    /// The object key of a `/`-separated path under the prefix.
    pub fn key(&self, relative: &str) -> String {
        if self.prefix.is_empty() {
            relative.to_owned()
        } else {
            format!("{}/{relative}", self.prefix)
        }
    }
}

impl std::fmt::Display for S3Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "s3://{}/{}", self.bucket, self.prefix)
    }
}

fn env(names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|v| !v.is_empty()))
}

fn tls_config() -> rustls::ClientConfig {
    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .expect("ring supports the default protocol versions")
        .with_root_certificates(roots)
        .with_no_client_auth()
}

/// One bucket, addressed through a location's prefix.
pub struct S3 {
    http: reqwest::blocking::Client,
    bucket: Bucket,
    credentials: Credentials,
    location: S3Location,
}

impl S3 {
    pub fn new(location: S3Location) -> Result<Self> {
        let (Some(key), Some(secret)) =
            (env(&["AWS_ACCESS_KEY_ID"]), env(&["AWS_SECRET_ACCESS_KEY"]))
        else {
            return Err(remote_error(
                "S3 needs AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY in the environment",
            ));
        };
        let credentials = match env(&["AWS_SESSION_TOKEN"]) {
            Some(token) => Credentials::new_with_token(key, secret, token),
            None => Credentials::new(key, secret),
        };
        let region =
            env(&["AWS_REGION", "AWS_DEFAULT_REGION"]).unwrap_or_else(|| "us-east-1".into());
        let custom = env(&["AWS_ENDPOINT_URL_S3", "AWS_ENDPOINT_URL"]);
        let style = match env(&["S3_ADDRESSING_STYLE"]).as_deref().unwrap_or("auto") {
            "path" => UrlStyle::Path,
            "virtual" => UrlStyle::VirtualHost,
            "auto" if custom.is_some() => UrlStyle::Path,
            "auto" => UrlStyle::VirtualHost,
            other => {
                return Err(remote_error(format!(
                    "S3_ADDRESSING_STYLE must be auto, path or virtual (got {other:?})"
                )));
            }
        };
        let endpoint = custom.unwrap_or_else(|| format!("https://s3.{region}.amazonaws.com"));
        let endpoint: url::Url = endpoint
            .parse()
            .map_err(|e| remote_error(format!("bad S3 endpoint {endpoint:?}: {e}")))?;
        let bucket = Bucket::new(endpoint, style, location.bucket.clone(), region)
            .map_err(|e| remote_error(format!("S3 bucket {}: {e:?}", location.bucket)))?;
        let http = reqwest::blocking::Client::builder()
            .use_preconfigured_tls(tls_config())
            .timeout(Duration::from_secs(1800))
            .build()
            .map_err(remote_error)?;
        Ok(Self {
            http,
            bucket,
            credentials,
            location,
        })
    }

    pub fn location(&self) -> &S3Location {
        &self.location
    }

    /// The object's bytes, or `None` when it does not exist.
    pub fn get(&self, relative: &str) -> Result<Option<Vec<u8>>> {
        let key = self.location.key(relative);
        let url = self
            .bucket
            .get_object(Some(&self.credentials), &key)
            .sign(SIGNATURE_TTL);
        let response = self.http.get(url).send().map_err(remote_error)?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let response = checked(response, "GET", &key)?;
        Ok(Some(response.bytes().map_err(remote_error)?.to_vec()))
    }

    /// Uploads a local file to `relative`, streaming it.
    pub fn put_file(&self, relative: &str, path: &Path, content_type: &str) -> Result<()> {
        let key = self.location.key(relative);
        let url = self
            .bucket
            .put_object(Some(&self.credentials), &key)
            .sign(SIGNATURE_TTL);
        let file = std::fs::File::open(path).map_err(|source| AssetError::Io {
            path: path.to_owned(),
            source,
        })?;
        let len = file.metadata().map(|m| m.len()).unwrap_or(0);
        let response = self
            .http
            .put(url)
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(reqwest::blocking::Body::sized(file, len))
            .send()
            .map_err(remote_error)?;
        checked(response, "PUT", &key)?;
        Ok(())
    }
}

fn checked(
    response: reqwest::blocking::Response,
    method: &str,
    key: &str,
) -> Result<reqwest::blocking::Response> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let body = response.text().unwrap_or_default();
    Err(remote_error(format!(
        "S3 {method} {key}: {status}: {}",
        body.trim()
    )))
}

fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    let io = |source| AssetError::Io {
        path: path.to_owned(),
        source,
    };
    std::fs::create_dir_all(path.parent().expect("has parent")).map_err(io)?;
    std::fs::write(path, bytes).map_err(io)
}

fn local(root: &Path, relative: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    path.extend(relative.split('/'));
    path
}

/// Mirrors a remote library into `root`.
pub struct Mirror {
    s3: S3,
    root: PathBuf,
}

impl Mirror {
    pub fn new(s3: S3, root: PathBuf) -> Self {
        Self { s3, root }
    }

    pub fn location(&self) -> &S3Location {
        self.s3.location()
    }

    /// Fetches `relative` (always, replacing the cached copy); `false` when it does not exist.
    pub fn refresh(&self, relative: &str) -> Result<bool> {
        match self.s3.get(relative)? {
            Some(bytes) => write(&local(&self.root, relative), &bytes).map(|()| true),
            None => Ok(false),
        }
    }

    /// Brings every file `index` needs into the mirror.
    pub fn sync_episode(&self, index: &EpisodeIndex) -> Result<()> {
        // `sound/…` SE and voice bundles: only the referenced waveforms. BGM bundles come
        // whole: interactive BGMs need every block's waveform and the `.acb`.
        let mut partial: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        let full_bgm: BTreeSet<&str> = index.bgm.values().map(|a| a.bundle.as_str()).collect();
        for audio in index.se.values().chain(index.voices.values()) {
            let files = partial.entry(audio.bundle.as_str()).or_default();
            for file in &audio.files {
                if let Some(rest) = file.strip_prefix(&format!("{}/", audio.bundle)) {
                    files.insert(rest);
                }
            }
        }
        let mut bundles: BTreeSet<&str> = index.bundles.iter().map(String::as_str).collect();
        bundles.extend(partial.keys().copied());
        bundles.extend(full_bgm.iter().copied());
        partial.retain(|b, _| !full_bgm.contains(b));

        let mut downloads = Vec::new();
        let mut records = Vec::new();
        for bundle in bundles {
            let dir = format!("library/{bundle}");
            let record_path = format!("{dir}/{RECORD_FILE}");
            let Some(remote) = self.s3.get(&record_path)? else {
                // Missing bundles are reported by `verify_episode` with the full list.
                continue;
            };
            let record: UnpackRecord = serde_json::from_slice(&remote).map_err(|e| {
                remote_error(format!("{}: {e}", self.s3.location().key(&record_path)))
            })?;
            let wanted: Vec<String> = match partial.get(bundle) {
                Some(files) if bundle.starts_with("sound/") => {
                    files.iter().map(|f| (*f).to_owned()).collect()
                }
                _ => record.files.iter().map(|f| f.path.clone()).collect(),
            };
            let local_dir = local(&self.root, &dir);
            let cached = std::fs::read(local_dir.join(RECORD_FILE)).ok();
            if cached.as_deref() != Some(remote.as_slice()) {
                // New content: drop the stale copy (and its record) before fetching.
                let _ = std::fs::remove_dir_all(&local_dir);
            }
            downloads.extend(
                wanted
                    .into_iter()
                    .map(|file| format!("{dir}/{file}"))
                    .filter(|path| !local(&self.root, path).is_file()),
            );
            records.push((record_path, remote));
        }
        if !downloads.is_empty() {
            eprintln!(
                "fetching {} file(s) from {} ...",
                downloads.len(),
                self.s3.location()
            );
        }
        self.fetch_all(downloads)?;
        for (path, bytes) in records {
            write(&local(&self.root, &path), &bytes)?;
        }
        Ok(())
    }

    fn fetch_all(&self, paths: Vec<String>) -> Result<()> {
        let queue = Mutex::new(paths);
        let failure: Mutex<Option<AssetError>> = Mutex::new(None);
        std::thread::scope(|scope| {
            for _ in 0..PARALLEL {
                scope.spawn(|| {
                    loop {
                        if failure.lock().expect("lock").is_some() {
                            return;
                        }
                        let Some(path) = queue.lock().expect("lock").pop() else {
                            return;
                        };
                        let result = match self.s3.get(&path) {
                            Ok(Some(bytes)) => write(&local(&self.root, &path), &bytes),
                            Ok(None) => Err(remote_error(format!(
                                "{} is listed in its bundle record but missing",
                                self.s3.location().key(&path)
                            ))),
                            Err(error) => Err(error),
                        };
                        if let Err(error) = result {
                            *failure.lock().expect("lock") = Some(error);
                        }
                    }
                });
            }
        });
        match failure.into_inner().expect("lock") {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

/// `Content-Type` of an output file.
pub fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("mp4") => "video/mp4",
        Some("png") => "image/png",
        Some("json") => "application/json",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_s3_urls() {
        assert!(S3Location::parse("/tmp/lib").is_none());
        let loc = S3Location::parse("s3://sekai/jp/").unwrap().unwrap();
        assert_eq!(loc.key("ripper.lock.json"), "jp/ripper.lock.json");
        let root = S3Location::parse("s3://sekai").unwrap().unwrap();
        assert_eq!(root.key("a/b"), "a/b");
        assert!(S3Location::parse("s3:///x").unwrap().is_err());
    }
}
