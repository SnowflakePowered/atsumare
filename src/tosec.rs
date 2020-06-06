use anyhow::{anyhow, Error, Result};
use bytes::Bytes;
use futures_util::TryStreamExt;
use reqwest::ClientBuilder;
use tokio::stream::Stream;
use std::pin::Pin;

const TOSEC_PACK: &str = "https://www.tosecdev.org/downloads/category/48-2019-12-24?download=95:tosec-dat-pack-complete-3012-tosec-v2019-12-24";

pub async fn fetch_zip() -> Result<(String, u64, Pin<Box<dyn Stream<Item = Result<Bytes>>>>)> {
    let download_req = ClientBuilder::new().build()?.get(TOSEC_PACK).send().await?;

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
        .unwrap_or(String::from("tosec.zip"));

    Ok((
        content_diposition,
        download_req.content_length().unwrap_or(0),
        Box::pin(download_req.bytes_stream().map_err(|e| Error::new(e))),
    ))
}
