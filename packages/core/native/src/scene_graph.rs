use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::NativeTextBufferView;
use yoga::{
    Align, Context, Direction, Display, Edge, FlexDirection, Gutter, Justify, Layout, MeasureMode,
    Node, NodeRef, Overflow, PositionType, Size, StyleUnit, Wrap, get_node_ref_context,
};

static NEXT_SCENE_NODE_ID: AtomicU64 = AtomicU64::new(1);
static SCENE_GRAPH: OnceLock<Mutex<SceneGraph>> = OnceLock::new();

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeSceneLayout {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct NativeSceneStyle {
    pub width: f32,
    pub height: f32,
    pub min_width: f32,
    pub min_height: f32,
    pub max_width: f32,
    pub max_height: f32,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: f32,
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
    pub margin_top: f32,
    pub margin_right: f32,
    pub margin_bottom: f32,
    pub margin_left: f32,
    pub padding_top: f32,
    pub padding_right: f32,
    pub padding_bottom: f32,
    pub padding_left: f32,
    pub margin_all: f32,
    pub margin_horizontal: f32,
    pub margin_vertical: f32,
    pub padding_all: f32,
    pub padding_horizontal: f32,
    pub padding_vertical: f32,
    pub gap_all: f32,
    pub gap_row: f32,
    pub gap_column: f32,
    pub border_top: f32,
    pub border_right: f32,
    pub border_bottom: f32,
    pub border_left: f32,
    pub width_unit: u8,
    pub height_unit: u8,
    pub min_width_unit: u8,
    pub min_height_unit: u8,
    pub max_width_unit: u8,
    pub max_height_unit: u8,
    pub flex_basis_unit: u8,
    pub left_unit: u8,
    pub right_unit: u8,
    pub top_unit: u8,
    pub bottom_unit: u8,
    pub margin_top_unit: u8,
    pub margin_right_unit: u8,
    pub margin_bottom_unit: u8,
    pub margin_left_unit: u8,
    pub padding_top_unit: u8,
    pub padding_right_unit: u8,
    pub padding_bottom_unit: u8,
    pub padding_left_unit: u8,
    pub margin_all_unit: u8,
    pub margin_horizontal_unit: u8,
    pub margin_vertical_unit: u8,
    pub padding_all_unit: u8,
    pub padding_horizontal_unit: u8,
    pub padding_vertical_unit: u8,
    pub gap_all_unit: u8,
    pub gap_row_unit: u8,
    pub gap_column_unit: u8,
    pub display: u8,
    pub flex_direction: u8,
    pub position_type: u8,
    pub overflow: u8,
    pub flex_wrap: u8,
    pub align_items: u8,
    pub justify_content: u8,
    pub align_self: u8,
}

impl Default for NativeSceneStyle {
    fn default() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
            min_width: 0.0,
            min_height: 0.0,
            max_width: 0.0,
            max_height: 0.0,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: 0.0,
            left: 0.0,
            right: 0.0,
            top: 0.0,
            bottom: 0.0,
            margin_top: 0.0,
            margin_right: 0.0,
            margin_bottom: 0.0,
            margin_left: 0.0,
            padding_top: 0.0,
            padding_right: 0.0,
            padding_bottom: 0.0,
            padding_left: 0.0,
            margin_all: 0.0,
            margin_horizontal: 0.0,
            margin_vertical: 0.0,
            padding_all: 0.0,
            padding_horizontal: 0.0,
            padding_vertical: 0.0,
            gap_all: 0.0,
            gap_row: 0.0,
            gap_column: 0.0,
            border_top: 0.0,
            border_right: 0.0,
            border_bottom: 0.0,
            border_left: 0.0,
            width_unit: 3,
            height_unit: 3,
            min_width_unit: 3,
            min_height_unit: 3,
            max_width_unit: 3,
            max_height_unit: 3,
            flex_basis_unit: 3,
            left_unit: 3,
            right_unit: 3,
            top_unit: 3,
            bottom_unit: 3,
            margin_top_unit: 3,
            margin_right_unit: 3,
            margin_bottom_unit: 3,
            margin_left_unit: 3,
            padding_top_unit: 3,
            padding_right_unit: 3,
            padding_bottom_unit: 3,
            padding_left_unit: 3,
            margin_all_unit: 3,
            margin_horizontal_unit: 3,
            margin_vertical_unit: 3,
            padding_all_unit: 3,
            padding_horizontal_unit: 3,
            padding_vertical_unit: 3,
            gap_all_unit: 3,
            gap_row_unit: 3,
            gap_column_unit: 3,
            display: 0,
            flex_direction: 0,
            position_type: 0,
            overflow: 0,
            flex_wrap: 0,
            align_items: 4,
            justify_content: 0,
            align_self: 0,
        }
    }
}

