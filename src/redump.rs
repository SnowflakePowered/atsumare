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

const HTTP_LOGIN: &str = "http://forum.redump.org/login/";
const HTTP_DOWNLOADS: &str = "http://redump.org/downloads/";

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
            redirect_url: "http://forum.redump.org/"
        }
    }
}

lazy_static! {
    static ref CSRF_RE: Regex = Regex::new(r#"<input type="hidden" name="csrf_token" value="([\w]+?)" />"#).unwrap();
}

pub async fn fetch_authenticated_session(credentials: Credentials) -> Result<String> {
    let login_page = ClientBuilder::new()
        .redirect(Policy::none())
        .build()?
        .get(HTTP_LOGIN)
        .send().await?;
    
    let session_id = login_page
        .cookies()
        .find(|c| c.name() == "PHPSESSID")
        .map(|c| c.value().to_owned())
        .ok_or(anyhow!("Unable to retrieve session cookie."))?;

    let page_body = &login_page.text().await?;
    let csrf = CSRF_RE.captures(page_body)
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
        .send().await?;

    login_req.headers().get("location")
        .ok_or(anyhow!("Login credentials were incorrect"))?;
   
    let session = login_req
        .cookies()
        .find(|c| c.name() == "redump_cookie")
        .map(|c| c.value().to_owned())
        .ok_or(anyhow!("Unable to retrieve session cookie."))?;
    
    Ok(session)
}