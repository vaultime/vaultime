// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Valve's text `KeyValues` format, which Steam uses for its config files:
//! quoted keys followed by a quoted value or a block in braces.

/// A value or a block of named entries, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Vdf {
    Text(String),
    Block(Vec<(String, Vdf)>),
}

impl Vdf {
    /// The entry `key`, without regard to case, as Steam writes both.
    pub fn get(&self, key: &str) -> Option<&Vdf> {
        self.entries()
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(key))
            .map(|(_, value)| value)
    }

    /// Follows `keys` down through nested blocks.
    pub fn path(&self, keys: &[&str]) -> Option<&Vdf> {
        keys.iter().try_fold(self, |node, key| node.get(key))
    }

    pub fn entries(&self) -> &[(String, Vdf)] {
        match self {
            Vdf::Block(entries) => entries,
            Vdf::Text(_) => &[],
        }
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            Vdf::Text(text) => Some(text),
            Vdf::Block(_) => None,
        }
    }
}

enum Token {
    Open,
    Close,
    Word(String),
}

fn tokens(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            '{' => {
                chars.next();
                tokens.push(Token::Open);
            }
            '}' => {
                chars.next();
                tokens.push(Token::Close);
            }
            '"' => {
                chars.next();
                let mut word = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some('n') => word.push('\n'),
                            Some('t') => word.push('\t'),
                            Some(other) => word.push(other),
                            None => break,
                        },
                        other => word.push(other),
                    }
                }
                tokens.push(Token::Word(word));
            }
            '/' if input_starts_comment(&mut chars) => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            // Platform conditions like [$WIN32] after a value carry no data here.
            '[' => {
                for c in chars.by_ref() {
                    if c == ']' {
                        break;
                    }
                }
            }
            c if c.is_whitespace() => {
                chars.next();
            }
            _ => {
                let mut word = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || matches!(c, '{' | '}' | '"') {
                        break;
                    }
                    word.push(c);
                    chars.next();
                }
                tokens.push(Token::Word(word));
            }
        }
    }
    tokens
}

fn input_starts_comment(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> bool {
    let mut ahead = chars.clone();
    ahead.next();
    ahead.next() == Some('/')
}

/// Parses a whole file into a block of its top level entries.
pub fn parse(input: &str) -> Vdf {
    let mut stack: Vec<(String, Vec<(String, Vdf)>)> = vec![(String::new(), Vec::new())];
    let mut pending_key: Option<String> = None;
    for token in tokens(input) {
        match token {
            Token::Word(word) => match pending_key.take() {
                Some(key) => {
                    if let Some((_, entries)) = stack.last_mut() {
                        entries.push((key, Vdf::Text(word)));
                    }
                }
                None => pending_key = Some(word),
            },
            Token::Open => stack.push((pending_key.take().unwrap_or_default(), Vec::new())),
            Token::Close => {
                pending_key = None;
                if stack.len() > 1
                    && let Some((key, entries)) = stack.pop()
                    && let Some((_, parent)) = stack.last_mut()
                {
                    parent.push((key, Vdf::Block(entries)));
                }
            }
        }
    }
    // A file cut short still yields what it had.
    while stack.len() > 1 {
        if let Some((key, entries)) = stack.pop()
            && let Some((_, parent)) = stack.last_mut()
        {
            parent.push((key, Vdf::Block(entries)));
        }
    }
    Vdf::Block(stack.pop().map(|(_, entries)| entries).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCALCONFIG: &str = r#"
"UserLocalConfigStore"
{
	"Software"
	{
		"valve"
		{
			"Steam"
			{
				"apps"
				{
					"730"
					{
						"LastPlayed"		"1727600000"
						"Playtime"		"12345"
						"cloud" { "last_sync_state" "synchronized" }
					}
					"1245620" { "Playtime" "3100" }
				}
			}
		}
	}
	// a comment
	"streaming_v2" { "EnableStreaming" "0" [$WIN32] }
}
"#;

    #[test]
    fn reads_nested_blocks_without_regard_to_case() {
        let root = parse(LOCALCONFIG);
        let apps = root
            .path(&["UserLocalConfigStore", "software", "Valve", "steam", "Apps"])
            .unwrap();
        assert_eq!(apps.entries().len(), 2);
        assert_eq!(
            apps.path(&["730", "Playtime"]).and_then(Vdf::text),
            Some("12345")
        );
        assert_eq!(
            apps.path(&["730", "LastPlayed"]).and_then(Vdf::text),
            Some("1727600000")
        );
        assert_eq!(
            root.path(&["UserLocalConfigStore", "streaming_v2", "EnableStreaming"])
                .and_then(Vdf::text),
            Some("0")
        );
    }

    #[test]
    fn unescapes_values() {
        let root = parse(r#""path" "C:\\Games\\Steam" "quote" "say \"hi\"""#);
        assert_eq!(
            root.get("path").and_then(Vdf::text),
            Some(r"C:\Games\Steam")
        );
        assert_eq!(root.get("quote").and_then(Vdf::text), Some(r#"say "hi""#));
    }

    #[test]
    fn survives_broken_and_nested_input() {
        let cut = parse(r#""a" { "b" { "c" "1""#);
        assert_eq!(cut.path(&["a", "b", "c"]).and_then(Vdf::text), Some("1"));
        let deep = format!("{}{}", "\"k\" {".repeat(1_000), "}".repeat(1_000));
        assert!(parse(&deep).get("k").is_some());
        assert_eq!(parse("}}}"), Vdf::Block(Vec::new()));
    }
}