#[derive(Debug)]
struct SceneNode {
    yoga: Node,
    parent: Option<u64>,
    children: Vec<u64>,
    measure: Option<SceneMeasure>,
}

unsafe impl Send for SceneNode {}

#[derive(Clone, Copy, Debug)]
enum SceneMeasure {
    TextBufferView {
        view: *mut NativeTextBufferView,
        clamp_at_most: bool,
    },
    LineNumber {
        view: *mut NativeTextBufferView,
        logical_line_count: u32,
        min_width: u32,
        padding_right: u32,
        line_number_offset: i32,
        max_custom_line_number: u32,
        max_before_width: u32,
        max_after_width: u32,
    },
}

#[derive(Clone, Copy, Debug, Default)]
struct SceneNodeContext {
    measure: Option<SceneMeasure>,
}

#[derive(Debug, Default)]
struct SceneGraph {
    nodes: HashMap<u64, Box<SceneNode>>,
}

impl SceneGraph {
    fn create_node(&mut self) -> u64 {
        let id = NEXT_SCENE_NODE_ID.fetch_add(1, Ordering::Relaxed);
        self.nodes.insert(
            id,
            Box::new(SceneNode {
                yoga: Node::new(),
                parent: None,
                children: Vec::new(),
                measure: None,
            }),
        );
        if let Some(node) = self.nodes.get_mut(&id) {
            sync_measure_binding(node);
        }
        id
    }

    fn destroy_node(&mut self, id: u64) -> bool {
        let Some(node) = self.nodes.get(&id) else {
            return false;
        };
        let parent = node.parent;
        let children = node.children.clone();
        let _ = node;

        if let Some(parent) = parent {
            let _ = self.remove_child(parent, id);
        }
        for child in children {
            if let Some(node) = self.nodes.get_mut(&child) {
                node.parent = None;
            }
        }
        self.nodes.remove(&id).is_some()
    }

    fn append_child(&mut self, parent: u64, child: u64) -> bool {
        let index = match self.nodes.get(&parent) {
            Some(parent_node) => parent_node.children.len(),
            None => return false,
        };
        self.insert_child(parent, child, index)
    }

    fn insert_before(&mut self, parent: u64, child: u64, anchor: u64) -> bool {
        let Some(parent_node) = self.nodes.get(&parent) else {
            return false;
        };
        let Some(index) = parent_node.children.iter().position(|existing| *existing == anchor) else {
            return false;
        };
        self.insert_child(parent, child, index)
    }

    fn remove_child(&mut self, parent: u64, child: u64) -> bool {
        let (parent_ptr, child_ptr, child_index) = {
            let Some(parent_node) = self.nodes.get(&parent) else {
                return false;
            };
            let Some(child_index) = parent_node.children.iter().position(|existing| *existing == child) else {
                return false;
            };
            let Some(child_node) = self.nodes.get(&child) else {
                return false;
            };
            (
                &parent_node.yoga as *const Node as *mut Node,
                &child_node.yoga as *const Node as *mut Node,
                child_index,
            )
        };

        unsafe {
            (*parent_ptr).remove_child(&mut *child_ptr);
        }

        if let Some(parent_node) = self.nodes.get_mut(&parent) {
            parent_node.children.remove(child_index);
        }
        if let Some(child_node) = self.nodes.get_mut(&child) {
            child_node.parent = None;
        }
        true
    }

