use anyhow::{anyhow, Error, Result};
use bytes::Bytes;
use futures_util::TryStreamExt;
use reqwest::ClientBuilder;
use tokio::stream::Stream;
use std::pin::Pin;

const HTTPS_DOWNLOAD: &str = "https://www.tosecdev.org/downloads/category/50-2020-07-29?download=99:tosec-dat-pack-complete-3036-tosec-v2020-07-29";

pub async fn fetch_zip() -> Result<(String, u64, Pin<Box<dyn Stream<Item = Result<Bytes>>>>)> {
    let download_req = ClientBuilder::new().build()?.get(HTTPS_DOWNLOAD).send().await?;

    let headers = download_req.headers();

    if !headers
        .get("content-type")
        .map(|t| t == "application/zip" || t == "application/x-zip")
        .unwrap_or(false)
    {
        return Err(anyhow!("Response was not a valid ZIP archive"));
    }

    let content_diposition = headers
        .get("content-disposition")
        .and_then(|s| s.to_str().map(|s| s.to_string()).ok())
        .map(|s| String::from(&s["attachment; filename=\"".len()..s.len() - 1]))
        .ok_or(anyhow!("Unable to fetch attachment filename"))?;

    Ok((
        content_diposition,
        download_req.content_length().unwrap_or(0),
        Box::pin(download_req.bytes_stream().map_err(|e| Error::new(e))),
    ))
}
