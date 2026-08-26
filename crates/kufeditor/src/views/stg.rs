#![allow(
    dead_code,
    reason = "pure STG projection contracts precede their GPUI render and edit consumers"
)]

use std::{ops::Range, sync::Arc};

use gpui::{
    AnyElement, App, Div, ElementId, IntoElement, SharedString, Stateful, UniformList, Window, div,
    prelude::*, px, uniform_list,
};
use kufeditor_workspace::{
    DocumentID, STGEditor, STGEventTarget, STGFloatTarget, STGNumberTarget, STGParameterTarget,
    STGReferenceKind, STGScriptKind, STGScriptTarget, STGText, STGTextTarget, STGValueTarget,
};

use crate::{
    components,
    state::{
        STGEventBlockRange, STGEventVisibility, STGIndexVisibility, STGReferenceCursor,
        STGReferenceVisibility, STGSection,
    },
    theme::Theme,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGProjectionField {
    Row,
    Text(STGTextTarget),
    Number(STGNumberTarget),
    Float(STGFloatTarget),
    Value(STGValueTarget),
    EventDetail(STGEventDetailRow),
    Magic,
    Reserved(&'static str),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGRowCursor {
    Unit(usize),
    Area(usize),
    Variable(usize),
    EventBlock(usize),
    Event(STGEventTarget),
    Footer(usize),
    EventDetail { event: STGEventTarget, row: usize },
}

impl STGRowCursor {
    pub const fn section(self) -> STGSection {
        match self {
            Self::Unit(_) => STGSection::Units,
            Self::Area(_) => STGSection::Areas,
            Self::Variable(_) => STGSection::Variables,
            Self::EventBlock(_) | Self::Event(_) | Self::EventDetail { .. } => STGSection::Events,
            Self::Footer(_) => STGSection::Footer,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct STGProjectionID {
    document: DocumentID,
    section: STGSection,
    cursor: Option<STGRowCursor>,
    field: STGProjectionField,
}

impl STGProjectionID {
    pub const fn field(
        document: DocumentID,
        section: STGSection,
        field: STGProjectionField,
    ) -> Self {
        Self {
            document,
            section,
            cursor: None,
            field,
        }
    }

    pub const fn row(document: DocumentID, cursor: STGRowCursor) -> Self {
        Self {
            document,
            section: cursor.section(),
            cursor: Some(cursor),
            field: STGProjectionField::Row,
        }
    }

    pub const fn document(self) -> DocumentID {
        self.document
    }

    pub const fn section(self) -> STGSection {
        self.section
    }

    pub const fn cursor(self) -> Option<STGRowCursor> {
        self.cursor
    }

    pub const fn field_kind(self) -> STGProjectionField {
        self.field
    }

    pub fn element_key(self, prefix: &str) -> String {
        format!("{prefix}:{self:?}")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum STGFieldState {
    Value,
    InvalidText,
    UnknownChoice,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct STGFieldProjection {
    id: STGProjectionID,
    label: String,
    display_value: String,
    state: STGFieldState,
}

impl STGFieldProjection {
    pub fn text(
        document: DocumentID,
        section: STGSection,
        target: STGTextTarget,
        value: STGText<'_>,
    ) -> Self {
        let (display_value, state) = match value {
            STGText::Decoded(value) => (display_text(value.as_ref()), STGFieldState::Value),
            STGText::Raw(bytes) => (
                format!(
                    "Invalid source text · {} · {}",
                    byte_count(bytes.len()),
                    hex_preview(bytes)
                ),
                STGFieldState::InvalidText,
            ),
        };
        Self {
            id: STGProjectionID::field(document, section, STGProjectionField::Text(target)),
            label: target.label().to_owned(),
            display_value,
            state,
        }
    }

    pub fn number(
        document: DocumentID,
        section: STGSection,
        target: STGNumberTarget,
        raw_value: i64,
        editor: Option<STGEditor>,
    ) -> Self {
        let (display_value, state) = match editor {
            Some(STGEditor::Choice { choices }) => choices
                .iter()
                .find(|choice| choice.value == raw_value)
                .map_or_else(
                    || {
                        (
                            format!("Unknown ({raw_value})"),
                            STGFieldState::UnknownChoice,
                        )
                    },
                    |choice| {
                        (
                            format!("{} ({raw_value})", choice.label),
                            STGFieldState::Value,
                        )
                    },
                ),
            Some(STGEditor::Number { .. }) | None => (raw_value.to_string(), STGFieldState::Value),
        };
        Self {
            id: STGProjectionID::field(document, section, STGProjectionField::Number(target)),
            label: target.label().to_owned(),
            display_value,
            state,
        }
    }

    pub fn float(
        document: DocumentID,
        section: STGSection,
        target: STGFloatTarget,
        bits: u32,
    ) -> Self {
        let value = f32::from_bits(bits);
        let display_value = if value.is_finite() {
            value.to_string()
        } else {
            format!("{value} · bits 0x{bits:08X}")
        };
        Self {
            id: STGProjectionID::field(document, section, STGProjectionField::Float(target)),
            label: target.label().to_owned(),
            display_value,
            state: STGFieldState::Value,
        }
    }

    pub fn value(
        document: DocumentID,
        section: STGSection,
        target: STGValueTarget,
        label: impl Into<String>,
        display_value: impl Into<String>,
        state: STGFieldState,
    ) -> Self {
        Self {
            id: STGProjectionID::field(document, section, STGProjectionField::Value(target)),
            label: label.into(),
            display_value: display_value.into(),
            state,
        }
    }

    pub fn read_only(
        document: DocumentID,
        section: STGSection,
        field: STGProjectionField,
        label: impl Into<String>,
        display_value: impl Into<String>,
    ) -> Self {
        Self {
            id: STGProjectionID::field(document, section, field),
            label: label.into(),
            display_value: display_value.into(),
            state: STGFieldState::Value,
        }
    }

    pub fn error(
        document: DocumentID,
        section: STGSection,
        field: STGProjectionField,
        label: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self {
            id: STGProjectionID::field(document, section, field),
            label: label.into(),
            display_value: error.into(),
            state: STGFieldState::Error,
        }
    }

    pub const fn id(&self) -> STGProjectionID {
        self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn display_value(&self) -> &str {
        &self.display_value
    }

    pub const fn state(&self) -> STGFieldState {
        self.state
    }
}

fn display_text(value: &str) -> String {
    if value.is_empty() {
        "—".to_owned()
    } else {
        value.to_owned()
    }
}

fn byte_count(count: usize) -> String {
    let unit = if count == 1 { "byte" } else { "bytes" };
    format!("{count} {unit}")
}

fn hex_preview(bytes: &[u8]) -> String {
    const PREVIEW_BYTES: usize = 8;
    let mut value = bytes
        .iter()
        .take(PREVIEW_BYTES)
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ");
    if bytes.len() > PREVIEW_BYTES {
        value.push_str(" …");
    }
    format!("0x{value}")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum STGCatalogAvailability {
    Missing,
    Loading,
    Failed(String),
    Ready,
}

impl STGCatalogAvailability {
    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Missing => {
                Some("Crusaders names are not configured; raw values remain available.")
            }
            Self::Loading => Some("Loading Crusaders names; raw values remain available."),
            Self::Failed(error) => Some(error),
            Self::Ready => None,
        }
    }
}

pub fn render_editor(
    theme: &Theme,
    rail: Vec<AnyElement>,
    catalog_status: Option<AnyElement>,
    content: AnyElement,
) -> Div {
    div().size_full().flex().min_h_0().child(
        div()
            .id("stg-editor")
            .debug_selector(|| "stg-editor".to_owned())
            .size_full()
            .flex()
            .min_h_0()
            .child(
                div()
                    .id("stg-section-rail")
                    .debug_selector(|| "stg-section-rail".to_owned())
                    .flex()
                    .flex_col()
                    .flex_none()
                    .w(px(196.0))
                    .min_h_0()
                    .p(px(10.0))
                    .gap(px(7.0))
                    .bg(theme.surface)
                    .border_r_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .px(px(8.0))
                            .pb(px(5.0))
                            .text_size(px(11.0))
                            .text_color(theme.text_dim)
                            .child("STG FILE"),
                    )
                    .children(rail)
                    .child(div().flex_1())
                    .child(
                        div()
                            .px(px(8.0))
                            .py(px(6.0))
                            .rounded_md()
                            .border_1()
                            .border_color(theme.border)
                            .text_size(px(10.0))
                            .text_color(theme.text_dim)
                            .child("STRUCTURED VIEW"),
                    ),
            )
            .child(
                div()
                    .id("stg-editor-content")
                    .debug_selector(|| "stg-editor-content".to_owned())
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .children(catalog_status)
                    .child(div().flex_1().min_h_0().overflow_hidden().child(content)),
            ),
    )
}

pub fn section_rail_item(
    theme: &Theme,
    id: impl Into<ElementId>,
    label: impl Into<String>,
    selected: bool,
) -> Stateful<Div> {
    let hover = theme.raised;
    let accent = theme.accent;
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(8.0))
        .min_h(px(40.0))
        .px(px(10.0))
        .py(px(6.0))
        .rounded_md()
        .border_1()
        .border_color(if selected {
            theme.accent
        } else {
            theme.surface
        })
        .bg(if selected {
            theme.accent_dim
        } else {
            theme.surface
        })
        .text_color(if selected { theme.text } else { theme.text_dim })
        .cursor_pointer()
        .hover(move |style| style.bg(hover).text_color(accent))
        .active(move |style| style.border_color(accent))
        .child(
            div()
                .w(px(12.0))
                .text_color(if selected {
                    theme.accent
                } else {
                    theme.text_dim
                })
                .child(if selected { "◆" } else { "·" }),
        )
        .child(label.into())
        .children(selected.then(|| {
            div()
                .id("stg-section-active-marker")
                .debug_selector(|| "stg-section-active-marker".to_owned())
                .ml_auto()
                .text_size(px(9.0))
                .text_color(theme.accent)
                .child("ACTIVE")
        }))
}

pub fn section_header(
    theme: &Theme,
    title: &'static str,
    subtitle: impl Into<String>,
) -> Stateful<Div> {
    div()
        .id("stg-section-header")
        .debug_selector(|| "stg-section-header".to_owned())
        .flex_none()
        .px(px(20.0))
        .py(px(13.0))
        .flex()
        .items_center()
        .gap(px(12.0))
        .bg(theme.surface)
        .border_b_1()
        .border_color(theme.border)
        .child(
            div()
                .text_size(px(18.0))
                .text_color(theme.text)
                .child(title),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(theme.text_dim)
                .child(subtitle.into()),
        )
}

pub fn scrolling_section(
    theme: &Theme,
    id: &'static str,
    title: &'static str,
    subtitle: impl Into<String>,
    children: Vec<AnyElement>,
) -> Div {
    div().size_full().child(
        div()
            .id(id)
            .debug_selector(move || id.to_owned())
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .child(section_header(theme, title, subtitle))
            .child(
                div()
                    .id(SharedString::from(format!("stg-scroll:{id}")))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(18.0))
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .children(children),
            ),
    )
}

pub fn split_section(
    theme: &Theme,
    id: &'static str,
    title: &'static str,
    subtitle: impl Into<String>,
    list: AnyElement,
    details: AnyElement,
) -> Div {
    div().size_full().child(
        div()
            .id(id)
            .debug_selector(move || id.to_owned())
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .child(section_header(theme, title, subtitle))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_none()
                            .w(px(320.0))
                            .min_h_0()
                            .bg(theme.surface)
                            .border_r_1()
                            .border_color(theme.border)
                            .child(list),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("stg-detail:{id}")))
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .overflow_hidden()
                            .child(details),
                    ),
            ),
    )
}

pub fn scrolling_details(
    theme: &Theme,
    id: impl Into<ElementId>,
    children: Vec<AnyElement>,
) -> Stateful<Div> {
    div()
        .id(id)
        .size_full()
        .min_h_0()
        .overflow_y_scroll()
        .p(px(18.0))
        .flex()
        .flex_col()
        .gap(px(14.0))
        .bg(theme.background)
        .children(children)
}

pub fn group(theme: &Theme, label: impl Into<String>, fields: Vec<AnyElement>) -> Div {
    components::surface(theme)
        .flex()
        .flex_col()
        .child(
            div()
                .min_h(px(38.0))
                .px(px(13.0))
                .py(px(8.0))
                .flex()
                .items_center()
                .border_b_1()
                .border_color(theme.border)
                .text_size(px(11.0))
                .text_color(theme.accent)
                .child(label.into()),
        )
        .child(
            div()
                .p(px(9.0))
                .flex()
                .flex_col()
                .gap(px(6.0))
                .children(fields),
        )
}

pub fn field_row(theme: &Theme, field: &STGFieldProjection) -> Stateful<Div> {
    let state = field.state();
    div()
        .id(SharedString::from(field.id().element_key("stg-field")))
        .min_h(px(38.0))
        .px(px(10.0))
        .py(px(7.0))
        .flex()
        .items_center()
        .gap(px(12.0))
        .rounded_md()
        .bg(theme.background)
        .border_1()
        .border_color(if matches!(state, STGFieldState::Value) {
            theme.background
        } else {
            theme.accent
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_color(theme.text_dim)
                .child(field.label().to_owned()),
        )
        .children((!matches!(state, STGFieldState::Value)).then(|| {
            div()
                .flex_none()
                .px(px(6.0))
                .py(px(3.0))
                .rounded_md()
                .bg(theme.accent_dim)
                .text_size(px(9.0))
                .text_color(theme.accent)
                .child(match state {
                    STGFieldState::InvalidText => "INVALID TEXT",
                    STGFieldState::UnknownChoice => "UNKNOWN",
                    STGFieldState::Error => "READ ERROR",
                    STGFieldState::Value => "",
                })
        }))
        .child(
            div()
                .flex_none()
                .max_w(px(420.0))
                .truncate()
                .text_color(theme.text)
                .child(field.display_value().to_owned()),
        )
}

pub fn empty_state(theme: &Theme, id: &'static str, message: impl Into<String>) -> Stateful<Div> {
    components::surface(theme)
        .id(id)
        .debug_selector(move || id.to_owned())
        .p(px(18.0))
        .text_color(theme.text_dim)
        .child(message.into())
}

pub fn raw_tail_panel(theme: &Theme, raw: &STGRawTailProjection) -> Stateful<Div> {
    let id = format!("stg-raw-tail-{}", raw.section().label().to_lowercase());
    components::surface(theme)
        .id(SharedString::from(id.clone()))
        .debug_selector(move || id.clone())
        .p(px(18.0))
        .flex()
        .flex_col()
        .gap(px(9.0))
        .border_color(theme.accent)
        .child(
            div()
                .text_size(px(16.0))
                .text_color(theme.text)
                .child(format!("{} is unparsed", raw.section().label())),
        )
        .child(div().text_color(theme.text_dim).child(format!(
            "Parsing first failed in {} at byte {}.",
            raw.region(),
            raw.offset()
        )))
        .child(div().text_color(theme.accent).child(format!(
            "All {} are preserved exactly and will be written without changes.",
            byte_count(raw.bytes())
        )))
}

pub fn master_row(
    theme: &Theme,
    id: impl Into<ElementId>,
    title: impl Into<String>,
    metadata: impl Into<String>,
    selected: bool,
) -> Stateful<Div> {
    let hover = theme.raised;
    div()
        .id(id)
        .min_h(px(58.0))
        .px(px(11.0))
        .py(px(8.0))
        .flex()
        .items_center()
        .gap(px(9.0))
        .border_b_1()
        .border_color(theme.border)
        .bg(if selected {
            theme.accent_dim
        } else {
            theme.surface
        })
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(3.0))
                .child(div().truncate().text_color(theme.text).child(title.into()))
                .child(
                    div()
                        .truncate()
                        .text_size(px(11.0))
                        .text_color(theme.text_dim)
                        .child(metadata.into()),
                ),
        )
        .children(selected.then(|| {
            div()
                .id("stg-row-inspecting-marker")
                .debug_selector(|| "stg-row-inspecting-marker".to_owned())
                .flex_none()
                .text_size(px(9.0))
                .text_color(theme.accent)
                .child("INSPECTING")
        }))
}