    fn set_style(&mut self, id: u64, style: NativeSceneStyle) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };
        apply_style(&mut node.yoga, style);
        true
    }

    fn set_text_buffer_view_measure(
        &mut self,
        id: u64,
        view: *mut NativeTextBufferView,
        clamp_at_most: bool,
    ) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };
        if !node.children.is_empty() || view.is_null() {
            return false;
        }
        node.measure = Some(SceneMeasure::TextBufferView {
            view,
            clamp_at_most,
        });
        sync_measure_binding(node);
        node.yoga.mark_dirty();
        true
    }

    fn set_line_number_measure(
        &mut self,
        id: u64,
        view: *mut NativeTextBufferView,
        logical_line_count: u32,
        min_width: u32,
        padding_right: u32,
        line_number_offset: i32,
        max_custom_line_number: u32,
        max_before_width: u32,
        max_after_width: u32,
    ) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };
        if !node.children.is_empty() || view.is_null() {
            return false;
        }
        node.measure = Some(SceneMeasure::LineNumber {
            view,
            logical_line_count,
            min_width,
            padding_right,
            line_number_offset,
            max_custom_line_number,
            max_before_width,
            max_after_width,
        });
        sync_measure_binding(node);
        node.yoga.mark_dirty();
        true
    }

    fn calculate_layout(&mut self, root: u64, width: f32, height: f32) -> bool {
        let Some(node) = self.nodes.get_mut(&root) else {
            return false;
        };
        node.yoga.calculate_layout(width, height, Direction::LTR);
        true
    }

    fn get_layout(&self, id: u64) -> Option<NativeSceneLayout> {
        self.nodes.get(&id).map(|node| {
            let layout: Layout = node.yoga.get_layout();
            NativeSceneLayout {
                left: layout.left(),
                top: layout.top(),
                width: layout.width(),
                height: layout.height(),
            }
        })
    }

    fn child_count(&self, id: u64) -> usize {
        self.nodes.get(&id).map(|node| node.children.len()).unwrap_or(0)
    }

    fn insert_child(&mut self, parent: u64, child: u64, index: usize) -> bool {
        if parent == child {
            return false;
        }

        if self
            .nodes
            .get(&parent)
            .is_some_and(|node| node.measure.is_some())
        {
            return false;
        }

        let old_parent = self.nodes.get(&child).and_then(|node| node.parent);
        if let Some(old_parent) = old_parent {
            let _ = self.remove_child(old_parent, child);
        }

        let (parent_ptr, child_ptr) = {
            let Some(parent_node) = self.nodes.get(&parent) else {
                return false;
            };
            let Some(child_node) = self.nodes.get(&child) else {
                return false;
            };
            (
                &parent_node.yoga as *const Node as *mut Node,
                &child_node.yoga as *const Node as *mut Node,
            )
        };

        let insert_index = self
            .nodes
            .get(&parent)
            .map(|node| index.min(node.children.len()))
            .unwrap_or(index);

        unsafe {
            (*parent_ptr).insert_child(&mut *child_ptr, insert_index);
        }

        if let Some(parent_node) = self.nodes.get_mut(&parent) {
            parent_node.children.insert(insert_index, child);
        }
        if let Some(child_node) = self.nodes.get_mut(&child) {
            child_node.parent = Some(parent);
        }
        true
    }
}

fn scene_graph() -> &'static Mutex<SceneGraph> {
    SCENE_GRAPH.get_or_init(|| Mutex::new(SceneGraph::default()))
}

