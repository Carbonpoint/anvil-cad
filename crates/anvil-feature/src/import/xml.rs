//! A small XML tag scanner, enough for 3MF and SVG. It yields start and
//! end tags with their attributes; text, comments, processing
//! instructions and CDATA are skipped. It is not a full XML parser.

/// One start or end tag, with its attributes.
pub struct Tag<'a> {
    /// Local name, without a namespace prefix.
    pub name: &'a str,
    pub closing: bool,
    /// True for `<x/>`.
    pub empty: bool,
    pub attrs: Vec<(&'a str, String)>,
}

impl Tag<'_> {
    pub fn attr(&self, k: &str) -> Option<&str> {
        self.attrs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.as_str())
    }
}

/// Iterates the tags of an XML text. Text between tags, comments,
/// processing instructions and CDATA are skipped. This is not a full XML
/// parser; it is enough for 3MF.
pub struct Tags<'a> {
    s: &'a str,
    i: usize,
}

impl<'a> Tags<'a> {
    pub fn new(s: &'a str) -> Self {
        Tags { s, i: 0 }
    }
}

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn unescape(v: &str) -> String {
    if !v.contains('&') {
        return v.to_string();
    }
    v.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

impl<'a> Iterator for Tags<'a> {
    type Item = Tag<'a>;
    fn next(&mut self) -> Option<Tag<'a>> {
        loop {
            let rest = self.s.get(self.i..)?;
            let lt = rest.find('<')?;
            let start = self.i + lt;
            let body = &self.s[start + 1..];
            // Comments, CDATA and declarations end with their own marker.
            for (open, close) in [("!--", "-->"), ("![CDATA[", "]]>"), ("?", "?>"), ("!", ">")] {
                if body.starts_with(open) {
                    let end = body.find(close)?;
                    self.i = start + 1 + end + close.len();
                    break;
                }
            }
            if self.i > start {
                continue;
            }
            let gt = body.find('>')?;
            self.i = start + 1 + gt + 1;
            let mut inner = &body[..gt];
            let closing = inner.starts_with('/');
            if closing {
                inner = &inner[1..];
            }
            let empty = inner.ends_with('/');
            if empty {
                inner = &inner[..inner.len() - 1];
            }
            let name_end = inner.find(|c: char| c.is_whitespace()).unwrap_or(inner.len());
            let name = local(&inner[..name_end]);
            let mut attrs = Vec::new();
            let mut a = &inner[name_end..];
            while let Some(eq) = a.find('=') {
                let key = local(a[..eq].trim());
                let after = a[eq + 1..].trim_start();
                let Some(q) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else { break };
                let Some(close) = after[1..].find(q) else { break };
                attrs.push((key, unescape(&after[1..1 + close])));
                a = &after[1 + close + 1..];
            }
            return Some(Tag { name, closing, empty, attrs });
        }
    }
}
