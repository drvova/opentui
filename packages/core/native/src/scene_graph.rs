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

#[derive(Clone, Copy, Debug)]
struct ClipRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

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
    pub z_index: f32,
    pub opacity: f32,
    pub translate_x: f32,
    pub translate_y: f32,
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
    pub buffered: bool,
    pub reserved0: u8,
    pub reserved1: u8,
    pub reserved2: u8,
    pub renderable_num: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeTextTableMeasureConfig {
    pub row_count: u32,
    pub column_count: u32,
    pub cell_padding: u32,
    pub wrap_mode: u8,
    pub column_width_mode: u8,
    pub column_fitter: u8,
    pub border: bool,
    pub outer_border: bool,
    pub clamp_at_most: bool,
    pub reserved: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeSceneRenderCommand {
    pub kind: u8,
    pub has_clip: u8,
    pub reserved1: u8,
    pub reserved2: u8,
    pub renderable_num: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub screen_x: i32,
    pub screen_y: i32,
    pub clip_x: i32,
    pub clip_y: i32,
    pub clip_width: u32,
    pub clip_height: u32,
    pub opacity: f32,
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
            z_index: 0.0,
            opacity: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
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
            buffered: false,
            reserved0: 0,
            reserved1: 0,
            reserved2: 0,
            renderable_num: 0,
        }
    }
}

#[derive(Debug)]
struct SceneNode {
    yoga: Node,
    parent: Option<u64>,
    children: Vec<u64>,
    measure: Option<SceneMeasure>,
    visible_children: Option<Vec<u64>>,
    z_index: f32,
    opacity: f32,
    translate_x: f32,
    translate_y: f32,
    buffered: bool,
    renderable_num: u32,
    display: u8,
    overflow: u8,
}

unsafe impl Send for SceneNode {}

#[derive(Clone, Debug)]
enum SceneMeasure {
    TextBufferView {
        view: *mut NativeTextBufferView,
        clamp_at_most: bool,
    },
    TextTable(TextTableMeasure),
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

#[derive(Clone, Debug, Default)]
struct SceneNodeContext {
    measure: Option<SceneMeasure>,
}

#[derive(Clone, Debug)]
struct TextTableMeasure {
    row_count: usize,
    column_count: usize,
    cell_padding: u32,
    wrap_mode: u8,
    column_width_mode: u8,
    column_fitter: u8,
    border: bool,
    outer_border: bool,
    clamp_at_most: bool,
    cell_views: Vec<usize>,
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
                visible_children: None,
                z_index: 0.0,
                opacity: 1.0,
                translate_x: 0.0,
                translate_y: 0.0,
                buffered: false,
                renderable_num: 0,
                display: 0,
                overflow: 0,
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
        let Some(mut index) = parent_node.children.iter().position(|existing| *existing == anchor) else {
            return false;
        };
        let old_parent = self.nodes.get(&child).and_then(|node| node.parent);
        if old_parent == Some(parent)
            && let Some(child_index) = parent_node.children.iter().position(|existing| *existing == child)
            && child_index < index
        {
            index -= 1;
        }
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
        node.z_index = style.z_index;
        node.opacity = style.opacity;
        node.translate_x = style.translate_x;
        node.translate_y = style.translate_y;
        node.buffered = style.buffered;
        node.renderable_num = style.renderable_num;
        node.display = style.display;
        node.overflow = style.overflow;
        true
    }