fn sync_measure_binding(node: &mut SceneNode) {
    node.yoga
        .set_context(Some(Context::new(SceneNodeContext { measure: node.measure })));
    let callback = if node.measure.is_some() {
        Some(scene_graph_measure as yoga::MeasureFunc)
    } else {
        None
    };
    node.yoga.set_measure_func(callback);
}

extern "C" fn scene_graph_measure(
    node_ref: NodeRef,
    width: f32,
    width_mode: MeasureMode,
    height: f32,
    _height_mode: MeasureMode,
) -> Size {
    let Some(context) = get_node_ref_context(&node_ref)
        .and_then(|ctx| ctx.downcast_ref::<SceneNodeContext>())
    else {
        return Size {
            width: 1.0,
            height: 1.0,
        };
    };

    let Some(measure) = context.measure else {
        return Size {
            width: 1.0,
            height: 1.0,
        };
    };

    match measure {
        SceneMeasure::TextBufferView {
            view,
            clamp_at_most,
        } => measure_text_buffer_view(view, width, width_mode, height, clamp_at_most),
        SceneMeasure::LineNumber {
            view,
            logical_line_count,
            min_width,
            padding_right,
            line_number_offset,
            max_custom_line_number,
            max_before_width,
            max_after_width,
        } => measure_line_number(
            view,
            logical_line_count,
            min_width,
            padding_right,
            line_number_offset,
            max_custom_line_number,
            max_before_width,
            max_after_width,
        ),
    }
}

fn measure_text_buffer_view(
    view: *mut NativeTextBufferView,
    width: f32,
    width_mode: MeasureMode,
    height: f32,
    clamp_at_most: bool,
) -> Size {
    if view.is_null() {
        return Size {
            width: 1.0,
            height: 1.0,
        };
    }

    let effective_width = if width_mode == MeasureMode::Undefined || width.is_nan() {
        0
    } else {
        width.floor().max(0.0) as u32
    };
    let effective_height = if height.is_nan() {
        1
    } else {
        height.floor().max(1.0) as u32
    };

    let view = unsafe { &mut *view };
    let measure = view.measure_for_dimensions(effective_width, effective_height);
    let measured_width = measure.width_cols_max.max(1) as f32;
    let measured_height = measure.line_count.max(1) as f32;

    if clamp_at_most && width_mode == MeasureMode::AtMost {
        return Size {
            width: measured_width.min(effective_width as f32),
            height: measured_height.min(effective_height as f32),
        };
    }

    Size {
        width: measured_width,
        height: measured_height,
    }
}

fn measure_line_number(
    view: *mut NativeTextBufferView,
    logical_line_count: u32,
    min_width: u32,
    padding_right: u32,
    line_number_offset: i32,
    max_custom_line_number: u32,
    max_before_width: u32,
    max_after_width: u32,
) -> Size {
    if view.is_null() {
        return Size {
            width: 1.0,
            height: 1.0,
        };
    }

    let view = unsafe { &mut *view };
    let virtual_line_count = view.virtual_line_count().max(1);
    let total_lines = logical_line_count.max(virtual_line_count);
    let offset_total = i64::from(total_lines) + i64::from(line_number_offset);
    let max_line_number = offset_total.max(i64::from(max_custom_line_number));
    let digits = if max_line_number > 0 {
        (max_line_number as f64).log10().floor() as u32 + 1
    } else {
        1
    };
    let base_width = min_width.max(digits + padding_right + 1);

    Size {
        width: (base_width + max_before_width + max_after_width) as f32,
        height: virtual_line_count as f32,
    }
}

