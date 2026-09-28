// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Just enough XML for the manifests launchers leave in game folders: tags,
//! attributes and element text, no namespaces or nesting rules.

use std::fs;
use std::path::Path;

const UTF16_BOM: [u8; 2] = [0xFF, 0xFE];

/// A text file as UTF-8, or as UTF-16 when it starts with that byte order mark.
pub(crate) fn read_text(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    if let Some(text) = bytes.strip_prefix(&UTF16_BOM) {
        let units: Vec<u16> = text
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        return Some(String::from_utf16_lossy(&units));
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// The attributes of every `<name ...>` tag, `<ExecutableList>` is no `<Executable>`.
pub(crate) fn tags<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    open_tags(xml, name)
        .into_iter()
        .map(|(attributes, _)| attributes)
        .collect()
}

/// Every `<name ...>` tag as its attributes and the index right after its `>`.
fn open_tags<'a>(xml: &'a str, name: &str) -> Vec<(&'a str, usize)> {
    let open = format!("<{name}");
    xml.match_indices(&open)
        .filter_map(|(start, _)| {
            let from = start + open.len();
            let rest = &xml[from..];
            if !rest.starts_with(|c: char| c.is_whitespace() || c == '/' || c == '>') {
                return None;
            }
            let end = rest.find('>')?;
            Some((&rest[..end], from + end + 1))
        })
        .collect()
}

/// The value of `name="..."` in a tag's attributes.
pub(crate) fn attribute(tag: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let (start, _) = tag
        .match_indices(&key)
        .find(|(index, _)| tag[..*index].ends_with(char::is_whitespace))?;
    let value = &tag[start + key.len()..];
    Some(unescape(&value[..value.find('"')?]))
}

/// The trimmed text of every `<name>...</name>` element.
pub(crate) fn texts(xml: &str, name: &str) -> Vec<String> {
    let close = format!("</{name}>");
    open_tags(xml, name)
        .into_iter()
        .filter(|(attributes, _)| !attributes.ends_with('/'))
        .filter_map(|(_, start)| {
            let rest = &xml[start..];
            Some(unescape(rest[..rest.find(&close)?].trim()))
        })
        .collect()
}

fn unescape(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_tags_attributes_and_texts() {
        let xml =
            r#"<list><item id="a &amp; b"/><itemList/><item id="c">Tom &amp; Jerry</item></list>"#;
        let items = tags(xml, "item");
        assert_eq!(items.len(), 2);
        assert_eq!(attribute(items[0], "id").as_deref(), Some("a & b"));
        assert_eq!(texts(xml, "item"), ["Tom & Jerry"]);
        assert_eq!(attribute(items[1], "missing"), None);
    }
}
