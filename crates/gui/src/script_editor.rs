use eframe::egui;

pub(crate) const SCRIPT_EDITOR_ID: &str = "script-editor";

pub(crate) fn insert_snippet_at_editor_cursor(
    ctx: &egui::Context,
    editor_id: egui::Id,
    source: &mut String,
    snippet: &str,
) {
    let Some(mut state) = egui::TextEdit::load_state(ctx, editor_id) else {
        insert_snippet_at_char_index(source, source.chars().count(), snippet);
        return;
    };
    let Some(cursor_range) = state.cursor.char_range() else {
        insert_snippet_at_char_index(source, source.chars().count(), snippet);
        egui::TextEdit::store_state(ctx, editor_id, state);
        return;
    };
    let char_range = cursor_range.as_sorted_char_range();
    let insert_at = char_range.start.min(source.chars().count());
    delete_char_range(source, char_range);
    let inserted_chars = insert_snippet_at_char_index(source, insert_at, snippet);
    let cursor = egui::text::CCursor::new(insert_at + inserted_chars);
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(cursor)));
    egui::TextEdit::store_state(ctx, editor_id, state);
    ctx.memory_mut(|memory| memory.request_focus(editor_id));
}

fn insert_snippet_at_char_index(source: &mut String, char_index: usize, snippet: &str) -> usize {
    let insert_at = char_index.min(source.chars().count());
    let mut text = String::new();
    if insert_at > 0 && source.chars().nth(insert_at - 1) != Some('\n') {
        text.push('\n');
    }
    text.push_str(snippet);
    if !text.ends_with('\n') {
        text.push('\n');
    }
    let byte_index = byte_index_from_char_index(source, insert_at);
    source.insert_str(byte_index, &text);
    text.chars().count()
}

fn delete_char_range(source: &mut String, char_range: std::ops::Range<usize>) {
    if char_range.start >= char_range.end {
        return;
    }
    let start = byte_index_from_char_index(source, char_range.start);
    let end = byte_index_from_char_index(source, char_range.end);
    source.drain(start..end);
}

fn byte_index_from_char_index(source: &str, char_index: usize) -> usize {
    source
        .char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(source.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_insert_uses_char_index() {
        let mut source = String::from("中文\n尾部");

        insert_snippet_at_char_index(&mut source, 2, "run_ms(100);");

        assert_eq!(source, "中文\nrun_ms(100);\n\n尾部");
    }

    #[test]
    fn snippet_insert_replaces_char_range() {
        let mut source = String::from("前缀待替换后缀");

        delete_char_range(&mut source, 2..5);
        insert_snippet_at_char_index(&mut source, 2, "tap_key(S4, 80);");

        assert_eq!(source, "前缀\ntap_key(S4, 80);\n后缀");
    }
}
