#![allow(
    dead_code,
    reason = "Task 7 wires the reusable text input into the SkillInfo form"
)]

use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable, GlobalElementId, Hsla, KeyBinding,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point,
    ShapedLine, SharedString, Style, TextRun, UTF16Selection, UnderlineStyle, Window, actions, div,
    fill, point, prelude::*, px, relative, size,
};
use unicode_segmentation::UnicodeSegmentation;

const KEY_CONTEXT: &str = "KufEditorTextInput";

actions!(
    kufeditor_text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
        Commit,
        Cancel,
    ]
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TextInputEvent {
    Commit(String),
    Cancel,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TextInputColors {
    pub background: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    pub placeholder: Hsla,
    pub selection: Hsla,
    pub cursor: Hsla,
}

#[derive(Clone, Debug)]
struct TextBuffer {
    content: String,
    selection: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
}

impl TextBuffer {
    fn new(content: impl Into<String>) -> Self {
        let content = content.into();
        let selection = 0..content.len();
        Self {
            content,
            selection,
            selection_reversed: false,
            marked_range: None,
        }
    }

    fn content(&self) -> &str {
        &self.content
    }

    fn selection(&self) -> Range<usize> {
        self.selection.clone()
    }

    const fn selection_reversed(&self) -> bool {
        self.selection_reversed
    }

    fn marked_range(&self) -> Option<Range<usize>> {
        self.marked_range.clone()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selection.start
        } else {
            self.selection.end
        }
    }

    fn move_to(&mut self, offset: usize) {
        self.selection = offset..offset;
        self.selection_reversed = false;
    }

    fn select_to(&mut self, offset: usize) {
        if self.selection_reversed {
            self.selection.start = offset;
        } else {
            self.selection.end = offset;
        }

        if self.selection.end < self.selection.start {
            self.selection_reversed = !self.selection_reversed;
            self.selection = self.selection.end..self.selection.start;
        }
    }

    fn home(&mut self) {
        self.move_to(0);
    }

    fn end(&mut self) {
        self.move_to(self.content.len());
    }

    fn left(&mut self) {
        if self.selection.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()));
        } else {
            self.move_to(self.selection.start);
        }
    }

    fn right(&mut self) {
        if self.selection.is_empty() {
            self.move_to(self.next_boundary(self.cursor_offset()));
        } else {
            self.move_to(self.selection.end);
        }
    }

    fn select_left(&mut self) {
        self.select_to(self.previous_boundary(self.cursor_offset()));
    }

    fn select_right(&mut self) {
        self.select_to(self.next_boundary(self.cursor_offset()));
    }

    fn select_all(&mut self) {
        self.selection = 0..self.content.len();
        self.selection_reversed = false;
    }

    fn backspace(&mut self) {
        if self.selection.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()));
        }
        self.replace_selected("");
    }

    fn delete(&mut self) {
        if self.selection.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()));
        }
        self.replace_selected("");
    }

    fn replace_text(&mut self, range_utf16: Option<Range<usize>>, new_text: &str) {
        let range = self.replacement_range(range_utf16);
        let normalized = normalize_single_line(new_text);
        self.replace_range(range, &normalized.text);
        self.marked_range = None;
    }

    fn replace_and_mark(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        selected_range_utf16: Option<Range<usize>>,
    ) {
        let range = self.replacement_range(range_utf16);
        let normalized = normalize_single_line(new_text);
        let selection = selected_range_utf16.map(|relative| {
            let normalized_range = normalized.translate_utf16_range(&relative);
            range_from_utf16_in(&normalized.text, &normalized_range)
        });
        let start = range.start;

        self.replace_range(range, &normalized.text);
        self.marked_range =
            (!normalized.text.is_empty()).then(|| start..start + normalized.text.len());
        if let Some(selection) = selection {
            self.selection = start + selection.start..start + selection.end;
        }
    }

    fn unmark(&mut self) {
        self.marked_range = None;
    }

    fn text_for_utf16_range(&self, range_utf16: &Range<usize>) -> (String, Range<usize>) {
        let range = self.range_from_utf16(range_utf16);
        let text = self
            .content
            .get(range.clone())
            .unwrap_or_default()
            .to_owned();
        let adjusted = self.range_to_utf16(&range);
        (text, adjusted)
    }

    fn selected_text(&self) -> Option<String> {
        (!self.selection.is_empty()).then(|| {
            self.content
                .get(self.selection.clone())
                .unwrap_or_default()
                .to_owned()
        })
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        utf16_offset_from_byte(&self.content, range.start)
            ..utf16_offset_from_byte(&self.content, range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        range_from_utf16_in(&self.content, range_utf16)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(index, _)| (index > offset).then_some(index))
            .unwrap_or(self.content.len())
    }

    fn replacement_range(&self, range_utf16: Option<Range<usize>>) -> Range<usize> {
        range_utf16
            .map(|range| self.range_from_utf16(&range))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.selection.clone())
    }

    fn replace_selected(&mut self, new_text: &str) {
        self.replace_range(self.selection.clone(), new_text);
        self.marked_range = None;
    }

    fn replace_range(&mut self, range: Range<usize>, new_text: &str) {
        self.content.replace_range(range.clone(), new_text);
        let cursor = range.start + new_text.len();
        self.selection = cursor..cursor;
        self.selection_reversed = false;
    }

    fn commit(&self) -> TextInputEvent {
        TextInputEvent::Commit(self.content.clone())
    }

    const fn cancel() -> TextInputEvent {
        TextInputEvent::Cancel
    }
}

