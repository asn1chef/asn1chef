use num::{bigint::Sign, BigInt, BigUint};

use crate::{
    compiler::{context::DeclaredValue, parser::*},
    module::*,
    types::*,
    values::*,
};

use super::{object_id, types, AstParser};

pub fn parse_valuereference(
    parser: &AstParser<'_>,
    valref: &AstElement<AstValueReference>,
) -> AstElement<Value> {
    valref.as_ref().map(|valref| {
        Value::Reference(
            parser
                .context
                .lookup_module(&parser.module)
                .expect("lookup_module")
                .resolve_symbol(&valref.0),
        )
    })
}

pub fn parse_integer_value(num: &AstElement<AstIntegerValue>) -> Result<BuiltinValue> {
    if num.element.sign.is_none() {
        let uint = num.element.value.element.0;
        if uint > i64::MAX as u64 {
            return Err(Error {
                kind: ErrorKind::Ast(format!(
                    "number {} too large for a 64-bit signed integer",
                    uint,
                )),
                loc: num.loc,
            });
        }
        Ok(BuiltinValue::Integer(BigInt::from_biguint(
            Sign::Plus,
            BigUint::from(uint),
        )))
    } else {
        let int = num.element.value.element.0;
        if int > i64::MIN as u64 {
            return Err(Error {
                kind: ErrorKind::Ast(format!(
                    "number -{} too small for a 64-bit signed integer",
                    int,
                )),
                loc: num.loc,
            });
        }
        Ok(BuiltinValue::Integer(BigInt::from_biguint(
            Sign::Minus,
            BigUint::from(int),
        )))
    }
}

fn parse_structure_value(
    parser: &AstParser<'_>,
    struct_val: &AstElement<AstStructureValue>,
    target_type: &ResolvedType,
) -> Result<BuiltinValue> {
    let tag_type = target_type.ty.tag_type().expect("tag_type");
    let struct_ty_components = match &target_type.ty {
        BuiltinType::Structure(ty) => &ty.components,
        ty => {
            return Err(Error {
                kind: ErrorKind::Ast(format!("SEQUENCE value cannot be assigned to {} type", ty)),
                loc: struct_val.loc,
            })
        }
    };
    for val_component in &struct_val.element.components {
        if !struct_ty_components
            .iter()
            .any(|ty_component| ty_component.name.element == val_component.element.name.element.0)
        {
            return Err(Error {
                kind: ErrorKind::Ast(format!(
                    "no such component '{}' in SEQUENCE type",
                    val_component.element.name.element.0
                )),
                loc: val_component.element.name.loc,
            });
        }
    }
    let mut components = Vec::new();
    for ty_component in struct_ty_components {
        let (value, is_default) = {
            if let Some(val_component) =
                struct_val.element.components.iter().find(|val_component| {
                    val_component.element.name.element.0 == ty_component.name.element
                })
            {
                let component_type = ty_component.component_type.resolve(parser.context)?;
                (
                    parse_value(parser, &val_component.element.value, &component_type)?,
                    false,
                )
            } else if let Some(default_value) = &ty_component.default_value {
                ((**default_value).clone(), true)
            } else {
                if ty_component.optional {
                    continue;
                }
                return Err(Error {
                    kind: ErrorKind::Ast(format!(
                        "{} value missing component '{}' of type '{}'",
                        tag_type, ty_component.name.element, ty_component.component_type,
                    )),
                    loc: struct_val.loc,
                });
            }
        };

        components.push(StructureValueComponent {
            name: ty_component.name.clone(),
            value,
            is_default,
        });
    }
    let value = StructureValue { components };
    Ok(match tag_type {
        TagType::Sequence => BuiltinValue::Sequence(value),
        TagType::Set => BuiltinValue::Set(value),
        _ => unreachable!(),
    })
}

fn parse_character_string(
    str_lit: &AstElement<AstStringLiteral>,
    tag_type: TagType,
) -> Result<BuiltinValue> {
    let cstring = match &str_lit.element.kind {
        StringKind::CString => str_lit.element.data.clone(),
        _ => {
            return Err(Error {
                kind: ErrorKind::Ast(format!(
                    "{} value cannot be assigned to {}",
                    str_lit.element.kind, tag_type
                )),
                loc: str_lit.loc,
            });
        }
    };
    let validator = match tag_type {
        TagType::UTF8String
        | TagType::UniversalString
        | TagType::GeneralString
        | TagType::BMPString
        | TagType::CharacterString => |_: char| true,
        TagType::NumericString => |ch: char| ch.is_ascii_digit() || ch == ' ',
        TagType::PrintableString => {
            |ch: char| ch.is_ascii_alphanumeric() || " '()+,-./:=?".contains(ch)
        }
        TagType::TeletexString | TagType::VideotexString => {
            |ch: char| ch.is_ascii_graphic() || ch == ' ' || ch == '\x7f'
        }
        TagType::VisibleString => |ch: char| ch.is_ascii_graphic() || ch == ' ',
        TagType::IA5String => |ch: char| ch <= '\x7f',
        TagType::GraphicString | TagType::ObjectDescriptor => {
            |ch: char| ch == ' ' || ch.is_alphanumeric()
        }
        _ => unreachable!(),
    };
    let invalid = cstring.chars().map(validator).any(|valid| !valid);
    if invalid {
        return Err(Error {
            kind: ErrorKind::Ast(format!(
                "provided cstring does not meet the character constraints for {}",
                tag_type
            )),
            loc: str_lit.loc,
        });
    }
    Ok(BuiltinValue::CharacterString(tag_type, cstring))
}