    fn set_visible_children(&mut self, id: u64, visible_children: Vec<u64>) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };
        if visible_children.is_empty() {
            node.visible_children = None;
            return true;
        }
        node.visible_children = Some(visible_children);
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

    fn set_text_table_measure(
        &mut self,
        id: u64,
        config: NativeTextTableMeasureConfig,
        cell_views: Vec<usize>,
    ) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };
        if !node.children.is_empty() {
            return false;
        }

        let row_count = config.row_count as usize;
        let column_count = config.column_count as usize;
        if row_count.saturating_mul(column_count) != cell_views.len() {
            return false;
        }

        node.measure = Some(SceneMeasure::TextTable(TextTableMeasure {
            row_count,
            column_count,
            cell_padding: config.cell_padding,
            wrap_mode: config.wrap_mode,
            column_width_mode: config.column_width_mode,
            column_fitter: config.column_fitter,
            border: config.border,
            outer_border: config.outer_border,
            clamp_at_most: config.clamp_at_most,
            cell_views,
        }));
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

    fn child_handles(&self, id: u64) -> Option<&[u64]> {
        self.nodes.get(&id).map(|node| node.children.as_slice())
    }

    fn child_handles_by_z_index(&self, id: u64) -> Option<Vec<u64>> {
        let parent = self.nodes.get(&id)?;
        let mut ordered = parent.children.clone();
        ordered.sort_by(|a, b| {
            let az = self.nodes.get(a).map(|node| node.z_index).unwrap_or(0.0);
            let bz = self.nodes.get(b).map(|node| node.z_index).unwrap_or(0.0);
            az.partial_cmp(&bz).unwrap_or(std::cmp::Ordering::Equal)
        });
        Some(ordered)
    }

    fn subtree_node_count(&self, id: u64) -> usize {
        let Some(node) = self.nodes.get(&id) else {
            return 0;
        };

        1 + node
            .children
            .iter()
            .map(|child| self.subtree_node_count(*child))
            .sum::<usize>()
    }

    fn build_render_plan(&self, root: u64, out: &mut Vec<NativeSceneRenderCommand>) -> bool {
        let Some(root_node) = self.nodes.get(&root) else {
            return false;
        };

        for child in self.child_handles_by_z_index(root).unwrap_or_else(|| root_node.children.clone()) {
            self.build_render_plan_for_node(child, 0, 0, None, out);
        }
        true
    }

    fn build_render_plan_for_node(
        &self,
        id: u64,
        parent_x: i32,
        parent_y: i32,
        inherited_clip: Option<ClipRect>,
        out: &mut Vec<NativeSceneRenderCommand>,
    ) {
        let Some(node) = self.nodes.get(&id) else {
            return;
        };
        if node.display == 1 {
            return;
        }

        let x = parent_x + node.yoga.get_layout_left() as i32 + node.translate_x as i32;
        let y = parent_y + node.yoga.get_layout_top() as i32 + node.translate_y as i32;
        let width = node.yoga.get_layout_width().max(0.0) as u32;
        let height = node.yoga.get_layout_height().max(0.0) as u32;

        if node.opacity < 1.0 {
            out.push(NativeSceneRenderCommand {
                kind: 3,
                opacity: node.opacity,
                ..NativeSceneRenderCommand::default()
            });
        }

        out.push(NativeSceneRenderCommand {
            kind: 0,
            has_clip: u8::from(inherited_clip.is_some()),
            renderable_num: node.renderable_num,
            x,
            y,
            width,
            height,
            clip_x: inherited_clip.map(|clip| clip.x).unwrap_or_default(),
            clip_y: inherited_clip.map(|clip| clip.y).unwrap_or_default(),
            clip_width: inherited_clip.map(|clip| clip.width).unwrap_or_default(),
            clip_height: inherited_clip.map(|clip| clip.height).unwrap_or_default(),
            ..NativeSceneRenderCommand::default()
        });

        let mut child_clip = inherited_clip;
        if node.overflow != 0 && width > 0 && height > 0 {
            let left_inset = if node.yoga.get_layout_border_left() > 0.0 {
                1
            } else {
                0
            };
            let right_inset = if node.yoga.get_layout_border_right() > 0.0 {
                1
            } else {
                0
            };
            let top_inset = if node.yoga.get_layout_border_top() > 0.0 {
                1
            } else {
                0
            };
            let bottom_inset = if node.yoga.get_layout_border_bottom() > 0.0 {
                1
            } else {
                0
            };

            let scissor_x = if node.buffered { left_inset } else { x + left_inset };
            let scissor_y = if node.buffered { top_inset } else { y + top_inset };
            let scissor_width =
                width.saturating_sub((left_inset as u32).saturating_add(right_inset as u32));
            let scissor_height =
                height.saturating_sub((top_inset as u32).saturating_add(bottom_inset as u32));
            let screen_scissor = ClipRect {
                x: x + left_inset,
                y: y + top_inset,
                width: scissor_width,
                height: scissor_height,
            };

            child_clip = merge_clip_rects(inherited_clip, Some(screen_scissor));

            out.push(NativeSceneRenderCommand {
                kind: 1,
                x: scissor_x,
                y: scissor_y,
                width: scissor_width,
                height: scissor_height,
                screen_x: x,
                screen_y: y,
                ..NativeSceneRenderCommand::default()
            });
        }

        let ordered_children = if let Some(visible_children) = &node.visible_children {
            let mut ordered = visible_children.clone();
            ordered.sort_by(|a, b| {
                let az = self.nodes.get(a).map(|node| node.z_index).unwrap_or(0.0);
                let bz = self.nodes.get(b).map(|node| node.z_index).unwrap_or(0.0);
                az.partial_cmp(&bz).unwrap_or(std::cmp::Ordering::Equal)
            });
            ordered
        } else {
            self.child_handles_by_z_index(id).unwrap_or_else(|| node.children.clone())
        };

        for child in ordered_children {
            self.build_render_plan_for_node(child, x, y, child_clip, out);
        }

        if node.overflow != 0 && width > 0 && height > 0 {
            out.push(NativeSceneRenderCommand {
                kind: 2,
                ..NativeSceneRenderCommand::default()
            });
        }

        if node.opacity < 1.0 {
            out.push(NativeSceneRenderCommand {
                kind: 4,
                ..NativeSceneRenderCommand::default()
            });
        }
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
        .set_context(Some(Context::new(SceneNodeContext {
            measure: node.measure.clone(),
        })));
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

    let Some(ref measure) = context.measure else {
        return Size {
            width: 1.0,
            height: 1.0,
        };
    };

    match measure {
        SceneMeasure::TextBufferView {
            view,
            clamp_at_most,
        } => measure_text_buffer_view(*view, width, width_mode, height, *clamp_at_most),
        SceneMeasure::TextTable(table) => measure_text_table(table, width, width_mode),
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
            *view,
            *logical_line_count,
            *min_width,
            *padding_right,
            *line_number_offset,
            *max_custom_line_number,
            *max_before_width,
            *max_after_width,
        ),
    }
}

