use anyhow::Result;
use bytes::Bytes;
use listinfo::de;
use listinfo::parse;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, BytesDecl, Event};
use quick_xml::Writer;
use serde::{self, Deserialize, Serialize};
use std::io::Cursor;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "rom", rename_all(serialize = "lowercase"))]
struct Rom<'a> {
    name: &'a str,
    size: &'a str,
    crc: &'a str,
    md5: &'a str,
    sha1: &'a str,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "datafile", rename_all(serialize = "lowercase"))]
struct Datafile<'a> {
    #[serde(borrow, rename(serialize = "$value"))]
    clrmamepro: Header<'a>,
    #[serde(borrow, rename(serialize = "$value"))]
    game: Vec<Game<'a>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "header", rename_all(serialize = "lowercase"))]
struct Header<'a> {
    name: &'a str,
    description: &'a str,
    category: &'a str,
    version: &'a str,
    author: &'a str,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "game", rename_all(serialize = "lowercase"))]
struct Game<'a> {
    name: &'a str,
    description: &'a str,
    #[serde(rename(serialize = "$value"))]
    rom: Vec<Rom<'a>>,
}

pub fn convert_to_xml_dat<'a, 'b>(s: &'a str, homepage: &'b str) -> Result<Bytes> {
    let parsed_cmp = parse::parse_document(s)?;
    let doc: Datafile = de::from_document(&parsed_cmp)?;
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b'\t', 1);
    // write decl
    writer.write_event(Event::Decl(BytesDecl::new(b"1.0", None, None)))?;
    // write doctype
    writer.write_event(Event::DocType(BytesText::from_escaped_str(
        r#" datafile PUBLIC "-//Logiqx//DTD ROM Management Datafile//EN" "http://www.logiqx.com/Dats/datafile.dtd""#)))?;
    let datafile = BytesStart::borrowed_name(b"datafile");
    writer.write_event(Event::Start(datafile))?;
    push_header(&doc, homepage, &mut writer)?;
    for game in doc.game.iter() {
        push_game(game, doc.clrmamepro.category, &mut writer)?;
    }
    writer.write_event(Event::End(BytesEnd::borrowed(b"datafile")))?;
    Ok(Bytes::from(writer.into_inner().into_inner()))
}

fn push_elem_text<'a>(name: &[u8], text: &'a str, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::borrowed_name(name)))?;
    writer.write_event(Event::Text(BytesText::from_plain_str(text)))?;
    writer.write_event(Event::End(BytesEnd::borrowed(name)))?;
    Ok(())
}

fn push_header(doc: &Datafile, homepage: &str, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<()> {
    writer.write_event(Event::Start( BytesStart::borrowed_name(b"header")))?;
    push_elem_text(b"name", doc.clrmamepro.name, writer)?;
    push_elem_text(b"description", doc.clrmamepro.description, writer)?;
    push_elem_text(b"version", doc.clrmamepro.version, writer)?;
    push_elem_text(b"author", doc.clrmamepro.author, writer)?;
    push_elem_text(b"homepage", homepage, writer)?;
    writer.write_event(Event::End(BytesEnd::borrowed(b"header")))?;
    Ok(())
}

fn push_game(game: &Game, category: &str, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<()> {
    let mut elem = BytesStart::borrowed_name(b"game");
    elem.push_attribute(("name", game.name));
    writer.write_event(Event::Start(elem))?;
    push_elem_text(b"category", category, writer)?;
    push_elem_text(b"description", game.description, writer)?;
    for rom in game.rom.iter() {
        push_rom(rom, writer)?;
    }
    writer.write_event(Event::End(BytesEnd::borrowed(b"game")))?;
    Ok(())
}

fn push_rom(rom: &Rom, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<()> {
    let mut elem = BytesStart::borrowed_name(b"rom");
    elem.push_attribute(("name", rom.name));
    elem.push_attribute(("size", rom.size));
    elem.push_attribute(("crc", rom.crc));
    elem.push_attribute(("md5", rom.md5));
    elem.push_attribute(("sha1", rom.sha1));
    writer.write_event(Event::Empty(elem))?;
    Ok(())
}