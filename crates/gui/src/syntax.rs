use eframe::egui;

pub(crate) fn highlight_rhai(
    ui: &egui::Ui,
    source: &str,
    wrap_width: f32,
) -> egui::text::LayoutJob {
    let font_id = egui::TextStyle::Monospace.resolve(ui.style());
    let text_color = ui.visuals().text_color();
    let dark_mode = ui.visuals().dark_mode;
    let plain = egui::text::TextFormat::simple(font_id.clone(), text_color);
    let keyword = egui::text::TextFormat::simple(
        font_id.clone(),
        syntax_color(dark_mode, [103, 164, 255], [25, 82, 170]),
    );
    let api = egui::text::TextFormat::simple(
        font_id.clone(),
        syntax_color(dark_mode, [88, 190, 190], [0, 112, 130]),
    );
    let constant = egui::text::TextFormat::simple(
        font_id.clone(),
        syntax_color(dark_mode, [220, 160, 82], [156, 83, 0]),
    );
    let string = egui::text::TextFormat::simple(
        font_id.clone(),
        syntax_color(dark_mode, [205, 170, 90], [145, 92, 0]),
    );
    let number = egui::text::TextFormat::simple(
        font_id.clone(),
        syntax_color(dark_mode, [190, 142, 238], [110, 70, 160]),
    );
    let mut comment = egui::text::TextFormat::simple(
        font_id,
        syntax_color(dark_mode, [124, 150, 128], [82, 120, 82]),
    );
    comment.italics = true;

    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = wrap_width;
    let mut index = 0;
    while index < source.len() {
        let rest = &source[index..];
        if rest.starts_with("//") {
            let end = rest
                .find('\n')
                .map(|offset| index + offset)
                .unwrap_or(source.len());
            append_highlight(&mut job, &source[index..end], &comment);
            index = end;
        } else if rest.starts_with("/*") {
            let end = rest
                .find("*/")
                .map(|offset| index + offset + 2)
                .unwrap_or(source.len());
            append_highlight(&mut job, &source[index..end], &comment);
            index = end;
        } else if let Some(quote) = string_quote(rest.as_bytes()[0]) {
            let end = scan_quoted(source, index, quote);
            append_highlight(&mut job, &source[index..end], &string);
            index = end;
        } else if rest.as_bytes()[0].is_ascii_digit() {
            let end = scan_number(source, index);
            append_highlight(&mut job, &source[index..end], &number);
            index = end;
        } else if is_ident_start(rest.as_bytes()[0]) {
            let end = scan_ident(source, index);
            let token = &source[index..end];
            let format = if is_rhai_keyword(token) {
                &keyword
            } else if is_judge_api_token(token) {
                &api
            } else if is_judge_constant(token) {
                &constant
            } else {
                &plain
            };
            append_highlight(&mut job, token, format);
            index = end;
        } else {
            let ch = rest.chars().next().expect("source has remaining text");
            let end = index + ch.len_utf8();
            append_highlight(&mut job, &source[index..end], &plain);
            index = end;
        }
    }
    job
}

fn append_highlight(job: &mut egui::text::LayoutJob, text: &str, format: &egui::text::TextFormat) {
    if !text.is_empty() {
        job.append(text, 0.0, format.clone());
    }
}

fn syntax_color(dark_mode: bool, dark: [u8; 3], light: [u8; 3]) -> egui::Color32 {
    let [r, g, b] = if dark_mode { dark } else { light };
    egui::Color32::from_rgb(r, g, b)
}

fn string_quote(byte: u8) -> Option<u8> {
    matches!(byte, b'"' | b'\'' | b'`').then_some(byte)
}

fn scan_quoted(source: &str, start: usize, quote: u8) -> usize {
    let mut index = start + 1;
    while index < source.len() {
        let rest = &source[index..];
        let byte = rest.as_bytes()[0];
        if byte == b'\\' {
            index += 1;
            if index < source.len() {
                let escaped = source[index..]
                    .chars()
                    .next()
                    .expect("escaped text has remaining text");
                index += escaped.len_utf8();
            }
        } else if byte == quote {
            return index + 1;
        } else {
            let ch = rest.chars().next().expect("quoted text has remaining text");
            index += ch.len_utf8();
        }
    }
    source.len()
}

fn scan_number(source: &str, start: usize) -> usize {
    let mut index = start;
    while index < source.len() {
        let byte = source.as_bytes()[index];
        if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.') {
            index += 1;
        } else {
            break;
        }
    }
    index
}

fn scan_ident(source: &str, start: usize) -> usize {
    let mut index = start;
    while index < source.len() {
        let byte = source.as_bytes()[index];
        if is_ident_continue(byte) {
            index += 1;
        } else {
            break;
        }
    }
    index
}

fn is_ident_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

fn is_ident_continue(byte: u8) -> bool {
    is_ident_start(byte) || byte.is_ascii_digit()
}

fn is_rhai_keyword(token: &str) -> bool {
    matches!(
        token,
        "as" | "break"
            | "case"
            | "catch"
            | "const"
            | "continue"
            | "do"
            | "else"
            | "export"
            | "false"
            | "fn"
            | "for"
            | "global"
            | "if"
            | "import"
            | "in"
            | "is"
            | "let"
            | "loop"
            | "private"
            | "public"
            | "return"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "while"
    )
}

fn is_judge_api_token(token: &str) -> bool {
    matches!(
        token,
        "ckpt"
            | "display_number"
            | "display_text"
            | "led_on"
            | "relay_on"
            | "run_ms"
            | "run_to"
            | "run_to_event"
            | "run_to_state"
            | "run_us"
            | "set_voltage"
            | "tap_key"
            | "uart_write"
    ) || token.starts_with("assert_")
        || token.starts_with("display_")
        || token.starts_with("run_")
        || token.starts_with("set_")
        || token.starts_with("tap_")
        || token.starts_with("uart_")
}

fn is_judge_constant(token: &str) -> bool {
    matches!(token, "RB2" | "RD1" | "NET_SIG" | "SIG_OUT" | "DOWN" | "UP")
        || number_suffix_in_range(token, "S", 4, 19)
        || number_suffix_in_range(token, "L", 1, 8)
}

fn number_suffix_in_range(token: &str, prefix: &str, min: u8, max: u8) -> bool {
    token
        .strip_prefix(prefix)
        .and_then(|suffix| suffix.parse::<u8>().ok())
        .is_some_and(|value| (min..=max).contains(&value))
}
