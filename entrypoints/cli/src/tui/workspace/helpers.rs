use ratatui::prelude::*;

/// Estimate the number of visual lines after word-wrap at the given width.
pub fn count_visual_lines(lines: &[Line], width: u16) -> usize {
    if width == 0 {
        return 0;
    }
    let w = width as usize;
    let mut count = 0;
    for line in lines {
        let line_width = line.width();
        if line_width == 0 {
            count += 1;
        } else {
            count += line_width.div_ceil(w);
        }
    }
    count
}

pub fn truncate_chars(text: &str, max: usize) -> String {
    if char_len(text) <= max {
        return text.to_string();
    }
    if max <= 3 {
        return ".".repeat(max);
    }

    let mut out = String::new();
    let target = max - 3;
    for ch in text.chars() {
        let mut candidate = out.clone();
        candidate.push(ch);
        if char_len(&candidate) > target {
            break;
        }
        out.push(ch);
    }
    out.push_str("...");
    out
}

pub fn char_len(text: &str) -> usize {
    Line::from(text.to_string()).width()
}

pub fn fit_left_right(left: &str, right: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let left_len = char_len(left);
    let right_len = char_len(right);
    if left_len + right_len + 1 >= width {
        if right_len >= width {
            return truncate_chars(right, width);
        }
        let available_left = width.saturating_sub(right_len + 1);
        return format!("{} {}", truncate_chars(left, available_left), right);
    }

    format!(
        "{}{}{}",
        left,
        " ".repeat(width.saturating_sub(left_len + right_len)),
        right
    )
}

pub fn sanitize_for_tui(text: &str) -> String {
    strip_ansi_codes(text).trim().to_string()
}

pub fn strip_ansi_codes(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }

    out
}

pub fn compact_path(path: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    if !home.is_empty() && path.starts_with(&home) {
        format!("~{}", &path[home.len()..])
    } else {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_codes_are_removed_for_tui_blocks() {
        assert_eq!(strip_ansi_codes("\x1b[32mOK\x1b[0m"), "OK");
    }
}
