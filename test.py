import ifcopenshell as ios
from pprint import pprint


# IfcOpenShell uses -1 for "unbounded" which is represented as "?" in express.
def format_bound(bound):
    if bound < 0:
        return "?"
    return str(bound)


def determine_type(type_decl):
    if isinstance(type_decl, str):
        return type_decl.upper()

    # Handle LIST and SET
    aggregate = getattr(type_decl, "as_aggregation_type", lambda: None)()
    if aggregate is not None:
        kind = aggregate.type_of_aggregation_string().upper()
        lower = format_bound(aggregate.bound1())
        upper = format_bound(aggregate.bound2())
        element_type = determine_type(aggregate.type_of_element())
        return f"{kind} [{lower}:{upper}] OF {element_type}"

    if hasattr(type_decl, "declared_type") and not hasattr(type_decl, "name"):
        return determine_type(type_decl.declared_type())

    if hasattr(type_decl, "name"):
        return type_decl.name()

    raise TypeError(f"unsupported type declaration: {type_decl!r}")


schema = ios.schema_by_name("IFC2X3")
entities = schema.entities()
for entity in entities:
    all_attributes = entity.all_attributes()
    attributes = [
        {
            "name": attribute.name(),
            "type": determine_type(attribute.type_of_attribute()),
        }
        for attribute in all_attributes
    ]
    for attribute in all_attributes:
        attributes.append(
            {
                "name": attribute.name(),
                "type": determine_type(attribute.type_of_attribute()),
            }
        )
    pprint(attributes)