pub fn parse_value(
    parser: &AstParser<'_>,
    value: &AstElement<AstValue>,
    target_type: &ResolvedType,
) -> Result<AstElement<Value>> {
    Ok(AstElement::new(
        match &value.element {
            AstValue::BuiltinValue(builtin) => Value::BuiltinValue(match &builtin.element {
                AstBuiltinValue::Null(_) => BuiltinValue::Null,
                AstBuiltinValue::BooleanValue(b) => match b.element {
                    AstBooleanValue::True(_) => BuiltinValue::Boolean(true),
                    AstBooleanValue::False(_) => BuiltinValue::Boolean(false),
                },
                AstBuiltinValue::StringLiteral(str_lit) => match &target_type.ty {
                    BuiltinType::BitString(_) => {
                        let radix = match str_lit.element.kind {
                            StringKind::BString => 2,
                            StringKind::HString => 16,
                            StringKind::CString => {
                                return Err(Error {
                                    kind: ErrorKind::Ast(
                                        "cstring value cannot be assigned to BIT STRING"
                                            .to_string(),
                                    ),
                                    loc: builtin.loc,
                                })
                            }
                        };
                        BuiltinValue::BitString(
                            BigUint::parse_bytes(str_lit.element.data.as_bytes(), radix)
                                .expect("failed BigUint::parse_bytes"),
                        )
                    }
                    BuiltinType::OctetString(_) => {
                        let radix = match str_lit.element.kind {
                            StringKind::BString => 2,
                            StringKind::HString => 16,
                            StringKind::CString => {
                                return Err(Error {
                                    kind: ErrorKind::Ast(
                                        "cstring value cannot be assigned to OCTET STRING"
                                            .to_string(),
                                    ),
                                    loc: builtin.loc,
                                })
                            }
                        };
                        BuiltinValue::OctetString(
                            BigUint::parse_bytes(str_lit.element.data.as_bytes(), radix)
                                .expect("failed BigUint::parse_bytes")
                                .to_bytes_be(),
                        )
                    }
                    BuiltinType::CharacterString(tag_type) => {
                        parse_character_string(str_lit, *tag_type)?
                    }
                    other_type => {
                        return Err(Error {
                            kind: ErrorKind::Ast(format!(
                                "{} value cannot be assigned to {}",
                                str_lit.element.kind, other_type,
                            )),
                            loc: builtin.loc,
                        })
                    }
                },
                AstBuiltinValue::ObjectIdentifierValue(object_id) => {
                    BuiltinValue::ObjectIdentifier(object_id::parse_object_identifier(
                        parser, object_id,
                    )?)
                }
                AstBuiltinValue::IntegerValue(num) => parse_integer_value(num)?,
                AstBuiltinValue::StructureValue(seq_val) => {
                    parse_structure_value(parser, seq_val, target_type)?
                }
                AstBuiltinValue::StructureOfValue(of_val) => {
                    let component_type = match &target_type.ty {
                        BuiltinType::StructureOf(seq_of) => &seq_of.component_type,
                        other_type => {
                            return Err(Error {
                                kind: ErrorKind::Ast(format!(
                                    "SEQUENCE OF/SET OF value cannot be assigned to {}",
                                    other_type,
                                )),
                                loc: builtin.loc,
                            })
                        }
                    };
                    let component_type = component_type.resolve(parser.context)?;

                    let ast_elements = &of_val.element.elements;
                    let mut elements = Vec::with_capacity(ast_elements.len());
                    for ast_element in ast_elements {
                        elements.push(parse_value(parser, ast_element, &component_type)?);
                    }
                    BuiltinValue::SequenceOf(elements)
                }
                other_builtin => todo!("{:#?}", other_builtin),
            }),
            AstValue::ValueReference(valref) => match &target_type.ty {
                BuiltinType::Enumerated(items) => 'block: {
                    for item in items {
                        if item.name.element == valref.element.0 {
                            break 'block Value::BuiltinValue(BuiltinValue::Enumerated(Box::new(
                                match &item.value {
                                    EnumerationItemValue::Implied(implied) => AstElement::new(
                                        Value::BuiltinValue(BuiltinValue::Integer(BigInt::from(
                                            *implied,
                                        ))),
                                        value.loc,
                                    ),
                                    EnumerationItemValue::Specified(specified) => specified.clone(),
                                },
                            )));
                        }
                    }

                    parse_valuereference(parser, valref).element
                }
                _ => parse_valuereference(parser, valref).element,
            },
        },
        value.loc,
    ))
}

pub fn parse_value_assignment(
    parser: &AstParser<'_>,
    value_assignment: &AstElement<AstValueAssignment>,
) -> Result<(QualifiedIdentifier, DeclaredValue)> {
    let name = value_assignment.element.name.element.0.clone();
    let ty = types::parse_type(
        parser,
        &value_assignment.element.ty,
        types::TypeContext::Contextless,
    )?;
    let resolved_ty = ty.resolve(parser.context)?;
    let val = parse_value(parser, &value_assignment.element.value, &resolved_ty)?;

    Ok((
        QualifiedIdentifier {
            module: parser.module.clone(),
            name,
        },
        DeclaredValue { value: val, ty },
    ))
}