fn range_from_utf16_in(text: &str, range_utf16: &Range<usize>) -> Range<usize> {
    byte_offset_from_utf16(text, range_utf16.start)..byte_offset_from_utf16(text, range_utf16.end)
}

fn byte_offset_from_utf16(text: &str, offset: usize) -> usize {
    let mut utf16_offset = 0;
    for (byte_offset, character) in text.char_indices() {
        if utf16_offset >= offset {
            return byte_offset;
        }
        utf16_offset += character.len_utf16();
    }
    text.len()
}

fn utf16_offset_from_byte(text: &str, offset: usize) -> usize {
    text.char_indices()
        .take_while(|(byte_offset, _)| *byte_offset < offset)
        .map(|(_, character)| character.len_utf16())
        .sum()
}

struct NormalizedText {
    text: String,
    utf16_offset_map: Vec<usize>,
    utf16_len: usize,
}

impl NormalizedText {
    fn translate_utf16_range(&self, source: &Range<usize>) -> Range<usize> {
        self.translate_utf16_offset(source.start)..self.translate_utf16_offset(source.end)
    }

    fn translate_utf16_offset(&self, source: usize) -> usize {
        self.utf16_offset_map
            .get(source)
            .copied()
            .unwrap_or(self.utf16_len)
    }
}

fn normalize_single_line(text: &str) -> NormalizedText {
    let mut output = String::with_capacity(text.len());
    let mut utf16_offset_map = Vec::with_capacity(text.encode_utf16().count() + 1);
    utf16_offset_map.push(0);
    let mut output_utf16_len = 0;
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        let source_utf16_len = match character {
            '\r' => {
                let mut source_utf16_len = 1;
                if characters.peek() == Some(&'\n') {
                    characters.next();
                    source_utf16_len += 1;
                }
                output.push(' ');
                output_utf16_len += 1;
                source_utf16_len
            }
            '\n' => {
                output.push(' ');
                output_utf16_len += 1;
                1
            }
            _ => {
                output.push(character);
                let character_utf16_len = character.len_utf16();
                output_utf16_len += character_utf16_len;
                character_utf16_len
            }
        };
        for _ in 0..source_utf16_len {
            utf16_offset_map.push(output_utf16_len);
        }
    }
    NormalizedText {
        text: output,
        utf16_offset_map,
        utf16_len: output_utf16_len,
    }
}

pub(crate) fn bind(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some(KEY_CONTEXT)),
        KeyBinding::new("delete", Delete, Some(KEY_CONTEXT)),
        KeyBinding::new("left", Left, Some(KEY_CONTEXT)),
        KeyBinding::new("right", Right, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-left", SelectLeft, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-right", SelectRight, Some(KEY_CONTEXT)),
        KeyBinding::new("home", Home, Some(KEY_CONTEXT)),
        KeyBinding::new("end", End, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-a", SelectAll, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-a", SelectAll, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-c", Copy, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-c", Copy, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-x", Cut, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-x", Cut, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-v", Paste, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-v", Paste, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", Commit, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", Cancel, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some(KEY_CONTEXT)),
    ]);
}

pub(crate) struct TextInput {
    focus_handle: FocusHandle,
    buffer: TextBuffer,
    placeholder: SharedString,
    colors: TextInputColors,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
}