fn measure_text_table(table: &TextTableMeasure, width: f32, width_mode: MeasureMode) -> Size {
    let layout = compute_text_table_layout(
        table,
        if width_mode == MeasureMode::Undefined || width.is_nan() {
            None
        } else {
            Some(width.floor().max(1.0) as u32)
        },
    );

    let mut measured_width = layout.table_width.max(1) as f32;
    let measured_height = layout.table_height.max(1) as f32;

    if table.clamp_at_most && width_mode == MeasureMode::AtMost && width.is_finite() {
        measured_width = measured_width.min(width.floor().max(1.0));
    }

    Size {
        width: measured_width,
        height: measured_height,
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

#[derive(Clone, Copy)]
struct TableBorderLayout {
    left: bool,
    right: bool,
    top: bool,
    bottom: bool,
    inner_vertical: bool,
    inner_horizontal: bool,
}

#[derive(Default)]
struct TableMeasureLayout {
    table_width: u32,
    table_height: u32,
}

fn compute_text_table_layout(table: &TextTableMeasure, raw_width_constraint: Option<u32>) -> TableMeasureLayout {
    if table.row_count == 0 || table.column_count == 0 {
        return TableMeasureLayout::default();
    }

    let width_constraint = resolve_table_width_constraint(table, raw_width_constraint);
    let border_layout = resolve_table_border_layout(table);
    let column_widths = compute_table_column_widths(table, width_constraint, border_layout);
    let row_heights = compute_table_row_heights(table, &column_widths);
    let column_offsets = compute_table_offsets(
        &column_widths,
        border_layout.left,
        border_layout.right,
        border_layout.inner_vertical,
    );
    let row_offsets = compute_table_offsets(
        &row_heights,
        border_layout.top,
        border_layout.bottom,
        border_layout.inner_horizontal,
    );

    TableMeasureLayout {
        table_width: column_offsets.last().copied().unwrap_or(0).saturating_add(1),
        table_height: row_offsets.last().copied().unwrap_or(0).saturating_add(1),
    }
}

fn resolve_table_width_constraint(table: &TextTableMeasure, width: Option<u32>) -> Option<u32> {
    let width = width?;
    if width == 0 {
        return None;
    }
    if table.wrap_mode != 0 || table.column_width_mode == 1 {
        return Some(width.max(1));
    }
    None
}

fn resolve_table_border_layout(table: &TextTableMeasure) -> TableBorderLayout {
    TableBorderLayout {
        left: table.outer_border,
        right: table.outer_border,
        top: table.outer_border,
        bottom: table.outer_border,
        inner_vertical: table.border && table.column_count > 1,
        inner_horizontal: table.border && table.row_count > 1,
    }
}

fn get_horizontal_cell_padding(table: &TextTableMeasure) -> u32 {
    table.cell_padding.saturating_mul(2)
}

fn get_vertical_cell_padding(table: &TextTableMeasure) -> u32 {
    table.cell_padding.saturating_mul(2)
}

fn get_vertical_border_count(table: &TextTableMeasure, border_layout: TableBorderLayout) -> u32 {
    (border_layout.left as u32)
        + (border_layout.right as u32)
        + if border_layout.inner_vertical {
            table.column_count.saturating_sub(1) as u32
        } else {
            0
        }
}

fn cell_view_ptr(table: &TextTableMeasure, row: usize, col: usize) -> Option<*mut NativeTextBufferView> {
    let index = row.checked_mul(table.column_count)?.checked_add(col)?;
    let raw = *table.cell_views.get(index)?;
    if raw == 0 {
        return None;
    }
    Some(raw as *mut NativeTextBufferView)
}

fn compute_table_column_widths(
    table: &TextTableMeasure,
    max_table_width: Option<u32>,
    border_layout: TableBorderLayout,
) -> Vec<u32> {
    let horizontal_padding = get_horizontal_cell_padding(table);
    let mut intrinsic_widths = vec![1 + horizontal_padding; table.column_count];

    for row_idx in 0..table.row_count {
        for col_idx in 0..table.column_count {
            let Some(view_ptr) = cell_view_ptr(table, row_idx, col_idx) else {
                continue;
            };
            let view = unsafe { &mut *view_ptr };
            let measure = view.measure_for_dimensions(0, 10_000);
            let measured_width = measure.width_cols_max.max(1).saturating_add(horizontal_padding);
            intrinsic_widths[col_idx] = intrinsic_widths[col_idx].max(measured_width);
        }
    }

    let Some(max_table_width) = max_table_width else {
        return intrinsic_widths;
    };
    if max_table_width == 0 {
        return intrinsic_widths;
    }

    let max_content_width = max_table_width
        .saturating_sub(get_vertical_border_count(table, border_layout))
        .max(1);
    let current_width: u32 = intrinsic_widths.iter().copied().sum();

    if current_width == max_content_width {
        return intrinsic_widths;
    }

    if current_width < max_content_width {
        if table.column_width_mode == 1 {
            return expand_table_column_widths(&intrinsic_widths, max_content_width);
        }
        return intrinsic_widths;
    }

    if table.wrap_mode == 0 {
        return intrinsic_widths;
    }

    if table.column_fitter == 1 {
        fit_table_column_widths_balanced(table, &intrinsic_widths, max_content_width)
    } else {
        fit_table_column_widths_proportional(table, &intrinsic_widths, max_content_width)
    }
}

fn expand_table_column_widths(widths: &[u32], target_content_width: u32) -> Vec<u32> {
    let mut expanded: Vec<u32> = widths.iter().map(|width| (*width).max(1)).collect();
    let total_base_width: u32 = expanded.iter().copied().sum();

    if total_base_width >= target_content_width || expanded.is_empty() {
        return expanded;
    }

    let columns = expanded.len() as u32;
    let extra_width = target_content_width - total_base_width;
    let shared_width = extra_width / columns;
    let remainder = extra_width % columns;

    for (idx, width) in expanded.iter_mut().enumerate() {
        *width += shared_width;
        if (idx as u32) < remainder {
            *width += 1;
        }
    }

    expanded
}

fn fit_table_column_widths_proportional(
    table: &TextTableMeasure,
    widths: &[u32],
    target_content_width: u32,
) -> Vec<u32> {
    let min_width = 1 + get_horizontal_cell_padding(table);
    let hard_min_widths = vec![min_width; widths.len()];
    let base_widths: Vec<u32> = widths.iter().map(|width| (*width).max(1)).collect();

    let preferred_min_widths: Vec<u32> = base_widths
        .iter()
        .map(|width| (*width).min(min_width + 1))
        .collect();
    let preferred_min_total: u32 = preferred_min_widths.iter().copied().sum();
    let floor_widths = if preferred_min_total <= target_content_width {
        preferred_min_widths
    } else {
        hard_min_widths
    };
    let floor_total: u32 = floor_widths.iter().copied().sum();
    let clamped_target = floor_total.max(target_content_width);

    let total_base_width: u32 = base_widths.iter().copied().sum();
    if total_base_width <= clamped_target {
        return base_widths;
    }

    let shrinkable: Vec<u32> = base_widths
        .iter()
        .zip(floor_widths.iter())
        .map(|(width, floor)| width.saturating_sub(*floor))
        .collect();
    let total_shrinkable: u32 = shrinkable.iter().copied().sum();
    if total_shrinkable == 0 {
        return floor_widths;
    }

    let target_shrink = total_base_width - clamped_target;
    let mut integer_shrink = vec![0u32; base_widths.len()];
    let mut fractions = vec![0f64; base_widths.len()];
    let mut used_shrink = 0u32;

    for idx in 0..base_widths.len() {
        if shrinkable[idx] == 0 {
            continue;
        }
        let exact = (shrinkable[idx] as f64 / total_shrinkable as f64) * target_shrink as f64;
        let whole = shrinkable[idx].min(exact.floor() as u32);
        integer_shrink[idx] = whole;
        fractions[idx] = exact - whole as f64;
        used_shrink += whole;
    }

    let mut remaining_shrink = target_shrink.saturating_sub(used_shrink);
    while remaining_shrink > 0 {
        let mut best_idx = None;
        let mut best_fraction = -1f64;

        for idx in 0..base_widths.len() {
            if shrinkable[idx].saturating_sub(integer_shrink[idx]) == 0 {
                continue;
            }
            if fractions[idx] > best_fraction {
                best_fraction = fractions[idx];
                best_idx = Some(idx);
            }
        }

        let Some(best_idx) = best_idx else {
            break;
        };
        integer_shrink[best_idx] += 1;
        fractions[best_idx] = 0.0;
        remaining_shrink -= 1;
    }

    base_widths
        .iter()
        .zip(floor_widths.iter())
        .zip(integer_shrink.iter())
        .map(|((width, floor), shrink)| floor.max(&width.saturating_sub(*shrink)).to_owned())
        .collect()
}

fn fit_table_column_widths_balanced(
    table: &TextTableMeasure,
    widths: &[u32],
    target_content_width: u32,
) -> Vec<u32> {
    let min_width = 1 + get_horizontal_cell_padding(table);
    let hard_min_widths = vec![min_width; widths.len()];
    let base_widths: Vec<u32> = widths.iter().map(|width| (*width).max(1)).collect();
    let total_base_width: u32 = base_widths.iter().copied().sum();
    let columns = base_widths.len() as u32;

    if columns == 0 || total_base_width <= target_content_width {
        return base_widths;
    }

    let even_share = min_width.max(target_content_width / columns.max(1));
    let preferred_min_widths: Vec<u32> = base_widths.iter().map(|width| (*width).min(even_share)).collect();
    let preferred_min_total: u32 = preferred_min_widths.iter().copied().sum();
    let floor_widths = if preferred_min_total <= target_content_width {
        preferred_min_widths
    } else {
        hard_min_widths
    };
    let floor_total: u32 = floor_widths.iter().copied().sum();
    let clamped_target = floor_total.max(target_content_width);

    if total_base_width <= clamped_target {
        return base_widths;
    }

    let shrinkable: Vec<u32> = base_widths
        .iter()
        .zip(floor_widths.iter())
        .map(|(width, floor)| width.saturating_sub(*floor))
        .collect();
    let total_shrinkable: u32 = shrinkable.iter().copied().sum();
    if total_shrinkable == 0 {
        return floor_widths;
    }

    let target_shrink = total_base_width - clamped_target;
    let shrink = allocate_table_shrink_by_weight(&shrinkable, target_shrink, true);

    base_widths
        .iter()
        .zip(floor_widths.iter())
        .zip(shrink.iter())
        .map(|((width, floor), shrink)| floor.max(&width.saturating_sub(*shrink)).to_owned())
        .collect()
}

fn allocate_table_shrink_by_weight(shrinkable: &[u32], target_shrink: u32, sqrt_mode: bool) -> Vec<u32> {
    let mut shrink = vec![0u32; shrinkable.len()];
    if target_shrink == 0 {
        return shrink;
    }

    let weights: Vec<f64> = shrinkable
        .iter()
        .map(|value| {
            if *value == 0 {
                0.0
            } else if sqrt_mode {
                (*value as f64).sqrt()
            } else {
                *value as f64
            }
        })
        .collect();
    let total_weight: f64 = weights.iter().sum();
    if total_weight <= 0.0 {
        return shrink;
    }

    let mut fractions = vec![0f64; shrinkable.len()];
    let mut used_shrink = 0u32;

    for idx in 0..shrinkable.len() {
        if shrinkable[idx] == 0 || weights[idx] <= 0.0 {
            continue;
        }
        let exact = (weights[idx] / total_weight) * target_shrink as f64;
        let whole = shrinkable[idx].min(exact.floor() as u32);
        shrink[idx] = whole;
        fractions[idx] = exact - whole as f64;
        used_shrink += whole;
    }

    let mut remaining_shrink = target_shrink.saturating_sub(used_shrink);
    while remaining_shrink > 0 {
        let mut best_idx = None;
        let mut best_fraction = -1f64;

        for idx in 0..shrinkable.len() {
            if shrinkable[idx].saturating_sub(shrink[idx]) == 0 {
                continue;
            }
            if best_idx.is_none()
                || fractions[idx] > best_fraction
                || (fractions[idx] == best_fraction
                    && shrinkable[idx] > shrinkable[best_idx.unwrap()])
            {
                best_idx = Some(idx);
                best_fraction = fractions[idx];
            }
        }

        let Some(best_idx) = best_idx else {
            break;
        };
        shrink[best_idx] += 1;
        fractions[best_idx] = 0.0;
        remaining_shrink -= 1;
    }

    shrink
}

fn compute_table_row_heights(table: &TextTableMeasure, column_widths: &[u32]) -> Vec<u32> {
    let horizontal_padding = get_horizontal_cell_padding(table);
    let vertical_padding = get_vertical_cell_padding(table);
    let mut row_heights = vec![1 + vertical_padding; table.row_count];

    for row_idx in 0..table.row_count {
        for col_idx in 0..table.column_count {
            let Some(view_ptr) = cell_view_ptr(table, row_idx, col_idx) else {
                continue;
            };
            let view = unsafe { &mut *view_ptr };
            let width = column_widths
                .get(col_idx)
                .copied()
                .unwrap_or(1)
                .saturating_sub(horizontal_padding)
                .max(1);
            let measure = view.measure_for_dimensions(width, 10_000);
            let line_count = measure.line_count.max(1);
            row_heights[row_idx] = row_heights[row_idx].max(line_count.saturating_add(vertical_padding));
        }
    }

    row_heights
}

fn compute_table_offsets(
    parts: &[u32],
    start_boundary: bool,
    end_boundary: bool,
    include_inner_boundaries: bool,
) -> Vec<u32> {
    let mut offsets = vec![if start_boundary { 0 } else { u32::MAX }];
    let mut cursor = *offsets.first().unwrap_or(&0);

    for (idx, size) in parts.iter().enumerate() {
        let has_boundary_after = if idx < parts.len().saturating_sub(1) {
            include_inner_boundaries
        } else {
            end_boundary
        };
        cursor = cursor
            .wrapping_add(*size)
            .wrapping_add(if has_boundary_after { 1 } else { 0 });
        offsets.push(cursor);
    }

    offsets
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
pub extern "C" fn sceneNodeSetVisibleChildren(
    id: u64,
    children_ptr: *const u64,
    child_count: usize,
) -> bool {
    let children = if child_count == 0 || children_ptr.is_null() {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(children_ptr, child_count) }.to_vec()
    };
    scene_graph()
        .lock()
        .unwrap()
        .set_visible_children(id, children)
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
pub extern "C" fn sceneNodeSetTextTableMeasure(
    id: u64,
    config_ptr: *const NativeTextTableMeasureConfig,
    cell_views_ptr: *const u64,
    cell_view_count: usize,
) -> bool {
    if config_ptr.is_null() {
        return false;
    }
    let config = unsafe { std::ptr::read_unaligned(config_ptr) };
    let cell_views = if cell_view_count == 0 || cell_views_ptr.is_null() {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(cell_views_ptr, cell_view_count) }
            .iter()
            .map(|ptr| *ptr as usize)
            .collect()
    };
    scene_graph()
        .lock()
        .unwrap()
        .set_text_table_measure(id, config, cell_views)
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

#[unsafe(no_mangle)]
pub extern "C" fn sceneNodeGetSubtreeNodeCount(id: u64) -> usize {
    scene_graph().lock().unwrap().subtree_node_count(id)
}

#[unsafe(no_mangle)]
pub extern "C" fn sceneNodeGetChildren(id: u64, out_ptr: *mut u64, max_count: usize) -> usize {
    if out_ptr.is_null() || max_count == 0 {
        return 0;
    }

    let graph = scene_graph().lock().unwrap();
    let Some(children) = graph.child_handles(id) else {
        return 0;
    };

    let count = children.len().min(max_count);
    for (index, child) in children.iter().take(count).enumerate() {
        unsafe {
            std::ptr::write_unaligned(out_ptr.add(index), *child);
        }
    }

    count
}

#[unsafe(no_mangle)]
pub extern "C" fn sceneNodeGetChildrenByZIndex(id: u64, out_ptr: *mut u64, max_count: usize) -> usize {
    if out_ptr.is_null() || max_count == 0 {
        return 0;
    }

    let graph = scene_graph().lock().unwrap();
    let Some(children) = graph.child_handles_by_z_index(id) else {
        return 0;
    };

    let count = children.len().min(max_count);
    for (index, child) in children.iter().take(count).enumerate() {
        unsafe {
            std::ptr::write_unaligned(out_ptr.add(index), *child);
        }
    }

    count
}

#[unsafe(no_mangle)]
pub extern "C" fn sceneNodeBuildRenderPlan(
    id: u64,
    out_ptr: *mut NativeSceneRenderCommand,
    max_count: usize,
) -> usize {
    if out_ptr.is_null() || max_count == 0 {
        return 0;
    }

    let mut commands = Vec::new();
    {
        let graph = scene_graph().lock().unwrap();
        if !graph.build_render_plan(id, &mut commands) {
            return 0;
        }
    }

    let count = commands.len().min(max_count);
    for (index, command) in commands.iter().take(count).enumerate() {
        unsafe {
            std::ptr::write_unaligned(out_ptr.add(index), *command);
        }
    }

    count
}

fn intersect_rects(left: ClipRect, right: ClipRect) -> Option<ClipRect> {
    let x1 = left.x.max(right.x);
    let y1 = left.y.max(right.y);
    let x2 = (left.x + left.width as i32).min(right.x + right.width as i32);
    let y2 = (left.y + left.height as i32).min(right.y + right.height as i32);
    if x2 <= x1 || y2 <= y1 {
        return None;
    }

    Some(ClipRect {
        x: x1,
        y: y1,
        width: (x2 - x1) as u32,
        height: (y2 - y1) as u32,
    })
}

fn merge_clip_rects(left: Option<ClipRect>, right: Option<ClipRect>) -> Option<ClipRect> {
    match (left, right) {
        (Some(left), Some(right)) => intersect_rects(left, right).or(Some(ClipRect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        })),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{NativeSceneLayout, NativeSceneStyle, createSceneNode, destroySceneNode, sceneNodeAppendChild, sceneNodeCalculateLayout, sceneNodeGetChildCount, sceneNodeGetLayout, sceneNodeSetStyle};

    #[test]
    fn native_scene_style_abi_size_is_stable() {
        assert_eq!(std::mem::size_of::<NativeSceneStyle>(), 196);
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