pub fn uniform_stg_rows<R>(
    id: impl Into<ElementId>,
    rows: STGVirtualRows,
    render: impl 'static + Fn(STGRowLocation, &mut Window, &mut App) -> R,
) -> UniformList
where
    R: IntoElement,
{
    uniform_list(id, rows.len(), move |requested, window, cx| {
        rows.locations(requested)
            .into_iter()
            .map(|location| render(location, window, cx))
            .collect()
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum STGIndexRowsData {
    Range { count: usize },
    Filtered(Arc<[usize]>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct STGIndexRows {
    data: STGIndexRowsData,
}

impl STGIndexRows {
    pub const fn range(count: usize) -> Self {
        Self {
            data: STGIndexRowsData::Range { count },
        }
    }

    pub fn filtered(indices: Vec<usize>) -> Self {
        Self {
            data: STGIndexRowsData::Filtered(Arc::from(indices.into_boxed_slice())),
        }
    }

    pub fn len(&self) -> usize {
        match &self.data {
            STGIndexRowsData::Range { count } => *count,
            STGIndexRowsData::Filtered(indices) => indices.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn source_index(&self, position: usize) -> Option<usize> {
        match &self.data {
            STGIndexRowsData::Range { count } => (position < *count).then_some(position),
            STGIndexRowsData::Filtered(indices) => indices.get(position).copied(),
        }
    }

    pub fn position_of(&self, source_index: usize) -> Option<usize> {
        match &self.data {
            STGIndexRowsData::Range { count } => (source_index < *count).then_some(source_index),
            STGIndexRowsData::Filtered(indices) => indices
                .iter()
                .position(|candidate| *candidate == source_index),
        }
    }

    pub fn stored_index_count(&self) -> usize {
        match &self.data {
            STGIndexRowsData::Range { .. } => 0,
            STGIndexRowsData::Filtered(indices) => indices.len(),
        }
    }

    pub fn visibility(&self) -> STGIndexVisibility<'_> {
        match &self.data {
            STGIndexRowsData::Range { count } => STGIndexVisibility::Range(0..*count),
            STGIndexRowsData::Filtered(indices) => STGIndexVisibility::Sparse(indices),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct STGEventBlockProjection {
    block: usize,
    header: u32,
    event_count: usize,
    flat_start: usize,
}

impl STGEventBlockProjection {
    pub const fn new(block: usize, header: u32, event_count: usize) -> Self {
        Self {
            block,
            header,
            event_count,
            flat_start: 0,
        }
    }

    pub const fn block(self) -> usize {
        self.block
    }

    pub const fn header(self) -> u32 {
        self.header
    }

    pub const fn event_count(self) -> usize {
        self.event_count
    }

    pub const fn flat_start(self) -> usize {
        self.flat_start
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum STGEventRowsData {
    Blocks(Arc<[STGEventBlockProjection]>),
    Filtered(Arc<[STGEventTarget]>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct STGEventRows {
    data: STGEventRowsData,
    count: usize,
    block_ranges: Arc<[STGEventBlockRange]>,
}

pub struct STGEventTargets<'a> {
    data: &'a STGEventRowsData,
    block: usize,
    event: usize,
    filtered: usize,
}

impl Iterator for STGEventTargets<'_> {
    type Item = STGEventTarget;

    fn next(&mut self) -> Option<Self::Item> {
        match self.data {
            STGEventRowsData::Filtered(targets) => {
                let target = targets.get(self.filtered).copied()?;
                self.filtered += 1;
                Some(target)
            }
            STGEventRowsData::Blocks(blocks) => loop {
                let block = blocks.get(self.block)?;
                if self.event < block.event_count {
                    let target = STGEventTarget {
                        block: block.block,
                        event: self.event,
                    };
                    self.event += 1;
                    return Some(target);
                }
                self.block += 1;
                self.event = 0;
            },
        }
    }
}

impl STGEventRows {
    pub fn from_blocks(mut blocks: Vec<STGEventBlockProjection>) -> Self {
        let mut count = 0_usize;
        for block in &mut blocks {
            block.flat_start = count;
            count = count.saturating_add(block.event_count);
        }
        let block_ranges = blocks
            .iter()
            .map(|block| STGEventBlockRange::new(block.block, block.event_count))
            .collect::<Vec<_>>();
        Self {
            data: STGEventRowsData::Blocks(Arc::from(blocks.into_boxed_slice())),
            count,
            block_ranges: Arc::from(block_ranges.into_boxed_slice()),
        }
    }

    pub fn filtered(targets: Vec<STGEventTarget>) -> Self {
        let count = targets.len();
        Self {
            data: STGEventRowsData::Filtered(Arc::from(targets.into_boxed_slice())),
            count,
            block_ranges: Arc::from(Vec::new().into_boxed_slice()),
        }
    }

    pub const fn len(&self) -> usize {
        self.count
    }

    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub const fn targets(&self) -> STGEventTargets<'_> {
        STGEventTargets {
            data: &self.data,
            block: 0,
            event: 0,
            filtered: 0,
        }
    }

    pub fn target(&self, position: usize) -> Option<STGEventTarget> {
        if position >= self.count {
            return None;
        }
        match &self.data {
            STGEventRowsData::Filtered(targets) => targets.get(position).copied(),
            STGEventRowsData::Blocks(blocks) => {
                let block = blocks.get(blocks.partition_point(|block| {
                    block.flat_start.saturating_add(block.event_count) <= position
                }))?;
                Some(STGEventTarget {
                    block: block.block,
                    event: position.checked_sub(block.flat_start)?,
                })
            }
        }
    }

    pub fn position_of(&self, target: STGEventTarget) -> Option<usize> {
        match &self.data {
            STGEventRowsData::Filtered(targets) => {
                targets.iter().position(|candidate| *candidate == target)
            }
            STGEventRowsData::Blocks(blocks) => {
                let block = blocks
                    .binary_search_by_key(&target.block, |block| block.block)
                    .ok()
                    .and_then(|position| blocks.get(position))?;
                (target.event < block.event_count)
                    .then(|| block.flat_start.saturating_add(target.event))
            }
        }
    }

    pub fn stored_block_count(&self) -> usize {
        match &self.data {
            STGEventRowsData::Blocks(blocks) => blocks.len(),
            STGEventRowsData::Filtered(_) => 0,
        }
    }

    pub fn stored_target_count(&self) -> usize {
        match &self.data {
            STGEventRowsData::Blocks(_) => 0,
            STGEventRowsData::Filtered(targets) => targets.len(),
        }
    }

    pub fn blocks(&self) -> Option<&[STGEventBlockProjection]> {
        match &self.data {
            STGEventRowsData::Blocks(blocks) => Some(blocks),
            STGEventRowsData::Filtered(_) => None,
        }
    }

    pub fn filtered_targets(&self) -> Option<&[STGEventTarget]> {
        match &self.data {
            STGEventRowsData::Blocks(_) => None,
            STGEventRowsData::Filtered(targets) => Some(targets),
        }
    }

    pub fn visibility(&self) -> STGEventVisibility<'_> {
        match &self.data {
            STGEventRowsData::Blocks(_) => STGEventVisibility::Blocks(&self.block_ranges),
            STGEventRowsData::Filtered(targets) => STGEventVisibility::Filtered(targets),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum STGReferenceRowsData {
    Indices(STGIndexRows),
    Events(STGEventRows),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct STGReferenceRows {
    kind: STGReferenceKind,
    data: STGReferenceRowsData,
}

impl STGReferenceRows {
    pub const fn from_rows(kind: STGReferenceKind, rows: STGIndexRows) -> Self {
        Self {
            kind,
            data: STGReferenceRowsData::Indices(rows),
        }
    }

    pub const fn from_event_rows(kind: STGReferenceKind, rows: STGEventRows) -> Self {
        Self {
            kind,
            data: STGReferenceRowsData::Events(rows),
        }
    }

    pub const fn range(kind: STGReferenceKind, count: usize) -> Self {
        Self {
            kind,
            data: STGReferenceRowsData::Indices(STGIndexRows::range(count)),
        }
    }

    pub fn filtered(kind: STGReferenceKind, indices: Vec<usize>) -> Self {
        Self {
            kind,
            data: STGReferenceRowsData::Indices(STGIndexRows::filtered(indices)),
        }
    }

    pub const fn kind(&self) -> STGReferenceKind {
        self.kind
    }

    pub fn len(&self) -> usize {
        match &self.data {
            STGReferenceRowsData::Indices(rows) => rows.len(),
            STGReferenceRowsData::Events(rows) => rows.len(),
        }
    }

    pub fn source_index(&self, position: usize) -> Option<usize> {
        match &self.data {
            STGReferenceRowsData::Indices(rows) => rows.source_index(position),
            STGReferenceRowsData::Events(_) => None,
        }
    }

    pub fn cursor(&self, position: usize) -> Option<STGReferenceCursor> {
        match &self.data {
            STGReferenceRowsData::Indices(rows) => {
                rows.source_index(position).map(STGReferenceCursor::Index)
            }
            STGReferenceRowsData::Events(rows) => {
                rows.target(position).map(STGReferenceCursor::Event)
            }
        }
    }

    pub fn stored_index_count(&self) -> usize {
        match &self.data {
            STGReferenceRowsData::Indices(rows) => rows.stored_index_count(),
            STGReferenceRowsData::Events(_) => 0,
        }
    }

    pub fn visibility(&self) -> STGReferenceVisibility<'_> {
        match &self.data {
            STGReferenceRowsData::Indices(rows) => {
                STGReferenceVisibility::Indices(rows.visibility())
            }
            STGReferenceRowsData::Events(rows) => STGReferenceVisibility::Events(rows.visibility()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum STGTailProjection {
    Parsed {
        suffix_bytes: usize,
    },
    Raw {
        bytes: usize,
        region: String,
        offset: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum STGCollectionProjection {
    Units,
    Areas,
    Variables,
    Events,
    Footer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct STGDocumentProjection {
    units: STGIndexRows,
    areas: Option<STGIndexRows>,
    variables: Option<STGIndexRows>,
    events: Option<STGEventRows>,
    footer: Option<STGIndexRows>,
    tail: STGTailProjection,
}

impl STGDocumentProjection {
    pub fn new(
        units: usize,
        areas: Option<usize>,
        variables: Option<usize>,
        events: Option<Vec<STGEventBlockProjection>>,
        footer: Option<usize>,
        tail: STGTailProjection,
    ) -> Self {
        Self {
            units: STGIndexRows::range(units),
            areas: areas.map(STGIndexRows::range),
            variables: variables.map(STGIndexRows::range),
            events: events.map(STGEventRows::from_blocks),
            footer: footer.map(STGIndexRows::range),
            tail,
        }
    }

    pub const fn units(&self) -> &STGIndexRows {
        &self.units
    }

    pub const fn areas(&self) -> Option<&STGIndexRows> {
        self.areas.as_ref()
    }

    pub const fn variables(&self) -> Option<&STGIndexRows> {
        self.variables.as_ref()
    }

    pub const fn events(&self) -> Option<&STGEventRows> {
        self.events.as_ref()
    }

    pub const fn footer(&self) -> Option<&STGIndexRows> {
        self.footer.as_ref()
    }

    pub const fn tail(&self) -> &STGTailProjection {
        &self.tail
    }

    pub fn section(&self, section: STGCollectionProjection) -> Option<usize> {
        match section {
            STGCollectionProjection::Units => Some(self.units.len()),
            STGCollectionProjection::Areas => self.areas.as_ref().map(STGIndexRows::len),
            STGCollectionProjection::Variables => self.variables.as_ref().map(STGIndexRows::len),
            STGCollectionProjection::Events => self.events.as_ref().map(STGEventRows::len),
            STGCollectionProjection::Footer => self.footer.as_ref().map(STGIndexRows::len),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct STGSearchRecord<'a> {
    source_index: usize,
    source_text: Option<&'a str>,
    derived_text: Option<&'a str>,
    raw_id: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct STGSearchQuery {
    folded: String,
}

impl STGSearchQuery {
    pub(crate) fn new(query: &str) -> Self {
        Self {
            folded: query.trim().to_lowercase(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.folded.is_empty()
    }

    fn folded(&self) -> &str {
        &self.folded
    }
}

impl<'a> STGSearchRecord<'a> {
    pub const fn new(
        source_index: usize,
        source_text: Option<&'a str>,
        derived_text: Option<&'a str>,
        raw_id: Option<i64>,
    ) -> Self {
        Self {
            source_index,
            source_text,
            derived_text,
            raw_id,
        }
    }

    pub(crate) fn matches(self, query: &STGSearchQuery) -> bool {
        if query.is_empty() {
            return true;
        }
        self.source_text
            .is_some_and(|text| contains_folded(text, query.folded()))
            || self
                .derived_text
                .is_some_and(|text| contains_folded(text, query.folded()))
            || self
                .raw_id
                .is_some_and(|raw_id| raw_id.to_string().contains(query.folded()))
            || self.source_index.to_string().contains(query.folded())
    }

    pub const fn draft_seed(self) -> Option<&'a str> {
        self.source_text
    }

    pub const fn derived_text(self) -> Option<&'a str> {
        self.derived_text
    }
}

fn contains_folded(value: &str, folded_query: &str) -> bool {
    value.to_lowercase().contains(folded_query)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct STGCatalogTextSuggestion<'a> {
    source_text: &'a str,
    display_text: &'a str,
}

impl<'a> STGCatalogTextSuggestion<'a> {
    pub const fn new(source_text: &'a str, display_text: &'a str) -> Self {
        Self {
            source_text,
            display_text,
        }
    }

    pub const fn source_preview(self) -> &'a str {
        self.source_text
    }

    pub const fn display_text(self) -> &'a str {
        self.display_text
    }

    pub fn apply_to(self, draft: &mut String) {
        draft.clear();
        draft.push_str(self.source_text);
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGEventDetailField {
    Description,
    ID,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGEventDetailRow {
    EventField(STGEventDetailField),
    ScriptHeader(STGScriptTarget),
    Parameter(STGParameterTarget),
    AddScript(STGScriptKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct STGScriptRowGroup {
    target: STGScriptTarget,
    start: usize,
    parameter_count: usize,
}

impl STGScriptRowGroup {
    fn row(self, position: usize) -> Option<STGEventDetailRow> {
        if position == self.start {
            return Some(STGEventDetailRow::ScriptHeader(self.target));
        }
        let parameter = position.checked_sub(self.start.checked_add(1)?)?;
        (parameter < self.parameter_count).then_some(STGEventDetailRow::Parameter(
            STGParameterTarget {
                script: self.target,
                parameter,
            },
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct STGEventDetailRows {
    groups: Arc<[STGScriptRowGroup]>,
    condition_add: usize,
    action_add: usize,
    count: usize,
}

impl STGEventDetailRows {
    pub fn from_parameter_counts(
        event: STGEventTarget,
        condition_parameter_counts: &[usize],
        action_parameter_counts: &[usize],
    ) -> Option<Self> {
        let mut groups = Vec::with_capacity(
            condition_parameter_counts
                .len()
                .checked_add(action_parameter_counts.len())?,
        );
        let mut next = 2_usize;
        append_script_groups(
            &mut groups,
            event,
            STGScriptKind::Condition,
            condition_parameter_counts,
            &mut next,
        )?;
        let condition_add = next;
        next = next.checked_add(1)?;
        append_script_groups(
            &mut groups,
            event,
            STGScriptKind::Action,
            action_parameter_counts,
            &mut next,
        )?;
        let action_add = next;
        let count = next.checked_add(1)?;
        Some(Self {
            groups: Arc::from(groups.into_boxed_slice()),
            condition_add,
            action_add,
            count,
        })
    }

    pub const fn len(&self) -> usize {
        self.count
    }

    pub fn stored_script_group_count(&self) -> usize {
        self.groups.len()
    }

    pub fn row(&self, position: usize) -> Option<STGEventDetailRow> {
        match position {
            0 => {
                return Some(STGEventDetailRow::EventField(
                    STGEventDetailField::Description,
                ));
            }
            1 => {
                return Some(STGEventDetailRow::EventField(STGEventDetailField::ID));
            }
            _ if position == self.condition_add => {
                return Some(STGEventDetailRow::AddScript(STGScriptKind::Condition));
            }
            _ if position == self.action_add => {
                return Some(STGEventDetailRow::AddScript(STGScriptKind::Action));
            }
            _ if position >= self.count => return None,
            _ => {}
        }
        self.groups
            .partition_point(|group| group.start <= position)
            .checked_sub(1)
            .and_then(|group| self.groups.get(group))
            .and_then(|group| group.row(position))
    }
}

fn append_script_groups(
    groups: &mut Vec<STGScriptRowGroup>,
    event: STGEventTarget,
    kind: STGScriptKind,
    parameter_counts: &[usize],
    next: &mut usize,
) -> Option<()> {
    for (script, parameter_count) in parameter_counts.iter().copied().enumerate() {
        groups.push(STGScriptRowGroup {
            target: STGScriptTarget {
                block: event.block,
                event: event.event,
                kind,
                script,
            },
            start: *next,
            parameter_count,
        });
        *next = next.checked_add(parameter_count.checked_add(1)?)?;
    }
    Some(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum STGVirtualRowsData {
    Indices(STGIndexRows),
    Events(STGEventMasterRows),
    EventDetails {
        event: STGEventTarget,
        rows: STGEventDetailRows,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct STGEventMasterBlock {
    block: usize,
    event_count: usize,
    virtual_start: usize,
}

impl STGEventMasterBlock {
    const fn virtual_count(self) -> usize {
        if self.event_count == 0 {
            1
        } else {
            self.event_count
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct STGEventMasterRows {
    source: STGEventRows,
    blocks: Arc<[STGEventMasterBlock]>,
    count: usize,
}

impl STGEventMasterRows {
    fn new(source: STGEventRows) -> Self {
        let mut count = 0_usize;
        let blocks = source.blocks().map_or_else(Vec::new, |blocks| {
            blocks
                .iter()
                .map(|block| {
                    let projection = STGEventMasterBlock {
                        block: block.block(),
                        event_count: block.event_count(),
                        virtual_start: count,
                    };
                    count = count.saturating_add(projection.virtual_count());
                    projection
                })
                .collect()
        });
        if blocks.is_empty() {
            count = source.len();
        }
        Self {
            source,
            blocks: Arc::from(blocks.into_boxed_slice()),
            count,
        }
    }

    fn cursor(&self, position: usize) -> Option<STGRowCursor> {
        if position >= self.count {
            return None;
        }
        if self.blocks.is_empty() {
            return self.source.target(position).map(STGRowCursor::Event);
        }
        let block = self.blocks.get(self.blocks.partition_point(|block| {
            block.virtual_start.saturating_add(block.virtual_count()) <= position
        }))?;
        if block.event_count == 0 {
            return Some(STGRowCursor::EventBlock(block.block));
        }
        Some(STGRowCursor::Event(STGEventTarget {
            block: block.block,
            event: position.checked_sub(block.virtual_start)?,
        }))
    }

    fn position_of(&self, cursor: STGRowCursor) -> Option<usize> {
        if self.blocks.is_empty() {
            let STGRowCursor::Event(target) = cursor else {
                return None;
            };
            return self.source.position_of(target);
        }
        match cursor {
            STGRowCursor::EventBlock(block) => self
                .blocks
                .binary_search_by_key(&block, |candidate| candidate.block)
                .ok()
                .and_then(|position| self.blocks.get(position))
                .filter(|block| block.event_count == 0)
                .map(|block| block.virtual_start),
            STGRowCursor::Event(target) => self
                .blocks
                .binary_search_by_key(&target.block, |block| block.block)
                .ok()
                .and_then(|position| self.blocks.get(position))
                .filter(|block| target.event < block.event_count)
                .map(|block| block.virtual_start.saturating_add(target.event)),
            STGRowCursor::Unit(_)
            | STGRowCursor::Area(_)
            | STGRowCursor::Variable(_)
            | STGRowCursor::Footer(_)
            | STGRowCursor::EventDetail { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGVirtualRowKind {
    Unit,
    Area,
    Variable,
    Event,
    Footer,
    EventDetail,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct STGVirtualRows {
    document: DocumentID,
    kind: STGVirtualRowKind,
    data: STGVirtualRowsData,
}

impl STGVirtualRows {
    pub const fn units(document: DocumentID, rows: STGIndexRows) -> Self {
        Self::indices(document, STGVirtualRowKind::Unit, rows)
    }

    pub const fn areas(document: DocumentID, rows: STGIndexRows) -> Self {
        Self::indices(document, STGVirtualRowKind::Area, rows)
    }

    pub const fn variables(document: DocumentID, rows: STGIndexRows) -> Self {
        Self::indices(document, STGVirtualRowKind::Variable, rows)
    }

    pub const fn footer(document: DocumentID, rows: STGIndexRows) -> Self {
        Self::indices(document, STGVirtualRowKind::Footer, rows)
    }

    pub fn events(document: DocumentID, rows: STGEventRows) -> Self {
        Self {
            document,
            kind: STGVirtualRowKind::Event,
            data: STGVirtualRowsData::Events(STGEventMasterRows::new(rows)),
        }
    }

    pub const fn event_details(
        document: DocumentID,
        event: STGEventTarget,
        rows: STGEventDetailRows,
    ) -> Self {
        Self {
            document,
            kind: STGVirtualRowKind::EventDetail,
            data: STGVirtualRowsData::EventDetails { event, rows },
        }
    }

    const fn indices(document: DocumentID, kind: STGVirtualRowKind, rows: STGIndexRows) -> Self {
        Self {
            document,
            kind,
            data: STGVirtualRowsData::Indices(rows),
        }
    }

    pub const fn document(&self) -> DocumentID {
        self.document
    }

    pub const fn kind(&self) -> STGVirtualRowKind {
        self.kind
    }

    pub fn len(&self) -> usize {
        match &self.data {
            STGVirtualRowsData::Indices(rows) => rows.len(),
            STGVirtualRowsData::Events(rows) => rows.count,
            STGVirtualRowsData::EventDetails { rows, .. } => rows.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn cursor(&self, position: usize) -> Option<STGRowCursor> {
        match (&self.data, self.kind) {
            (STGVirtualRowsData::Indices(rows), STGVirtualRowKind::Unit) => {
                rows.source_index(position).map(STGRowCursor::Unit)
            }
            (STGVirtualRowsData::Indices(rows), STGVirtualRowKind::Area) => {
                rows.source_index(position).map(STGRowCursor::Area)
            }
            (STGVirtualRowsData::Indices(rows), STGVirtualRowKind::Variable) => {
                rows.source_index(position).map(STGRowCursor::Variable)
            }
            (STGVirtualRowsData::Indices(rows), STGVirtualRowKind::Footer) => {
                rows.source_index(position).map(STGRowCursor::Footer)
            }
            (STGVirtualRowsData::Events(rows), STGVirtualRowKind::Event) => rows.cursor(position),
            (STGVirtualRowsData::EventDetails { event, rows }, STGVirtualRowKind::EventDetail) => {
                (position < rows.len()).then_some(STGRowCursor::EventDetail {
                    event: *event,
                    row: position,
                })
            }
            (
                STGVirtualRowsData::Indices(_),
                STGVirtualRowKind::Event | STGVirtualRowKind::EventDetail,
            )
            | (
                STGVirtualRowsData::Events(_),
                STGVirtualRowKind::Unit
                | STGVirtualRowKind::Area
                | STGVirtualRowKind::Variable
                | STGVirtualRowKind::Footer
                | STGVirtualRowKind::EventDetail,
            )
            | (
                STGVirtualRowsData::EventDetails { .. },
                STGVirtualRowKind::Unit
                | STGVirtualRowKind::Area
                | STGVirtualRowKind::Variable
                | STGVirtualRowKind::Event
                | STGVirtualRowKind::Footer,
            ) => None,
        }
    }

    pub fn position_of(&self, cursor: STGRowCursor) -> Option<usize> {
        match (&self.data, cursor) {
            (STGVirtualRowsData::Indices(rows), STGRowCursor::Unit(index))
                if self.kind == STGVirtualRowKind::Unit =>
            {
                rows.position_of(index)
            }
            (STGVirtualRowsData::Indices(rows), STGRowCursor::Area(index))
                if self.kind == STGVirtualRowKind::Area =>
            {
                rows.position_of(index)
            }
            (STGVirtualRowsData::Indices(rows), STGRowCursor::Variable(index))
                if self.kind == STGVirtualRowKind::Variable =>
            {
                rows.position_of(index)
            }
            (STGVirtualRowsData::Indices(rows), STGRowCursor::Footer(index))
                if self.kind == STGVirtualRowKind::Footer =>
            {
                rows.position_of(index)
            }
            (STGVirtualRowsData::Events(rows), cursor) if self.kind == STGVirtualRowKind::Event => {
                rows.position_of(cursor)
            }
            (
                STGVirtualRowsData::EventDetails { event, rows },
                STGRowCursor::EventDetail {
                    event: cursor_event,
                    row,
                },
            ) if self.kind == STGVirtualRowKind::EventDetail
                && *event == cursor_event
                && row < rows.len() =>
            {
                Some(row)
            }
            _ => None,
        }
    }

    pub fn event_detail_row(&self, position: usize) -> Option<STGEventDetailRow> {
        match &self.data {
            STGVirtualRowsData::EventDetails { rows, .. } => rows.row(position),
            STGVirtualRowsData::Indices(_) | STGVirtualRowsData::Events(_) => None,
        }
    }

    pub fn locations(&self, requested: Range<usize>) -> Vec<STGRowLocation> {
        bounded_range(requested, self.len())
            .filter_map(|position| {
                let cursor = self.cursor(position)?;
                Some(STGRowLocation {
                    id: STGProjectionID::row(self.document, cursor),
                    cursor,
                    position,
                })
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct STGRowLocation {
    id: STGProjectionID,
    cursor: STGRowCursor,
    position: usize,
}

impl STGRowLocation {
    pub const fn id(self) -> STGProjectionID {
        self.id
    }

    pub const fn cursor(self) -> STGRowCursor {
        self.cursor
    }

    pub const fn position(self) -> usize {
        self.position
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct STGRawTailProjection {
    document: DocumentID,
    section: STGSection,
    bytes: usize,
    region: String,
    offset: usize,
}

impl STGRawTailProjection {
    pub const fn document(&self) -> DocumentID {
        self.document
    }

    pub const fn section(&self) -> STGSection {
        self.section
    }

    pub const fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn region(&self) -> &str {
        &self.region
    }

    pub const fn offset(&self) -> usize {
        self.offset
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum STGSectionProjectionData {
    Rows(STGVirtualRows),
    Empty,
    RawTail(STGRawTailProjection),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct STGSectionProjection {
    document: DocumentID,
    section: STGSection,
    data: STGSectionProjectionData,
}

impl STGSectionProjection {
    pub fn from_rows(document: DocumentID, section: STGSection, rows: STGVirtualRows) -> Self {
        debug_assert_eq!(document, rows.document());
        let data = if rows.is_empty() {
            STGSectionProjectionData::Empty
        } else {
            STGSectionProjectionData::Rows(rows)
        };
        Self {
            document,
            section,
            data,
        }
    }

    pub fn from_raw_tail(
        document: DocumentID,
        section: STGSection,
        bytes: usize,
        region: impl Into<String>,
        offset: usize,
    ) -> Self {
        Self {
            document,
            section,
            data: STGSectionProjectionData::RawTail(STGRawTailProjection {
                document,
                section,
                bytes,
                region: region.into(),
                offset,
            }),
        }
    }

    pub const fn document(&self) -> DocumentID {
        self.document
    }

    pub const fn section(&self) -> STGSection {
        self.section
    }

    pub const fn is_empty(&self) -> bool {
        matches!(self.data, STGSectionProjectionData::Empty)
    }

    pub const fn is_raw_tail(&self) -> bool {
        matches!(self.data, STGSectionProjectionData::RawTail(_))
    }

    pub fn rows(&self) -> Option<&STGVirtualRows> {
        match &self.data {
            STGSectionProjectionData::Rows(rows) => Some(rows),
            STGSectionProjectionData::Empty | STGSectionProjectionData::RawTail(_) => None,
        }
    }

    pub fn raw_tail(&self) -> Option<&STGRawTailProjection> {
        match &self.data {
            STGSectionProjectionData::RawTail(raw) => Some(raw),
            STGSectionProjectionData::Rows(_) | STGSectionProjectionData::Empty => None,
        }
    }

    pub fn row_bindings(&self, requested: Range<usize>) -> Vec<STGRowLocation> {
        self.rows()
            .map_or_else(Vec::new, |rows| rows.locations(requested))
    }
}

fn bounded_range(requested: Range<usize>, len: usize) -> Range<usize> {
    let start = requested.start.min(len);
    let end = requested.end.min(len).max(start);
    start..end
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, path::PathBuf};

    use gpui::{
        Context, IntoElement, Render, TestAppContext, Window, div, point, prelude::*, px, size,
    };
    use kufeditor_workspace::{
        Document, DocumentID, STGDocument, STGEditor, STGEventTarget, STGHeaderTextField,
        STGNumberTarget, STGParameterTarget, STGReferenceKind, STGScriptKind, STGScriptTarget,
        STGText, STGTextTarget, STGUnitField, Workspace,
    };

    use super::{
        STGCatalogAvailability, STGCatalogTextSuggestion, STGCollectionProjection,
        STGDocumentProjection, STGEventBlockProjection, STGEventDetailField, STGEventDetailRow,
        STGEventDetailRows, STGEventRows, STGFieldProjection, STGFieldState, STGIndexRows,
        STGReferenceRows, STGRowCursor, STGSearchQuery, STGSearchRecord, STGSectionProjection,
        STGTailProjection, STGVirtualRows, master_row, render_editor, section_rail_item,
    };
    use crate::state::{STGReferenceCursor, STGSection};
    use crate::theme::Theme;

    struct STGSelectionMarkers;

    impl Render for STGSelectionMarkers {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .child(section_rail_item(
                    &Theme::default(),
                    "test-section",
                    "Units · 1",
                    true,
                ))
                .child(master_row(
                    &Theme::default(),
                    "test-row",
                    "Unit 001",
                    "UCD 0 · ID 4",
                    true,
                ))
        }
    }

    fn event(block: usize, event: usize) -> STGEventTarget {
        STGEventTarget { block, event }
    }

    fn script(block: usize, event: usize, kind: STGScriptKind, script: usize) -> STGScriptTarget {
        STGScriptTarget {
            block,
            event,
            kind,
            script,
        }
    }

    fn document_id() -> DocumentID {
        let mut bytes = 1_001_u32.to_le_bytes().to_vec();
        bytes.resize(bytes.len() + 620, 0);
        for _ in 0..5 {
            bytes.extend_from_slice(&0_u32.to_le_bytes());
        }
        let mut workspace = Workspace::new();
        workspace.open_loaded(
            PathBuf::from("projection.stg"),
            Document::STG(STGDocument::parse(bytes).unwrap()),
        )
    }

    #[test]
    fn stg_projection_searches_source_display_raw_id_and_source_index() {
        let record = STGSearchRecord::new(
            17,
            Some("PaladinInternal"),
            Some("Holy Guard 수호자"),
            Some(4_001),
        );

        assert!(record.matches(&STGSearchQuery::new("paladininternal")));
        assert!(record.matches(&STGSearchQuery::new("  HOLY GUARD  ")));
        assert!(record.matches(&STGSearchQuery::new("수호자")));
        assert!(record.matches(&STGSearchQuery::new("4001")));
        assert!(record.matches(&STGSearchQuery::new("17")));
        assert!(!record.matches(&STGSearchQuery::new("archer")));
        assert!(record.matches(&STGSearchQuery::new("")));
        assert_eq!(record.draft_seed(), Some("PaladinInternal"));
        assert_eq!(record.derived_text(), Some("Holy Guard 수호자"));
    }

    #[test]
    fn stg_projection_derived_match_never_replaces_source_draft_implicitly() {
        let record =
            STGSearchRecord::new(3, Some("AreaInternal"), Some("Forest Crossing"), Some(22));
        assert!(record.matches(&STGSearchQuery::new("forest")));

        let mut draft = record.draft_seed().unwrap().to_owned();
        assert_eq!(draft, "AreaInternal");
        let suggestion = STGCatalogTextSuggestion::new("CatalogAreaSource", "Forest Crossing");
        assert_eq!(suggestion.source_preview(), "CatalogAreaSource");
        assert_eq!(suggestion.display_text(), "Forest Crossing");
        assert_eq!(draft, "AreaInternal");

        suggestion.apply_to(&mut draft);
        assert_eq!(draft, "CatalogAreaSource");
    }

    #[test]
    fn stg_projection_large_collections_store_ranges_and_block_metadata() {
        let units = STGIndexRows::range(1_000_000);
        assert_eq!(units.len(), 1_000_000);
        assert_eq!(units.source_index(999_999), Some(999_999));
        assert_eq!(units.stored_index_count(), 0);

        let filtered = STGIndexRows::filtered(vec![7, 70, 700]);
        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered.source_index(1), Some(70));
        assert_eq!(filtered.stored_index_count(), 3);

        let events = STGEventRows::from_blocks(vec![
            STGEventBlockProjection::new(0, 9, 750_000),
            STGEventBlockProjection::new(1, 10, 250_000),
        ]);
        assert_eq!(events.len(), 1_000_000);
        assert_eq!(events.target(749_999), Some(event(0, 749_999)));
        assert_eq!(events.target(750_000), Some(event(1, 0)));
        assert_eq!(events.target(999_999), Some(event(1, 249_999)));
        assert_eq!(events.stored_block_count(), 2);
        assert_eq!(events.stored_target_count(), 0);

        let references = STGReferenceRows::range(STGReferenceKind::Troop, 2_000_000);
        assert_eq!(references.len(), 2_000_000);
        assert_eq!(references.source_index(1_999_999), Some(1_999_999));
        assert_eq!(references.stored_index_count(), 0);
    }

    #[test]
    fn stg_reference_rows_preserve_event_targets_across_blocks_and_filters() {
        let unfiltered = STGReferenceRows::from_event_rows(
            STGReferenceKind::Trigger,
            STGEventRows::from_blocks(vec![
                STGEventBlockProjection::new(1, 8, 2),
                STGEventBlockProjection::new(4, 9, 1),
            ]),
        );

        assert_eq!(
            unfiltered.cursor(0),
            Some(STGReferenceCursor::Event(event(1, 0)))
        );
        assert_eq!(
            unfiltered.cursor(2),
            Some(STGReferenceCursor::Event(event(4, 0)))
        );

        let filtered = STGReferenceRows::from_event_rows(
            STGReferenceKind::Event,
            STGEventRows::filtered(vec![event(4, 0), event(1, 1)]),
        );

        assert_eq!(
            filtered.cursor(0),
            Some(STGReferenceCursor::Event(event(4, 0)))
        );
        assert_eq!(
            filtered.cursor(1),
            Some(STGReferenceCursor::Event(event(1, 1)))
        );
        assert_eq!(filtered.cursor(2), None);
    }

    #[test]
    fn stg_projection_iterates_event_targets_once_in_flat_source_order() {
        let events = STGEventRows::from_blocks(vec![
            STGEventBlockProjection::new(0, 7, 0),
            STGEventBlockProjection::new(1, 8, 2),
            STGEventBlockProjection::new(2, 9, 1),
        ]);

        assert_eq!(
            events.targets().collect::<Vec<_>>(),
            vec![event(1, 0), event(1, 1), event(2, 0)]
        );
    }

    #[test]
    fn stg_projection_indexes_late_rows_across_many_event_blocks() {
        let blocks = (0..100_000)
            .map(|block| STGEventBlockProjection::new(block, u32::try_from(block).unwrap(), 1))
            .collect();
        let events = STGEventRows::from_blocks(blocks);
        let last = event(99_999, 0);

        assert_eq!(
            events.blocks().unwrap().last().unwrap().flat_start(),
            99_999
        );
        assert_eq!(events.target(99_999), Some(last));
        assert_eq!(events.position_of(last), Some(99_999));
    }

    #[test]
    fn stg_projection_document_sections_hold_counts_instead_of_rendered_rows() {
        let projection = STGDocumentProjection::new(
            1_000_000,
            Some(900_000),
            Some(800_000),
            Some(vec![STGEventBlockProjection::new(4, 12, 700_000)]),
            Some(600_000),
            STGTailProjection::Parsed { suffix_bytes: 31 },
        );

        assert_eq!(projection.units().len(), 1_000_000);
        assert_eq!(projection.units().stored_index_count(), 0);
        assert_eq!(projection.areas().unwrap().len(), 900_000);
        assert_eq!(projection.variables().unwrap().len(), 800_000);
        assert_eq!(projection.events().unwrap().len(), 700_000);
        assert_eq!(projection.events().unwrap().stored_block_count(), 1);
        assert_eq!(projection.events().unwrap().stored_target_count(), 0);
        assert_eq!(projection.footer().unwrap().len(), 600_000);
        assert_eq!(
            projection.tail(),
            &STGTailProjection::Parsed { suffix_bytes: 31 }
        );
        assert_eq!(
            projection.section(STGCollectionProjection::Units),
            Some(1_000_000)
        );
        assert_eq!(
            projection.section(STGCollectionProjection::Events),
            Some(700_000)
        );
    }

    #[test]
    fn stg_projection_flattens_event_details_without_storing_parameter_rows() {
        let target = event(2, 4);
        let conditions = [1_000_000, 2];
        let actions = [3];
        let rows =
            STGEventDetailRows::from_parameter_counts(target, &conditions, &actions).unwrap();

        assert_eq!(rows.len(), 1_000_012);
        assert_eq!(rows.stored_script_group_count(), 3);
        assert_eq!(
            rows.row(0),
            Some(STGEventDetailRow::EventField(
                STGEventDetailField::Description
            ))
        );
        assert_eq!(
            rows.row(1),
            Some(STGEventDetailRow::EventField(STGEventDetailField::ID))
        );
        assert_eq!(
            rows.row(2),
            Some(STGEventDetailRow::ScriptHeader(script(
                2,
                4,
                STGScriptKind::Condition,
                0
            )))
        );
        assert_eq!(
            rows.row(3),
            Some(STGEventDetailRow::Parameter(STGParameterTarget {
                script: script(2, 4, STGScriptKind::Condition, 0),
                parameter: 0,
            }))
        );
        assert_eq!(
            rows.row(1_000_003),
            Some(STGEventDetailRow::ScriptHeader(script(
                2,
                4,
                STGScriptKind::Condition,
                1
            )))
        );
        assert_eq!(
            rows.row(1_000_006),
            Some(STGEventDetailRow::AddScript(STGScriptKind::Condition))
        );
        assert_eq!(
            rows.row(1_000_007),
            Some(STGEventDetailRow::ScriptHeader(script(
                2,
                4,
                STGScriptKind::Action,
                0
            )))
        );
        assert_eq!(
            rows.row(1_000_011),
            Some(STGEventDetailRow::AddScript(STGScriptKind::Action))
        );
        assert_eq!(rows.row(rows.len()), None);
    }

    #[test]
    fn stg_projection_checked_event_detail_length_rejects_overflow() {
        let target = event(0, 0);
        assert!(STGEventDetailRows::from_parameter_counts(target, &[usize::MAX], &[]).is_none());
    }

    #[test]
    fn stg_view_fields_keep_stable_ids_and_honest_source_states() {
        let document = document_id();
        let text_target = STGTextTarget::Header(STGHeaderTextField::MapFilename);
        let decoded = STGFieldProjection::text(
            document,
            STGSection::Header,
            text_target,
            STGText::Decoded(Cow::Borrowed("MAP\\STAGE01")),
        );
        let repeated = STGFieldProjection::text(
            document,
            STGSection::Header,
            text_target,
            STGText::Decoded(Cow::Borrowed("MAP\\STAGE01")),
        );
        let invalid = STGFieldProjection::text(
            document,
            STGSection::Header,
            text_target,
            STGText::Raw(&[0x81, 0xff]),
        );
        let choice_target = STGNumberTarget::Unit {
            unit: 3,
            field: STGUnitField::UCD,
        };
        let unknown = STGFieldProjection::number(
            document,
            STGSection::Units,
            choice_target,
            99,
            choice_target.editor(),
        );

        assert_eq!(decoded.id(), repeated.id());
        assert_eq!(decoded.state(), STGFieldState::Value);
        assert_eq!(decoded.display_value(), "MAP\\STAGE01");
        assert_eq!(invalid.state(), STGFieldState::InvalidText);
        assert!(invalid.display_value().contains("2 bytes"));
        assert_eq!(unknown.state(), STGFieldState::UnknownChoice);
        assert_eq!(unknown.display_value(), "Unknown (99)");
        assert!(matches!(
            choice_target.editor(),
            Some(STGEditor::Choice { .. })
        ));
        assert_ne!(decoded.id(), unknown.id());
    }

    #[test]
    fn stg_view_catalog_states_explain_raw_value_fallbacks() {
        let states = [
            STGCatalogAvailability::Missing,
            STGCatalogAvailability::Loading,
            STGCatalogAvailability::Failed("catalog could not be decoded".to_owned()),
            STGCatalogAvailability::Ready,
        ];

        assert!(states[0].message().unwrap().contains("not configured"));
        assert!(states[1].message().unwrap().contains("Loading"));
        assert!(
            states[2]
                .message()
                .unwrap()
                .contains("catalog could not be decoded")
        );
        assert_eq!(states[3].message(), None);
    }

    #[test]
    fn stg_virtual_lists_project_only_requested_typed_rows() {
        let document = document_id();
        let requested = 900_000..900_004;
        for rows in [
            STGVirtualRows::units(document, STGIndexRows::range(1_000_000)),
            STGVirtualRows::areas(document, STGIndexRows::range(1_000_000)),
            STGVirtualRows::variables(document, STGIndexRows::range(1_000_000)),
            STGVirtualRows::footer(document, STGIndexRows::range(1_000_000)),
        ] {
            let mut locations = rows.locations(requested.clone()).into_iter();
            assert_eq!(locations.len(), 4);
            let first = locations.next().unwrap();
            let second = locations.next().unwrap();
            let _third = locations.next().unwrap();
            let fourth = locations.next().unwrap();
            assert_eq!(first.position(), 900_000);
            assert_eq!(fourth.position(), 900_003);
            assert_ne!(first.id(), second.id());
        }

        let events = STGVirtualRows::events(
            document,
            STGEventRows::from_blocks(vec![
                STGEventBlockProjection::new(2, 7, 900_001),
                STGEventBlockProjection::new(9, 8, 99_999),
            ]),
        );
        let mut locations = events.locations(requested).into_iter();
        assert_eq!(locations.len(), 4);
        let first = locations.next().unwrap();
        let second = locations.next().unwrap();
        assert_eq!(first.cursor(), STGRowCursor::Event(event(2, 900_000)));
        assert_eq!(second.cursor(), STGRowCursor::Event(event(9, 0)));

        let detail =
            STGEventDetailRows::from_parameter_counts(event(4, 5), &[1_000_000], &[250_000])
                .unwrap();
        let detail = STGVirtualRows::event_details(document, event(4, 5), detail);
        let mut locations = detail.locations(999_999..1_000_003).into_iter();
        assert_eq!(locations.len(), 4);
        let first = locations.next().unwrap();
        assert_eq!(first.position(), 999_999);
        assert!(matches!(
            first.cursor(),
            STGRowCursor::EventDetail { event: target, .. } if target == event(4, 5)
        ));
    }

    #[test]
    fn stg_virtual_empty_and_raw_tail_sections_create_no_row_bindings() {
        let document = document_id();
        let empty = STGSectionProjection::from_rows(
            document,
            STGSection::Variables,
            STGVirtualRows::variables(document, STGIndexRows::range(0)),
        );
        let raw =
            STGSectionProjection::from_raw_tail(document, STGSection::Events, 417, "events", 1_024);

        assert!(empty.is_empty());
        assert!(empty.row_bindings(0..10).is_empty());
        assert!(raw.is_raw_tail());
        assert!(raw.row_bindings(0..10).is_empty());
        let panel = raw.raw_tail().unwrap();
        assert_eq!(panel.bytes(), 417);
        assert_eq!(panel.region(), "events");
        assert_eq!(panel.offset(), 1_024);
    }

    #[test]
    fn stg_virtual_events_keep_empty_source_blocks_visible() {
        let document = document_id();
        let rows = STGVirtualRows::events(
            document,
            STGEventRows::from_blocks(vec![
                STGEventBlockProjection::new(0, 17, 0),
                STGEventBlockProjection::new(2, 18, 2),
                STGEventBlockProjection::new(7, 19, 0),
            ]),
        );

        assert_eq!(rows.len(), 4);
        assert_eq!(rows.cursor(0), Some(STGRowCursor::EventBlock(0)));
        assert_eq!(rows.cursor(1), Some(STGRowCursor::Event(event(2, 0))));
        assert_eq!(rows.cursor(2), Some(STGRowCursor::Event(event(2, 1))));
        assert_eq!(rows.cursor(3), Some(STGRowCursor::EventBlock(7)));
        assert_eq!(rows.position_of(STGRowCursor::EventBlock(7)), Some(3));
    }

    #[gpui::test]
    fn stg_view_contextual_rail_and_content_share_the_files_editor_row(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        cx.draw(
            point(px(0.0), px(0.0)),
            size(px(900.0), px(600.0)),
            |_, _| {
                render_editor(
                    &Theme::default(),
                    Vec::new(),
                    None,
                    div().size_full().into_any_element(),
                )
            },
        );

        let editor = cx.debug_bounds("stg-editor").unwrap();
        let rail = cx.debug_bounds("stg-section-rail").unwrap();
        let content = cx.debug_bounds("stg-editor-content").unwrap();
        assert_eq!(rail.origin, editor.origin);
        assert_eq!(content.origin.y, editor.origin.y);
        assert_eq!(content.origin.x, rail.origin.x + rail.size.width);
        assert_eq!(rail.size.width + content.size.width, editor.size.width);
    }

    #[gpui::test]
    fn stg_view_selection_has_non_color_markers(cx: &mut TestAppContext) {
        let view = cx.new(|_| STGSelectionMarkers);
        let cx = cx.add_empty_window();
        cx.draw(
            point(px(0.0), px(0.0)),
            size(px(500.0), px(240.0)),
            move |_, _| view,
        );

        assert!(cx.debug_bounds("stg-section-active-marker").is_some());
        assert!(cx.debug_bounds("stg-row-inspecting-marker").is_some());
    }
}