impl TextInput {
    pub(crate) fn new(
        content: impl Into<String>,
        placeholder: impl Into<SharedString>,
        colors: TextInputColors,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            buffer: TextBuffer::new(content),
            placeholder: placeholder.into(),
            colors,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
        }
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        self.buffer.left();
        cx.notify();
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        self.buffer.right();
        cx.notify();
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.buffer.select_left();
        cx.notify();
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.buffer.select_right();
        cx.notify();
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.buffer.select_all();
        cx.notify();
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.buffer.home();
        cx.notify();
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.buffer.end();
        cx.notify();
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        self.buffer.backspace();
        cx.notify();
    }

    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        self.buffer.delete();
        cx.notify();
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.buffer.replace_text(None, &text);
            cx.notify();
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.buffer.selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.buffer.selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            self.buffer.replace_selected("");
            cx.notify();
        }
    }

    fn commit(&mut self, _: &Commit, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(self.buffer.commit());
    }

    #[allow(
        clippy::unused_self,
        reason = "GPUI action listeners require an entity receiver"
    )]
    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextBuffer::cancel());
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        self.is_selecting = true;
        let offset = self.index_for_mouse_position(event.position);
        if event.modifiers.shift {
            self.buffer.select_to(offset);
        } else {
            self.buffer.move_to(offset);
        }
        cx.notify();
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            let offset = self.index_for_mouse_position(event.position);
            self.buffer.select_to(offset);
            cx.notify();
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.buffer.content().is_empty() {
            return 0;
        }

        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.buffer.content().len();
        }
        line.closest_index_for_x(position.x - bounds.left())
    }
}

