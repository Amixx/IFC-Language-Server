# Current Diagnostics Capabilities

This document summarizes what the current diagnostics provider in `ifc-language-server` can already validate for IFC STEP entity instances.

It is intended as a short showcase document. Screenshots can be added below each section.

## Current Scope

The current implementation validates IFC entity instance arguments against the bundled EXPRESS-derived schema model for:

- IFC 2x3 TC1
- IFC 4 ADD2 TC1
- IFC 4x3 ADD2

Validation is currently:

- single-document only
- schema-aware
- focused on entity instance arguments
- reported as LSP diagnostics on open/change

## 1. Reference Diagnostics

The diagnostics provider checks whether a reference points to an entity that is compatible with the expected schema type.

What is detected:

- unresolved local references such as `#9999` when that entity does not exist in the current file
- references to the wrong entity kind

Example:

```ifc
#10=IFCRELAGGREGATES(...,#20,...);
#20=IFCPERSON(...);
```

If the schema expects a building element or another incompatible entity type, the diagnostic reports that `#20` points to the wrong entity.

Suggested screenshot:

- wrong referenced entity type

## 2. Datatype Diagnostics

The diagnostics provider checks primitive datatype mismatches for attribute values.

What is detected:

- expected string, got number
- expected number, got string
- expected reference, got enumeration/string/number
- expected aggregate, got scalar

Example:

```ifc
#1=IFCWALL(123,.MOVABLE.);
```

If the first attribute expects something string-based such as `IfcLabel`, the diagnostic reports that a string was expected but a number was found.

Suggested screenshot:

- expected string, got number

## 3. Enum Diagnostics

The diagnostics provider validates EXPRESS enumerations.

What is detected:

- invalid enum literal for a given attribute
- correct enum syntax but value not part of the allowed set
- wrong value kind where an enum is expected

Example:

```ifc
#1=IFCWALL('id',.NOT_A_REAL_ENUM_VALUE.);
```

The diagnostic reports the allowed values and the invalid enum literal that was found.

Suggested screenshot:

- expected one of these enum values

## 4. Attribute Count Diagnostics

The diagnostics provider checks the number of supplied arguments against the resolved schema definition of the entity, including inherited attributes.

What is detected:

- too few attributes
- too many attributes

Example:

```ifc
#1=IFCWALL('id');
```

If `IfcWall` expects more attributes in that schema version, the diagnostic reports the expected number and the provided number.

Suggested screenshot:

- entity expects N attributes but found M

## 5. Required Attribute Omission Diagnostics

The diagnostics provider checks whether required attributes are omitted with `$`.

What is detected:

- `$` used for a non-optional attribute

Example:

```ifc
#1=IFCSIUNIT(*,$,.MILLI.,.METRE.);
```

If the corresponding attribute is required, the diagnostic reports that the attribute does not allow `$`.

Suggested screenshot:

- required attribute omitted with `$`

## 6. Omitted `*` Handling

The diagnostics provider distinguishes between valid and invalid uses of `*`.

What is detected:

- invalid `*` usage for ordinary explicit attributes
- valid `*` usage for inherited attributes that are derived in a subtype

Example:

```ifc
#15=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);
```

This is correctly accepted because `IfcSIUnit` derives `IfcNamedUnit.Dimensions`, so the omitted marker is valid there.

Suggested screenshot:

- valid derived attribute omission with `*`

## Additional Diagnostics Already Supported

Beyond the five core showcase cases above, the current implementation also supports:

- syntax diagnostics for invalid IFC STEP syntax detected by the parser
- aggregate cardinality checks such as “expected at least 1 item”
- aggregate type checks such as “expected set/list/array, got scalar”
- `SELECT` validation by checking whether a value matches any allowed branch
- inline typed value validation such as `IFCLABEL('Example')`

These may also be worth demonstrating if you want to show that the provider is already broader than basic primitive checks.

## Current Limitations

The current diagnostics provider does not yet implement:

- evaluation of EXPRESS `WHERE` rules
- cross-file reference validation
- semantic checks beyond the currently modeled argument/type validation
- suggestions or quick fixes
- rich diagnostics for every malformed STEP construct

## Suggested Demo Order

If you want to present the feature set quickly to colleagues, this order should work well:

1. Wrong referenced entity
2. Wrong primitive datatype
3. Invalid enum literal
4. Wrong attribute count
5. Required attribute omitted with `$`
6. Valid `*` on a derived inherited attribute
7. Invalid STEP syntax
