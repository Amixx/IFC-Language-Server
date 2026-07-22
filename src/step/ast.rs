//! Tree-sitter helpers for locating STEP instance and parameter context around a cursor node.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterContext {
    pub instance_id: u32,
    pub entity_name: String,
    pub parameter_index: usize,
}

pub fn parameter_context(node: tree_sitter::Node<'_>, text: &str) -> Option<ParameterContext> {
    let entity_instance = ancestor_with_kind(node, "entity_instance")?;
    let parameter_sequence = entity_parameter_sequence(entity_instance)?;
    let parameter_index = parameter_sequence
        .named_children(&mut parameter_sequence.walk())
        .filter(|child| child.kind() == "parameter")
        .position(|parameter| contains_node(parameter, node))?;

    Some(ParameterContext {
        instance_id: parse_instance_id(entity_instance, text)?,
        entity_name: parse_entity_name(entity_instance, text)?,
        parameter_index,
    })
}

pub fn omitted_value_context(node: tree_sitter::Node<'_>, text: &str) -> Option<ParameterContext> {
    parameter_context(node, text)
}

fn ancestor_with_kind<'tree>(
    mut node: tree_sitter::Node<'tree>,
    expected_kind: &str,
) -> Option<tree_sitter::Node<'tree>> {
    loop {
        if node.kind() == expected_kind {
            return Some(node);
        }
        node = node.parent()?;
    }
}

fn parse_instance_id(node: tree_sitter::Node<'_>, text: &str) -> Option<u32> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == "instance_id")
        .and_then(|child| child.utf8_text(text.as_bytes()).ok())
        .and_then(|value| value.trim_start_matches('#').parse::<u32>().ok())
}

fn parse_entity_name(node: tree_sitter::Node<'_>, text: &str) -> Option<String> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == "entity_name")
        .and_then(|child| child.utf8_text(text.as_bytes()).ok())
        .map(|value| value.to_ascii_uppercase())
}

fn entity_parameter_sequence(
    entity_instance: tree_sitter::Node<'_>,
) -> Option<tree_sitter::Node<'_>> {
    let mut cursor = entity_instance.walk();
    let parameter_list = entity_instance
        .named_children(&mut cursor)
        .find(|child| child.kind() == "parameter_list")?;
    let mut cursor = parameter_list.walk();
    parameter_list
        .named_children(&mut cursor)
        .find(|child| child.kind() == "parameter_sequence")
}

fn contains_node(outer: tree_sitter::Node<'_>, inner: tree_sitter::Node<'_>) -> bool {
    outer.start_byte() <= inner.start_byte() && inner.end_byte() <= outer.end_byte()
}