fn apply_style(node: &mut Node, style: NativeSceneStyle) {
    node.set_width(style_unit(style.width, style.width_unit));
    node.set_height(style_unit(style.height, style.height_unit));
    node.set_min_width(style_unit(style.min_width, style.min_width_unit));
    node.set_min_height(style_unit(style.min_height, style.min_height_unit));
    node.set_max_width(style_unit(style.max_width, style.max_width_unit));
    node.set_max_height(style_unit(style.max_height, style.max_height_unit));
    node.set_flex_grow(style.flex_grow);
    node.set_flex_shrink(style.flex_shrink);
    node.set_flex_basis(style_unit(style.flex_basis, style.flex_basis_unit));
    node.set_position(Edge::Left, style_unit(style.left, style.left_unit));
    node.set_position(Edge::Right, style_unit(style.right, style.right_unit));
    node.set_position(Edge::Top, style_unit(style.top, style.top_unit));
    node.set_position(Edge::Bottom, style_unit(style.bottom, style.bottom_unit));
    if style.margin_all_unit != 3 {
        node.set_margin(Edge::All, style_unit(style.margin_all, style.margin_all_unit));
    }
    if style.margin_horizontal_unit != 3 {
        node.set_margin(
            Edge::Horizontal,
            style_unit(style.margin_horizontal, style.margin_horizontal_unit),
        );
    }
    if style.margin_vertical_unit != 3 {
        node.set_margin(
            Edge::Vertical,
            style_unit(style.margin_vertical, style.margin_vertical_unit),
        );
    }
    if style.margin_top_unit != 3 {
        node.set_margin(Edge::Top, style_unit(style.margin_top, style.margin_top_unit));
    }
    if style.margin_right_unit != 3 {
        node.set_margin(Edge::Right, style_unit(style.margin_right, style.margin_right_unit));
    }
    if style.margin_bottom_unit != 3 {
        node.set_margin(
            Edge::Bottom,
            style_unit(style.margin_bottom, style.margin_bottom_unit),
        );
    }
    if style.margin_left_unit != 3 {
        node.set_margin(Edge::Left, style_unit(style.margin_left, style.margin_left_unit));
    }
    if style.padding_all_unit != 3 {
        node.set_padding(Edge::All, style_unit(style.padding_all, style.padding_all_unit));
    }
    if style.padding_horizontal_unit != 3 {
        node.set_padding(
            Edge::Horizontal,
            style_unit(style.padding_horizontal, style.padding_horizontal_unit),
        );
    }
    if style.padding_vertical_unit != 3 {
        node.set_padding(
            Edge::Vertical,
            style_unit(style.padding_vertical, style.padding_vertical_unit),
        );
    }
    if style.padding_top_unit != 3 {
        node.set_padding(Edge::Top, style_unit(style.padding_top, style.padding_top_unit));
    }
    if style.padding_right_unit != 3 {
        node.set_padding(
            Edge::Right,
            style_unit(style.padding_right, style.padding_right_unit),
        );
    }
    if style.padding_bottom_unit != 3 {
        node.set_padding(
            Edge::Bottom,
            style_unit(style.padding_bottom, style.padding_bottom_unit),
        );
    }
    if style.padding_left_unit != 3 {
        node.set_padding(Edge::Left, style_unit(style.padding_left, style.padding_left_unit));
    }
    if style.gap_all_unit != 3 {
        let gap_all = style_unit(style.gap_all, style.gap_all_unit);
        node.set_gap(Gutter::Row, gap_all);
        node.set_gap(Gutter::Column, gap_all);
    }
    if style.gap_row_unit != 3 {
        node.set_gap(Gutter::Row, style_unit(style.gap_row, style.gap_row_unit));
    }
    if style.gap_column_unit != 3 {
        node.set_gap(
            Gutter::Column,
            style_unit(style.gap_column, style.gap_column_unit),
        );
    }
    node.set_border(Edge::Top, style.border_top);
    node.set_border(Edge::Right, style.border_right);
    node.set_border(Edge::Bottom, style.border_bottom);
    node.set_border(Edge::Left, style.border_left);
    node.set_display(match style.display {
        1 => Display::None,
        2 => Display::Contents,
        _ => Display::Flex,
    });
    node.set_flex_direction(match style.flex_direction {
        1 => FlexDirection::ColumnReverse,
        2 => FlexDirection::Row,
        3 => FlexDirection::RowReverse,
        _ => FlexDirection::Column,
    });
    node.set_flex_wrap(match style.flex_wrap {
        1 => Wrap::Wrap,
        2 => Wrap::WrapReverse,
        _ => Wrap::NoWrap,
    });
    node.set_align_items(match style.align_items {
        1 => Align::FlexStart,
        2 => Align::Center,
        3 => Align::FlexEnd,
        4 => Align::Stretch,
        5 => Align::Baseline,
        6 => Align::SpaceBetween,
        7 => Align::SpaceAround,
        8 => Align::SpaceEvenly,
        _ => Align::Auto,
    });
    node.set_justify_content(match style.justify_content {
        1 => Justify::Center,
        2 => Justify::FlexEnd,
        3 => Justify::SpaceBetween,
        4 => Justify::SpaceAround,
        5 => Justify::SpaceEvenly,
        _ => Justify::FlexStart,
    });
    node.set_align_self(match style.align_self {
        1 => Align::FlexStart,
        2 => Align::Center,
        3 => Align::FlexEnd,
        4 => Align::Stretch,
        5 => Align::Baseline,
        6 => Align::SpaceBetween,
        7 => Align::SpaceAround,
        8 => Align::SpaceEvenly,
        _ => Align::Auto,
    });
    node.set_position_type(match style.position_type {
        1 => PositionType::Absolute,
        _ => PositionType::Relative,
    });
    node.set_overflow(match style.overflow {
        1 => Overflow::Hidden,
        2 => Overflow::Scroll,
        _ => Overflow::Visible,
    });
}

