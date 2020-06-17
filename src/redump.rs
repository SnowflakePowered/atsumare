use crate::Credentials;
use crate::convert::convert_to_xml_dat;

use anyhow::{anyhow, Error, Result};
use bytes::Bytes;
use futures_util::TryStreamExt;
use futures_util::stream;
use lazy_static::lazy_static;
use regex::Regex;
use reqwest::redirect::Policy;
use reqwest::ClientBuilder;
use scraper::{Html, Selector};
use serde::Serialize;
use std::pin::Pin;
use tokio::stream::Stream;

const HTTP_LOGIN: &str = "http://forum.redump.org/login/";
const HTTP_DOWNLOADS: &str = "http://redump.org/downloads/";
const HTTP_ROOT: &str = "http://redump.org";

#[derive(Debug, Serialize)]
struct Login<'a> {
    req_username: &'a str,
    req_password: &'a str,
    login: &'static str,
    form_sent: &'static str,
    redirect_url: &'static str,
    csrf_token: &'a str,
}

impl<'a> Login<'a> {
    fn login(c: &'a Credentials, csrf: &'a String) -> Self {
        Login {
            req_username: &c.username,
            req_password: &c.password,
            csrf_token: csrf.as_ref(),
            login: "Login",
            form_sent: "1",
            redirect_url: "http://forum.redump.org/",
        }
    }
}

lazy_static! {
    static ref CSRF_RE: Regex =
        Regex::new(r#"<input type="hidden" name="csrf_token" value="([\w]+?)" />"#).unwrap();
}

pub async fn fetch_authenticated_session(credentials: &Credentials) -> Result<String> {
    let login_page = ClientBuilder::new()
        .redirect(Policy::none())
        .build()?
        .get(HTTP_LOGIN)
        .send()
        .await?;

    let session_id = login_page
        .cookies()
        .find(|c| c.name() == "PHPSESSID")
        .map(|c| c.value().to_owned())
        .ok_or(anyhow!("Unable to retrieve session cookie."))?;

    let page_body = &login_page.text().await?;
    let csrf = CSRF_RE
        .captures(page_body)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_owned())
        .ok_or(anyhow!("Unable to find CSRF token."))?;

    let login_req = ClientBuilder::new()
        .redirect(Policy::none())
        .build()?
        .post(HTTP_LOGIN)
        .form(&Login::login(&credentials, &csrf))
        .header("Cookie", format!("PHPSESSID={}", session_id))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send()
        .await?;

    login_req
        .headers()
        .get("location")
        .ok_or(anyhow!("Login credentials were incorrect"))?;

    let session = login_req
        .cookies()
        .find(|c| c.name() == "redump_cookie")
        .map(|c| c.value().to_owned())
        .ok_or(anyhow!("Unable to retrieve session cookie."))?;

    Ok(session)
}

pub async fn fetch_download_urls<S: AsRef<str>>(session: &Option<S>) -> Result<Vec<String>> {
    let downloads_page = ClientBuilder::new().build()?.get(HTTP_DOWNLOADS);
    let downloads_page = if let Some(session) = session {
        downloads_page.header("Cookie", format!("redump_cookie={}", session.as_ref()))
    } else {
        downloads_page
    }
    .send()
    .await?;

    let page_body = &downloads_page.text().await?;
    let fragment = Html::parse_document(page_body);
    let selector = Selector::parse("table.statistics > tbody > tr > td > a")
        .map_err(|_| anyhow!("Unable to parse selector! This should never happen!!"))?;

    let anchors = fragment
        .select(&selector)
        .map(|n| n.value())
        .map(|n| n.attr("href"))
        .flat_map(|n| n)
        .filter(|n| n.starts_with("/datfile/"))
        .map(|n| format!("{}{}", HTTP_ROOT, n))
        .collect::<Vec<_>>();
    Ok(anchors)
}

pub async fn fetch_zip<S: AsRef<str>>(
    download_url: S,
    session: &Option<S>,
) -> Result<(String, u64, Pin<Box<dyn Stream<Item = Result<Bytes>>>>)> {
    let download_req = ClientBuilder::new().build()?.get(download_url.as_ref());
    let download_req = if let Some(session) = session {
        download_req.header("Cookie", format!("redump_cookie={}", session.as_ref()))
    } else {
        download_req
    }
    .send()
    .await?;

    let headers = download_req.headers();
    let content_diposition = headers
        .get("content-disposition")
        .and_then(|s| s.to_str().map(|s| s.to_string()).ok())
        .map(|s| String::from(&s["attachment; filename=\"".len()..s.len() - 1]))
        .ok_or(anyhow!("Unable to fetch attachment filename"))?;

    match headers.get("content-type").and_then(|f| f.to_str().ok()) {
        Some("application/x-zip") | Some("application/zip") => {
            Ok((
                content_diposition,
                download_req.content_length().unwrap_or(0),
                Box::pin(download_req.bytes_stream().map_err(|e| Error::new(e))),
            ))
        },
        Some("application/x-ms-download; charset=ISO-8859-1") => {
            // ISO-8859-1 is the same as windows-1252
            let content = download_req.text_with_charset("windows-1252").await?;
            let bytes = convert_to_xml_dat(&content, "redump.org")?;
            
            Ok((content_diposition, bytes.len() as u64, Box::pin(stream::iter(vec![Ok(bytes)].into_iter()))))
        },
        Some(i) => Err(anyhow!("Response was not a valid ZIP archive or DAT file: {}", i)),
        None => Err(anyhow!("Response did not give valid content-type"))
    }
}