impl EventEmitter<TextInputEvent> for TextInput {}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let (text, adjusted) = self.buffer.text_for_utf16_range(&range_utf16);
        adjusted_range.replace(adjusted);
        Some(text)
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.buffer.range_to_utf16(&self.buffer.selection()),
            reversed: self.buffer.selection_reversed(),
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.buffer
            .marked_range()
            .map(|range| self.buffer.range_to_utf16(&range))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.buffer.unmark();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.buffer.replace_text(range_utf16, text);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.buffer
            .replace_and_mark(range_utf16, text, selected_range_utf16);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let line = self.last_layout.as_ref()?;
        let range = self.buffer.range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(bounds.left() + line.x_for_index(range.start), bounds.top()),
            point(bounds.left() + line.x_for_index(range.end), bounds.bottom()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let local_point = self.last_bounds?.localize(&point)?;
        let line = self.last_layout.as_ref()?;
        let byte_index = line.closest_index_for_x(local_point.x);
        Some(utf16_offset_from_byte(self.buffer.content(), byte_index))
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content: SharedString = input.buffer.content().to_owned().into();
        let selection = input.buffer.selection();
        let cursor_offset = input.buffer.cursor_offset();
        let text_style = window.text_style();
        let (display_text, color) = if content.is_empty() {
            (input.placeholder.clone(), input.colors.placeholder)
        } else {
            (content, input.colors.text)
        };

        let run = TextRun {
            len: display_text.len(),
            font: text_style.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if let Some(marked_range) = input.buffer.marked_range() {
            vec![
                TextRun {
                    len: marked_range.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked_range.end - marked_range.start,
                    underline: Some(UnderlineStyle {
                        color: Some(input.colors.cursor),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len() - marked_range.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);
        let cursor_x = line.x_for_index(cursor_offset);
        let (selection_quad, cursor_quad) = if selection.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_x, bounds.top()),
                        size(px(1.0), bounds.size.height),
                    ),
                    input.colors.cursor,
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(
                            bounds.left() + line.x_for_index(selection.start),
                            bounds.top(),
                        ),
                        point(
                            bounds.left() + line.x_for_index(selection.end),
                            bounds.bottom(),
                        ),
                    ),
                    input.colors.selection,
                )),
                None,
            )
        };

        PrepaintState {
            line: Some(line),
            cursor: cursor_quad,
            selection: selection_quad,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );

        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection);
        }
        let Some(line) = prepaint.line.take() else {
            return;
        };
        if line
            .paint(bounds.origin, window.line_height(), window, cx)
            .is_err()
        {
            return;
        }
        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }

        self.input.update(cx, |input, _| {
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle(cx))
            .flex()
            .items_center()
            .w_full()
            .h(px(38.0))
            .px(px(10.0))
            .rounded_md()
            .border_1()
            .border_color(self.colors.border)
            .bg(self.colors.background)
            .text_color(self.colors.text)
            .text_size(px(14.0))
            .line_height(px(22.0))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::commit))
            .on_action(cx.listener(Self::cancel))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .child(TextElement { input: cx.entity() })
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::{TextBuffer, TextInputEvent};

    #[test]
    fn insert_replaces_the_selected_initial_value() {
        let mut buffer = TextBuffer::new("original");

        buffer.replace_text(None, "n");
        buffer.replace_text(None, "ew");

        assert_eq!(buffer.content(), "new");
        assert_eq!(buffer.selection(), 3..3);
    }

    #[test]
    fn backspace_removes_one_unicode_grapheme() {
        let mut buffer = TextBuffer::new("a🇺🇸b");
        buffer.end();

        buffer.backspace();
        assert_eq!(buffer.content(), "a🇺🇸");
        assert_eq!(buffer.selection(), 9..9);

        buffer.backspace();
        assert_eq!(buffer.content(), "a");
        assert_eq!(buffer.selection(), 1..1);
    }

    #[test]
    fn delete_removes_one_unicode_grapheme() {
        let mut buffer = TextBuffer::new("a🇺🇸b");
        buffer.home();

        buffer.delete();
        assert_eq!(buffer.content(), "🇺🇸b");
        assert_eq!(buffer.selection(), 0..0);

        buffer.delete();
        assert_eq!(buffer.content(), "b");
        assert_eq!(buffer.selection(), 0..0);
    }

    #[test]
    fn left_and_right_stop_at_grapheme_boundaries() {
        let mut buffer = TextBuffer::new("a🇺🇸b");
        buffer.home();

        buffer.right();
        assert_eq!(buffer.selection(), 1..1);
        buffer.right();
        assert_eq!(buffer.selection(), 9..9);
        buffer.left();
        assert_eq!(buffer.selection(), 1..1);
        buffer.left();
        assert_eq!(buffer.selection(), 0..0);
    }

    #[test]
    fn shift_movement_extends_and_reverses_the_selection() {
        let mut buffer = TextBuffer::new("abc");
        buffer.home();
        buffer.right();

        buffer.select_right();
        assert_eq!(buffer.selection(), 1..2);
        assert!(!buffer.selection_reversed());

        buffer.select_left();
        buffer.select_left();
        assert_eq!(buffer.selection(), 0..1);
        assert!(buffer.selection_reversed());

        buffer.select_right();
        buffer.select_right();
        assert_eq!(buffer.selection(), 1..2);
        assert!(!buffer.selection_reversed());
    }

    #[test]
    fn utf8_and_utf16_ranges_convert_across_a_non_bmp_character() {
        let buffer = TextBuffer::new("a💣z");

        assert_eq!(buffer.range_to_utf16(&(1..5)), 1..3);
        assert_eq!(buffer.range_from_utf16(&(1..3)), 1..5);
        assert_eq!(buffer.range_from_utf16(&(2..2)), 5..5);
    }

    #[test]
    fn marked_text_replacement_updates_the_mark_and_selection() {
        let mut buffer = TextBuffer::new("old");

        buffer.replace_and_mark(None, "💡x", Some(2..3));
        assert_eq!(buffer.content(), "💡x");
        assert_eq!(buffer.marked_range(), Some(0..5));
        assert_eq!(buffer.selection(), 4..5);

        buffer.replace_and_mark(None, "é", Some(0..1));
        assert_eq!(buffer.content(), "é");
        assert_eq!(buffer.marked_range(), Some(0..2));
        assert_eq!(buffer.selection(), 0..2);
    }

    #[test]
    fn marked_selection_after_crlf_uses_original_utf16_offsets_with_non_bmp_text() {
        let mut buffer = TextBuffer::new("");

        buffer.replace_and_mark(None, "a\r\n💡b", Some(3..3));

        assert_eq!(buffer.content(), "a 💡b");
        assert_eq!(buffer.marked_range(), Some(0..7));
        assert_eq!(buffer.selection(), 2..2);
    }

    #[test]
    fn marked_selection_clamps_crlf_interior_and_out_of_bounds_offsets() {
        let mut buffer = TextBuffer::new("");

        buffer.replace_and_mark(None, "a\r\n💡b", Some(2..99));

        assert_eq!(buffer.content(), "a 💡b");
        assert_eq!(buffer.selection(), 2..7);
    }

    #[test]
    fn paste_normalization_replaces_crlf_cr_and_lf_with_spaces() {
        let mut buffer = TextBuffer::new("");

        buffer.replace_text(None, "one\r\ntwo\rthree\nfour");

        assert_eq!(buffer.content(), "one two three four");
        assert_eq!(buffer.selection(), 18..18);
    }

    #[test]
    fn commit_returns_the_current_string() {
        let buffer = TextBuffer::new("draft");

        assert_eq!(
            buffer.commit(),
            TextInputEvent::Commit(String::from("draft"))
        );
    }

    #[test]
    fn cancel_returns_no_string_and_keeps_the_draft() {
        let buffer = TextBuffer::new("draft");

        assert_eq!(TextBuffer::cancel(), TextInputEvent::Cancel);
        assert_eq!(buffer.content(), "draft");
    }
}
