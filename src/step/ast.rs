//! Tree-sitter helpers for locating STEP instance and parameter context around a cursor node.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OmittedValueContext {
    pub instance_id: u32,
    pub entity_name: String,
    pub parameter_index: usize,
}

pub fn omitted_value_context(
    node: tree_sitter::Node<'_>,
    text: &str,
) -> Option<OmittedValueContext> {
    let parameter = ancestor_with_kind(node, "parameter")?;
    let parameter_sequence = ancestor_with_kind(parameter, "parameter_sequence")?;
    let entity_instance = ancestor_with_kind(parameter_sequence, "entity_instance")?;

    Some(OmittedValueContext {
        instance_id: parse_instance_id(entity_instance, text)?,
        entity_name: parse_entity_name(entity_instance, text)?,
        parameter_index: parameter_index(parameter_sequence, parameter)?,
    })
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

fn parameter_index(
    parameter_sequence: tree_sitter::Node<'_>,
    parameter: tree_sitter::Node<'_>,
) -> Option<usize> {
    let mut cursor = parameter_sequence.walk();
    parameter_sequence
        .children(&mut cursor)
        .filter(|child| child.kind() == "parameter")
        .position(|child| same_node(child, parameter))
}

fn same_node(a: tree_sitter::Node<'_>, b: tree_sitter::Node<'_>) -> bool {
    a.start_byte() == b.start_byte() && a.end_byte() == b.end_byte()
}