fn style_unit(value: f32, unit: u8) -> StyleUnit {
    match unit {
        1 => StyleUnit::Auto,
        2 => StyleUnit::Percent(value.into()),
        3 => StyleUnit::UndefinedValue,
        _ => StyleUnit::Point(value.into()),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn createSceneNode() -> u64 {
    scene_graph().lock().unwrap().create_node()
}

#[unsafe(no_mangle)]
pub extern "C" fn destroySceneNode(id: u64) -> bool {
    scene_graph().lock().unwrap().destroy_node(id)
}

#[unsafe(no_mangle)]
pub extern "C" fn sceneNodeAppendChild(parent: u64, child: u64) -> bool {
    scene_graph().lock().unwrap().append_child(parent, child)
}

#[unsafe(no_mangle)]
pub extern "C" fn sceneNodeInsertBefore(parent: u64, child: u64, anchor: u64) -> bool {
    scene_graph().lock().unwrap().insert_before(parent, child, anchor)
}

#[unsafe(no_mangle)]
pub extern "C" fn sceneNodeRemoveChild(parent: u64, child: u64) -> bool {
    scene_graph().lock().unwrap().remove_child(parent, child)
}

#[unsafe(no_mangle)]
pub extern "C" fn sceneNodeSetStyle(id: u64, style_ptr: *const NativeSceneStyle) -> bool {
    if style_ptr.is_null() {
        return false;
    }
    let style = unsafe { std::ptr::read_unaligned(style_ptr) };
    scene_graph().lock().unwrap().set_style(id, style)
}

#[unsafe(no_mangle)]
pub extern "C" fn sceneNodeSetTextBufferViewMeasure(
    id: u64,
    view: *mut NativeTextBufferView,
    clamp_at_most: bool,
) -> bool {
    scene_graph()
        .lock()
        .unwrap()
        .set_text_buffer_view_measure(id, view, clamp_at_most)
}

#[unsafe(no_mangle)]
pub extern "C" fn sceneNodeSetLineNumberMeasure(
    id: u64,
    view: *mut NativeTextBufferView,
    logical_line_count: u32,
    min_width: u32,
    padding_right: u32,
    line_number_offset: i32,
    max_custom_line_number: u32,
    max_before_width: u32,
    max_after_width: u32,
) -> bool {
    scene_graph().lock().unwrap().set_line_number_measure(
        id,
        view,
        logical_line_count,
        min_width,
        padding_right,
        line_number_offset,
        max_custom_line_number,
        max_before_width,
        max_after_width,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn sceneNodeCalculateLayout(root: u64, width: f32, height: f32) -> bool {
    scene_graph().lock().unwrap().calculate_layout(root, width, height)
}

#[unsafe(no_mangle)]
pub extern "C" fn sceneNodeGetLayout(id: u64, out_ptr: *mut NativeSceneLayout) -> bool {
    if out_ptr.is_null() {
        return false;
    }
    let layout = {
        let graph = scene_graph().lock().unwrap();
        graph.get_layout(id)
    };
    let Some(layout) = layout else {
        return false;
    };
    unsafe {
        std::ptr::write_unaligned(out_ptr, layout);
    }
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn sceneNodeGetChildCount(id: u64) -> usize {
    scene_graph().lock().unwrap().child_count(id)
}

#[cfg(test)]
mod tests {
    use super::{NativeSceneLayout, NativeSceneStyle, createSceneNode, destroySceneNode, sceneNodeAppendChild, sceneNodeCalculateLayout, sceneNodeGetChildCount, sceneNodeGetLayout, sceneNodeSetStyle};

    #[test]
    fn native_scene_style_abi_size_is_stable() {
        assert_eq!(std::mem::size_of::<NativeSceneStyle>(), 172);
    }

    #[test]
    fn scene_graph_calculates_simple_column_layout() {
        let root = createSceneNode();
        let child = createSceneNode();

        let root_style = NativeSceneStyle {
            width: 100.0,
            height: 40.0,
            flex_direction: 0,
            ..NativeSceneStyle::default()
        };
        let child_style = NativeSceneStyle {
            width: 50.0,
            height: 10.0,
            ..NativeSceneStyle::default()
        };

        assert!(sceneNodeSetStyle(root, &root_style));
        assert!(sceneNodeSetStyle(child, &child_style));
        assert!(sceneNodeAppendChild(root, child));
        assert_eq!(sceneNodeGetChildCount(root), 1);
        assert!(sceneNodeCalculateLayout(root, 100.0, 40.0));

        let mut root_layout = NativeSceneLayout::default();
        assert!(sceneNodeGetLayout(root, &mut root_layout));
        assert!(root_layout.width >= 0.0);
        assert!(root_layout.height >= 0.0);

        assert!(destroySceneNode(child));
        assert!(destroySceneNode(root));
    }

    #[test]
    fn scene_graph_applies_column_gap_layout() {
        let root = createSceneNode();
        let first = createSceneNode();
        let second = createSceneNode();

        let root_style = NativeSceneStyle {
            width: 30.0,
            height: 10.0,
            width_unit: 0,
            height_unit: 0,
            flex_direction: 2,
            gap_all: 2.0,
            gap_all_unit: 0,
            ..NativeSceneStyle::default()
        };
        let child_style = NativeSceneStyle {
            width: 5.0,
            height: 2.0,
            width_unit: 0,
            height_unit: 0,
            ..NativeSceneStyle::default()
        };
        let second_style = NativeSceneStyle {
            width: 7.0,
            height: 2.0,
            width_unit: 0,
            height_unit: 0,
            ..NativeSceneStyle::default()
        };

        assert!(sceneNodeSetStyle(root, &root_style));
        assert!(sceneNodeSetStyle(first, &child_style));
        assert!(sceneNodeSetStyle(second, &second_style));
        assert!(sceneNodeAppendChild(root, first));
        assert!(sceneNodeAppendChild(root, second));
        assert!(sceneNodeCalculateLayout(root, 30.0, 10.0));

        let mut second_layout = NativeSceneLayout::default();
        assert!(sceneNodeGetLayout(second, &mut second_layout));
        assert_eq!(second_layout.left, 7.0);

        assert!(destroySceneNode(second));
        assert!(destroySceneNode(first));
        assert!(destroySceneNode(root));
    }
}
