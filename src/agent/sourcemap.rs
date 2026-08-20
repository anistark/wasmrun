//! Agent mode: map TypeScript stack frames back to their original sources.
//!
//! The swc transpiler emits a sibling `.js.map` for every file it converts
//! under `--source-map`, so a frame naming the generated `.js` can be pointed
//! back at the `.ts` line the author wrote. QuickJS frames carry a file and a
//! line but no column, so lookups are line-granular: the first mapping on a
//! generated line that names a source wins.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A frame's file must look like a session-relative path ending in `.js`, and
/// its map must be small enough to be worth parsing. A map far larger than
/// this is a generated bundle, not agent code.
const MAX_MAP_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct RawSourceMap {
    #[serde(default)]
    sources: Vec<String>,
    #[serde(default)]
    mappings: String,
    #[serde(default, rename = "sourceRoot")]
    source_root: Option<String>,
}

/// Original positions for one generated file, indexed by generated line.
pub struct SourceMap {
    sources: Vec<String>,
    /// `lines[i]` is the origin of generated line `i + 1`, when it has one.
    lines: Vec<Option<Origin>>,
}

#[derive(Clone, Copy)]
struct Origin {
    source: usize,
    /// 1-based, as a stack frame reports it.
    line: u32,
}

impl SourceMap {
    /// Parse a `.map` file, or `None` if it is missing, oversized, or not a
    /// source map this can use.
    pub fn load(path: &Path) -> Option<Self> {
        if std::fs::metadata(path).ok()?.len() > MAX_MAP_BYTES {
            return None;
        }
        let raw: RawSourceMap = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
        if raw.sources.is_empty() {
            return None;
        }

        let root = raw.source_root.unwrap_or_default();
        let sources = raw
            .sources
            .iter()
            .map(|s| match root.is_empty() {
                true => s.clone(),
                false => format!(
                    "{}/{}",
                    root.trim_end_matches('/'),
                    s.trim_start_matches("./")
                ),
            })
            .collect();

        Some(Self {
            sources,
            lines: decode_mappings(&raw.mappings),
        })
    }

    /// The original source and 1-based line for a 1-based generated line.
    pub fn lookup(&self, generated_line: u32) -> Option<(&str, u32)> {
        let origin = (*self.lines.get(generated_line.checked_sub(1)? as usize)?)?;
        Some((self.sources.get(origin.source)?.as_str(), origin.line))
    }
}

/// Decode the `mappings` field into one origin per generated line.
///
/// Segments are `;`-separated by generated line and `,`-separated within one.
/// Every field but the generated column is a delta carried across the whole
/// string, so even lines whose segments are ignored have to be walked.
fn decode_mappings(mappings: &str) -> Vec<Option<Origin>> {
    let mut lines = Vec::new();
    let (mut source, mut source_line) = (0i64, 0i64);

    for group in mappings.split(';') {
        let mut best: Option<Origin> = None;
        for segment in group.split(',').filter(|s| !s.is_empty()) {
            let mut fields = VlqReader::new(segment);
            // Generated column: needed to advance the reader, not to match on.
            if fields.next_value().is_none() {
                break;
            }
            let (Some(d_source), Some(d_line)) = (fields.next_value(), fields.next_value()) else {
                // A one-field segment names no source; the deltas are unchanged.
                continue;
            };
            source += d_source;
            source_line += d_line;
            // Keep the leftmost segment, which is the one a column-less frame
            // is most likely to have come from.
            if best.is_none() && source >= 0 && source_line >= 0 {
                best = Some(Origin {
                    source: source as usize,
                    line: source_line as u32 + 1,
                });
            }
        }
        lines.push(best);
    }
    lines
}

/// Base64 VLQ field reader over one segment.
struct VlqReader<'a> {
    bytes: std::str::Bytes<'a>,
}

impl<'a> VlqReader<'a> {
    fn new(segment: &'a str) -> Self {
        Self {
            bytes: segment.bytes(),
        }
    }

    /// The next zigzag-signed value, or `None` at the end of the segment or on
    /// a character outside the base64 alphabet.
    fn next_value(&mut self) -> Option<i64> {
        let (mut result, mut shift) = (0i64, 0u32);
        loop {
            let digit = base64_digit(self.bytes.next()?)?;
            result |= ((digit & 0x1f) as i64) << shift;
            if digit & 0x20 == 0 {
                // The low bit is the sign, the rest the magnitude.
                let value = result >> 1;
                return Some(if result & 1 == 1 { -value } else { value });
            }
            shift += 5;
            // 64 bits of magnitude is already far past any real offset.
            if shift > 60 {
                return None;
            }
        }
    }
}

