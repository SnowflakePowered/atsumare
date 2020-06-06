use anyhow::Result;
use clap::*;

use std::env::var;
use std::pin::Pin;
use std::time::Duration;

mod nointro;
mod tosec;

const ATSUMARE_DOM_USER: &str = "ATSUMARE_DOM_USER";
const ATSUMARE_DOM_PASS: &str = "ATSUMARE_DOM_PASS";
const ATSUMARE_REDUMP_USER: &str = "ATSUMARE_REDUMP_USER";
const ATSUMARE_REDUMP_PASS: &str = "ATSUMARE_REDUMP_PASS";

use bytes::Bytes;
use futures_util::StreamExt;
use nointro::Prepare;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::stream::Stream;
use tokio::time::delay_for;

#[derive(Debug)]
enum Sources {
    NoIntro(Option<Credentials>),
    Redump(Option<Credentials>),
    TOSEC,
}

#[derive(Debug)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}
#[derive(Debug)]
struct Options {
    output_dir: String,
    sources: Vec<Sources>,
}

fn get_matches() -> Options {
    let matches = App::new("atsumare")
        .version(crate_version!())
        .arg(
            Arg::with_name("nointro")
                .long("datomatic")
                .help("Download DATs from DAT-o-Matic"),
        )
        .arg(
            Arg::with_name("tosec")
                .long("tosec")
                .help("Download DATs from TOSEC"),
        )
        .arg(
            Arg::with_name("redump")
                .long("redump")
                .help("Download DATs from Redump"),
        )
        .group(
            ArgGroup::with_name("sources")
                .args(&["nointro", "tosec", "redump"])
                .required(true)
                .multiple(true),
        )
        .arg(
            Arg::with_name("output")
                .required(true)
                .help("The output directory")
                .index(1),
        )
        .get_matches();

    let mut options = Options {
        output_dir: matches.value_of("output").unwrap_or("unsorted").to_owned(),
        sources: vec![],
    };

    if matches.is_present("nointro") {
        let creds = var(ATSUMARE_DOM_USER)
            .and_then(|username| {
                var(ATSUMARE_DOM_PASS).map(|password| Credentials { username, password })
            })
            .ok();

        options.sources.push(Sources::NoIntro(creds))
    }

    if matches.is_present("tosec") {
        options.sources.push(Sources::TOSEC)
    }

    if matches.is_present("redump") {
        let creds = var(ATSUMARE_REDUMP_USER)
            .and_then(|username| {
                var(ATSUMARE_REDUMP_PASS).map(|password| Credentials { username, password })
            })
            .ok();
        options.sources.push(Sources::Redump(creds))
    }

    options
}

async fn do_download<F>(
    filename: &str,
    stream: Pin<Box<dyn Stream<Item = Result<Bytes>>>>,
    f: F,
) -> Result<u64>
where
    F: Fn(u64) -> (),
{
    let mut output = File::create(&filename).await?;
    let mut written_len: u64 = 0;

    let mut stream = stream;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        output.write_all(&chunk).await?;
        written_len += chunk.len() as u64;
        f(written_len);
    }

    Ok(written_len)
}

async fn download_nointro(c: Option<Credentials>) -> Result<()> {
    let session: Option<String>;
    let mut prepares = Vec::new();
    prepares.push(Prepare::public());

    // DAT-o-matic doesn't seem to actually check credentials when getting the private DAT (!)
    // The login logic still exists... but we don't really need it.. until they fix their site..
    prepares.push(Prepare::private());

    if let Some(credentials) = c {
        match nointro::fetch_authenticated_session(&credentials)
            .await
            .ok()
        {
            Some(logged_in) => {
                session = Some(logged_in);
                println!("No-Intro: Logged in as {}.", credentials.username);
            }
            None => {
                println!("No-Intro: Invalid credentials.");
                session = None;
            }
        }
    } else {
        session = None
    }

    for prepare in prepares {
        let (download_url, session) = nointro::fetch_download_url(&prepare, &session).await?;

        let (filename, length, stream) = nointro::fetch_zip(download_url, session).await?;
        println!("No-Intro: Saving {:?}..", filename);
        do_download(&filename, stream, |f| {
            println!("{:?}: {} of {}", filename, f, length)
        })
        .await?;
        println!("Waiting 30 seconds to avoid throttling...");
        delay_for(Duration::new(30, 0)).await;
    }
    Ok(())
}

async fn download_tosec() -> Result<()> {
    let (filename, length, stream) = tosec::fetch_zip().await?;
    println!("TOSEC: Saving {:?}..", filename);
    do_download(&filename, stream, |f| {
        println!("{:?}: {} of {}", filename, f, length)
    })
    .await?;
    println!("Waiting 30 seconds to avoid throttling...");
    delay_for(Duration::new(30, 0)).await;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // println!("{:?}", );
    let matches = get_matches();
    for source in matches.sources {
        match source {
            Sources::NoIntro(c) => download_nointro(c).await?,
            _ => (),
        }
    }

    // for (filename, length, stream) in tosec::fetch_zip().await? {

    Ok(())
}
