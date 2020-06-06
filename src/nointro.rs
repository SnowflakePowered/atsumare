use anyhow::{anyhow, Error, Result};
use bytes::Bytes;
use futures_util::TryStreamExt;
use lazy_static::lazy_static;
use regex::Regex;
use reqwest::redirect::Policy;
use reqwest::ClientBuilder;
use serde::Serialize;
use tokio::stream::Stream;
use std::pin::Pin;
use crate::Credentials;

const HTTPS_ROOT: &str = "https://datomatic.no-intro.org/";
const HTTPS_DAILY: &str = "https://datomatic.no-intro.org/?page=download&op=daily";

lazy_static! {
    static ref DOWNLOAD_RE: Regex =
        Regex::new(r#"^index.php\?page=manager\&download=[0-9]+$"#).unwrap();
}

#[derive(Debug, Serialize)]
pub struct Prepare {
    dat_type: &'static str,
    prepare_2: &'static str,
    private: Option<&'static str>,
}

#[derive(Debug, Serialize)]
struct Download {
    download: &'static str,
}

impl Download {
    const fn download() -> Self {
        Download {
            download: "Download",
        }
    }
}

#[derive(Debug, Serialize)]
struct Login<'a> {
    username: &'a str,
    password: &'a str,
    login: &'static str,
}

impl<'a> Login<'a> {
    fn login(c: &'a Credentials) -> Self {
        Login {
            username: &c.username,
            password: &c.password,
            login: "Login",
        }
    }
}

impl Prepare {
    pub const fn public() -> Prepare {
        Prepare {
            dat_type: "standard",
            prepare_2: "Prepare",
            private: None,
        }
    }

    pub const fn private() -> Prepare {
        Prepare {
            dat_type: "standard",
            prepare_2: "Prepare",
            private: Some("Ok"),
        }
    }
}

pub async fn fetch_authenticated_session(credentials: &Credentials) -> Result<String>{
    let download_req = ClientBuilder::new()
        .redirect(Policy::none())
        .build()?
        .post(HTTPS_ROOT)
        .form(&Login::login(credentials))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send().await?;

    let session = download_req
        .cookies()
        .find(|c| c.name() == "PHPSESSID")
        .map(|c| c.value().to_owned())
        .ok_or(anyhow!("Unable to retrieve session cookie."))?;
    
    if download_req.headers().get("location")
        .ok_or(anyhow!("Unable to retrieve login redirect."))
        .and_then(|l| Ok(l.to_str()?.ends_with("message")))
        .unwrap_or(true) {
            return Err(anyhow!("Login credentials were incorrect"));
        }

    Ok(session)
}

pub async fn fetch_download_url(
    prepare: &Prepare,
    session: &Option<String>,
) -> Result<(String, String)> {
    let download_req = ClientBuilder::new()
        .redirect(Policy::none())
        .build()?
        .post(HTTPS_DAILY)
        .form(prepare)
        .header("Content-Type", "application/x-www-form-urlencoded");

    let download_req = if let Some(session) = session {
        download_req.header("Cookie", format!("PHPSESSID={}", session))
    } else {
        download_req
    }
    .send()
    .await?;

    if let Some(location) = download_req.headers().get("location") {
        let location = location.to_str()?;
        let session = download_req
            .cookies()
            .find(|c| c.name() == "PHPSESSID")
            .map(|c| c.value().to_owned())
            .ok_or(anyhow!("Unable to retrieve session cookie."))?;
        if !DOWNLOAD_RE.is_match(location) {
            Err(anyhow!("Unexpected download URL retrieved: {}", location))
        } else {
            Ok((format!("{}{}", HTTPS_ROOT, location), session))
        }
    } else {
        Err(anyhow!("Unable to fetch download location."))
    }
}

pub async fn fetch_zip<S: AsRef<str>>(
    download_url: S,
    session: S,
) -> Result<(String, u64, Pin<Box<dyn Stream<Item = Result<Bytes>>>>)> {
    let download_req = ClientBuilder::new()
        .redirect(Policy::none())
        .build()?
        .post(download_url.as_ref())
        .form(&Download::download())
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Cookie", format!("PHPSESSID={}", session.as_ref()))
        .send()
        .await?;

    let headers = download_req.headers();

    if !headers
        .get("content-type")
        .map(|t| t == "application/zip")
        .unwrap_or(false)
    {
        return Err(anyhow!("Response was not a valid ZIP archive"));
    }

    let content_diposition = headers
        .get("content-disposition")
        .and_then(|s| s.to_str().map(|s| s.to_string()).ok())
        .map(|s| String::from(&s["attachment; filename=\"".len()..s.len() - 1]))
        .unwrap_or(format!("nointro-{}.zip", session.as_ref()));

    Ok((
        content_diposition,
        download_req.content_length().unwrap_or(0),
        Box::pin(download_req.bytes_stream().map_err(|e| Error::new(e))),
    ))
}