fn base64_digit(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Rewrite `file.js:line` in every stack frame of `stderr` to the original
/// source and line, where a sibling `.js.map` under `work_dir` says so.
///
/// Frames naming files with no map (a vendored package, the runtime's own
/// `main.js`) are left exactly as they are, as is a line the map does not
/// cover. Maps are parsed at most once per call.
pub fn remap_stack(stderr: &str, work_dir: &Path) -> String {
    // Cheap bail before any parsing: almost no output mentions a `.js` line.
    if !stderr.contains(".js:") {
        return stderr.to_string();
    }

    let mut maps: HashMap<String, Option<SourceMap>> = HashMap::new();
    let mut out = String::with_capacity(stderr.len());
    let mut rest = stderr;

    while let Some(hit) = rest.find(".js:") {
        let after_ext = hit + ".js:".len();
        // Walk left to the start of the path: a frame wraps it in parentheses
        // and paths never contain whitespace here.
        let path_start = rest[..hit]
            .rfind(|c: char| c.is_whitespace() || c == '(' || c == ')')
            .map(|i| i + 1)
            .unwrap_or(0);
        let digits: String = rest[after_ext..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();

        let replaced = (!digits.is_empty())
            .then(|| digits.parse::<u32>().ok())
            .flatten()
            .and_then(|line| {
                let file = &rest[path_start..after_ext - 1];
                let map = maps
                    .entry(file.to_string())
                    .or_insert_with(|| load_sibling_map(file, work_dir));
                map.as_ref()?.lookup(line)
            })
            .map(|(source, line)| format!("{source}:{line}"));

        match replaced {
            Some(text) => {
                out.push_str(&rest[..path_start]);
                out.push_str(&text);
            }
            None => out.push_str(&rest[..after_ext + digits.len()]),
        }
        rest = &rest[after_ext + digits.len()..];
    }
    out.push_str(rest);
    out
}

/// The `.js.map` beside a frame's file, resolved inside the session.
///
/// The path is taken as session-relative, and anything climbing out of the
/// work dir is refused: a frame is program output, not a trusted path.
fn load_sibling_map(file: &str, work_dir: &Path) -> Option<SourceMap> {
    let relative = file.trim_start_matches('/');
    if relative.is_empty()
        || Path::new(relative)
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return None;
    }
    let candidate: PathBuf = work_dir.join(format!("{relative}.map"));
    SourceMap::load(&candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `AAAA` is the all-zero segment: generated column 0, source 0, line 0.
    #[test]
    fn test_decodes_the_zero_segment() {
        let lines = decode_mappings("AAAA");
        assert_eq!(lines.len(), 1);
        let origin = lines[0].expect("segment names a source");
        assert_eq!((origin.source, origin.line), (0, 1));
    }

    #[test]
    fn test_line_deltas_accumulate_across_groups() {
        // Three generated lines, each mapping one source line further on.
        let lines = decode_mappings("AAAA;AACA;AACA");
        let at = |i: usize| lines[i].expect("mapped").line;
        assert_eq!((at(0), at(1), at(2)), (1, 2, 3));
    }

    #[test]
    fn test_unmapped_generated_line_has_no_origin() {
        let lines = decode_mappings("AAAA;;AACA");
        assert!(lines[1].is_none(), "an empty group maps nothing");
        assert_eq!(lines[2].expect("mapped").line, 2);
    }

    #[test]
    fn test_single_field_segment_names_no_source() {
        // A lone generated column carries no source, and must not shift the
        // deltas for the segments that follow it.
        let lines = decode_mappings("AAAA;C;AACA");
        assert!(lines[1].is_none());
        assert_eq!(lines[2].expect("mapped").line, 2);
    }

    #[test]
    fn test_vlq_decodes_signed_and_continued_values() {
        let read = |s: &str| VlqReader::new(s).next_value();
        assert_eq!(read("A"), Some(0));
        assert_eq!(read("C"), Some(1));
        assert_eq!(read("D"), Some(-1));
        assert_eq!(read("K"), Some(5));
        assert_eq!(read("L"), Some(-5));
        // Two-digit continuation: 'g' sets the continuation bit.
        assert_eq!(read("gB"), Some(16));
        assert_eq!(read("!"), None, "outside the alphabet");
    }

    fn write_map(dir: &Path, generated: &str, json: &str) {
        std::fs::create_dir_all(dir.join(generated).parent().unwrap()).unwrap();
        std::fs::write(dir.join(format!("{generated}.map")), json).unwrap();
    }

    #[test]
    fn test_remaps_a_frame_to_its_original_line() {
        let tmp = tempfile::tempdir().unwrap();
        write_map(
            tmp.path(),
            "main.js",
            r#"{"version":3,"sources":["main.ts"],"mappings":"AAAA;AACA;AACA"}"#,
        );

        let remapped = remap_stack("    at go (/main.js:3)\n", tmp.path());
        assert_eq!(remapped, "    at go (main.ts:3)\n");
    }

    #[test]
    fn test_leaves_frames_without_a_map_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let stack = "    at x (/workspace/runtimes/nodejs/main.js:3497)\n";
        assert_eq!(remap_stack(stack, tmp.path()), stack);
    }

    #[test]
    fn test_leaves_an_uncovered_line_alone() {
        let tmp = tempfile::tempdir().unwrap();
        write_map(
            tmp.path(),
            "main.js",
            r#"{"version":3,"sources":["main.ts"],"mappings":"AAAA"}"#,
        );
        // Only generated line 1 is mapped.
        assert_eq!(
            remap_stack("    at go (/main.js:9)\n", tmp.path()),
            "    at go (/main.js:9)\n"
        );
    }

    #[test]
    fn test_remaps_every_frame_in_a_trace() {
        let tmp = tempfile::tempdir().unwrap();
        let map_for = |source: &str| {
            format!(r#"{{"version":3,"sources":["{source}"],"mappings":"AAAA;AACA;AACA"}}"#)
        };
        write_map(tmp.path(), "lib.js", &map_for("lib.ts"));
        write_map(tmp.path(), "main.js", &map_for("main.ts"));

        let remapped = remap_stack(
            "Error: boom\n    at inner (/lib.js:2)\n    at outer (/main.js:3)\n    at call (native)\n",
            tmp.path(),
        );
        assert_eq!(
            remapped,
            "Error: boom\n    at inner (lib.ts:2)\n    at outer (main.ts:3)\n    at call (native)\n"
        );
    }

    #[test]
    fn test_refuses_a_frame_climbing_out_of_the_session() {
        let tmp = tempfile::tempdir().unwrap();
        // A program can print whatever it likes; a traversal must not be read.
        assert!(load_sibling_map("../../etc/passwd.js", tmp.path()).is_none());
        assert!(load_sibling_map("/../escape.js", tmp.path()).is_none());
    }

    #[test]
    fn test_remaps_a_frame_inside_tap_output() {
        // node:test reports a failing assertion's stack on stdout, indented
        // inside the TAP `stack:` block. That is the trace an agent reads.
        let tmp = tempfile::tempdir().unwrap();
        write_map(
            tmp.path(),
            "sum.test.js",
            r#"{"version":3,"sources":["sum.test.ts"],"mappings":"AAAA;AACA;AACA"}"#,
        );

        let tap = "  stack: |-\n        at <anonymous> (/sum.test.js:3)\n";
        assert_eq!(
            remap_stack(tap, tmp.path()),
            "  stack: |-\n        at <anonymous> (sum.test.ts:3)\n"
        );
    }

    #[test]
    fn test_program_output_that_merely_looks_like_a_frame_is_left_alone() {
        // Rewriting is gated on a source map existing for that exact path, so
        // ordinary stdout mentioning a `.js:line` is not caught by it.
        let tmp = tempfile::tempdir().unwrap();
        let text = "checked vendor/thing.js:42 and moved on\n";
        assert_eq!(remap_stack(text, tmp.path()), text);
    }

    #[test]
    fn test_output_without_a_js_frame_is_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let text = "plain stderr with no frames at all\n";
        assert_eq!(remap_stack(text, tmp.path()), text);
    }

    #[test]
    fn test_source_root_is_joined_onto_sources() {
        let tmp = tempfile::tempdir().unwrap();
        write_map(
            tmp.path(),
            "out.js",
            r#"{"version":3,"sourceRoot":"src/","sources":["./a.ts"],"mappings":"AAAA"}"#,
        );
        assert_eq!(
            remap_stack("    at f (/out.js:1)\n", tmp.path()),
            "    at f (src/a.ts:1)\n"
        );
    }
}
