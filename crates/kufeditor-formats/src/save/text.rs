use std::collections::HashSet;

const COLOR_MARKERS: [&[u8]; 2] = [b"@(color=", b"(color="];

pub(super) fn extract_context_text(context: &[u8]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut text = Vec::new();

    for segment in context.split(|byte| !is_context_byte(*byte)) {
        if segment.len() < 4 {
            continue;
        }

        let cleaned = strip_color_codes(segment);
        for line in cleaned.split('\n') {
            let line = line.trim_matches(|character: char| character.is_ascii_whitespace());
            if line.len() < 4 {
                continue;
            }

            let line = line.to_owned();
            if seen.insert(line.clone()) {
                text.push(line);
            }
        }
    }

    text
}

const fn is_context_byte(byte: u8) -> bool {
    byte == b'\r' || byte == b'\n' || (byte >= 0x20 && byte <= 0x7e)
}

fn strip_color_codes(segment: &[u8]) -> String {
    let mut cleaned = String::with_capacity(segment.len());
    let mut remaining = segment;

    while let Some((&byte, rest)) = remaining.split_first() {
        let is_color_code = COLOR_MARKERS
            .iter()
            .any(|marker| remaining.starts_with(marker));
        if is_color_code
            && let Some(closing_offset) = remaining.iter().position(|candidate| *candidate == b')')
        {
            let next_offset = closing_offset.saturating_add(1);
            remaining = remaining.get(next_offset..).unwrap_or_default();
            continue;
        }

        cleaned.push(char::from(byte));
        remaining = rest;
    }

    cleaned
}
