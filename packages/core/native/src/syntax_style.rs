use std::collections::HashMap;

pub type Rgba = [f32; 4];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StyleDefinition {
    pub fg: Option<Rgba>,
    pub bg: Option<Rgba>,
    pub attributes: u32,
}

pub struct SyntaxStyleState {
    name_to_id: HashMap<Vec<u8>, u32>,
    id_to_style: HashMap<u32, StyleDefinition>,
    next_id: u32,
}

impl Default for SyntaxStyleState {
    fn default() -> Self {
        Self {
            name_to_id: HashMap::new(),
            id_to_style: HashMap::new(),
            next_id: 1,
        }
    }
}

impl SyntaxStyleState {
    pub fn register_style(
        &mut self,
        name: &[u8],
        fg: Option<Rgba>,
        bg: Option<Rgba>,
        attributes: u32,
    ) -> u32 {
        if let Some(existing_id) = self.name_to_id.get(name) {
            self.id_to_style
                .insert(*existing_id, StyleDefinition { fg, bg, attributes });
            return *existing_id;
        }

        let id = self.next_id;
        self.next_id += 1;

        self.name_to_id.insert(name.to_vec(), id);
        self.id_to_style
            .insert(id, StyleDefinition { fg, bg, attributes });

        id
    }

    pub fn resolve_by_name(&self, name: &[u8]) -> Option<u32> {
        self.name_to_id.get(name).copied()
    }

    pub fn resolve_by_id(&self, id: u32) -> Option<StyleDefinition> {
        self.id_to_style.get(&id).copied()
    }

    pub fn resolve_by_definition(
        &self,
        fg: Option<Rgba>,
        bg: Option<Rgba>,
        attributes: u32,
    ) -> Option<u32> {
        self.id_to_style.iter().find_map(|(id, style)| {
            (style.fg == fg && style.bg == bg && style.attributes == attributes).then_some(*id)
        })
    }

    pub fn style_count(&self) -> usize {
        self.id_to_style.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{Rgba, SyntaxStyleState};

    fn rgba(r: f32, g: f32, b: f32, a: f32) -> Rgba {
        [r, g, b, a]
    }

    #[test]
    fn register_style_assigns_incrementing_ids() {
        let mut state = SyntaxStyleState::default();

        let keyword = state.register_style(b"keyword", Some(rgba(1.0, 0.0, 0.0, 1.0)), None, 0);
        let string = state.register_style(b"string", Some(rgba(0.0, 1.0, 0.0, 1.0)), None, 0);

        assert_eq!(keyword, 1);
        assert_eq!(string, 2);
        assert_eq!(state.style_count(), 2);
    }

    #[test]
    fn register_style_reuses_existing_id_for_same_name() {
        let mut state = SyntaxStyleState::default();

        let first = state.register_style(b"keyword", Some(rgba(1.0, 0.0, 0.0, 1.0)), None, 1);
        let second = state.register_style(b"keyword", Some(rgba(0.0, 1.0, 0.0, 1.0)), None, 2);

        assert_eq!(first, second);
        assert_eq!(state.style_count(), 1);
    }

    #[test]
    fn resolve_by_name_is_byte_exact() {
        let mut state = SyntaxStyleState::default();
        let style_id = state.register_style("关键字".as_bytes(), None, None, 0);

        assert_eq!(state.resolve_by_name("关键字".as_bytes()), Some(style_id));
        assert_eq!(state.resolve_by_name("关键".as_bytes()), None);
        assert_eq!(state.resolve_by_name("Keyword".as_bytes()), None);
    }
}
