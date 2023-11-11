use crate::avm2::api_version::ApiVersion;
use crate::avm2::bytearray::ByteArrayStorage;
use crate::avm2::object::{ByteArrayObject, TObject, VectorObject};
use crate::avm2::vector::VectorStorage;
use crate::avm2::ArrayObject;
use crate::avm2::ArrayStorage;
use crate::avm2::{Activation, Error, Object, Value};
use crate::string::AvmString;
use enumset::EnumSet;
use flash_lso::types::{AMFVersion, Element, Lso};
use flash_lso::types::{Attribute, ClassDefinition, Value as AmfValue};

use super::property::Property;
use super::{ClassObject, Namespace, QName};

/// Serialize a Value to an AmfValue
pub fn serialize_value<'gc>(
    activation: &mut Activation<'_, 'gc>,
    elem: Value<'gc>,
    amf_version: AMFVersion,
) -> Option<AmfValue> {
    match elem {
        Value::Undefined => Some(AmfValue::Undefined),
        Value::Null => Some(AmfValue::Null),
        Value::Bool(b) => Some(AmfValue::Bool(b)),
        Value::Number(f) => Some(AmfValue::Number(f)),
        Value::Integer(num) => {
            // NOTE - we should really be converting `Value::Integer` to `Value::Number`
            // whenever it's outside this range, instead of performing this during AMF serialization.
            // Integers are unsupported in AMF0, and must be converted to Number regardless of whether
            // it can be represented as an integer.
            // FIXME - handle coercion floats like '1.0' to integers
            if amf_version == AMFVersion::AMF0 || num >= (1 << 28) || num < -(1 << 28) {
                Some(AmfValue::Number(num as f64))
            } else {
                Some(AmfValue::Integer(num))
            }
        }
        Value::String(s) => Some(AmfValue::String(s.to_string())),
        Value::Object(o) => {
            // TODO: Find a more general rule for which object types should be skipped,
            // and which turn into undefined.
            if o.as_executable().is_some() {
                None
            } else if o.as_display_object().is_some() {
                Some(AmfValue::Undefined)
            } else if o.as_array_storage().is_some() {
                let mut values = Vec::new();
                // Don't serialize properties from the vtable (we don't want a 'length' field)
                recursive_serialize(activation, o, &mut values, None, amf_version).unwrap();

                let mut dense = vec![];
                let mut sparse = vec![];
                // ActionScript `Array`s can have non-number properties, and these properties
                // are confirmed and tested to also be serialized, so do not limit the values
                // iterated over by the length of the internal array data.
                for (i, elem) in values.into_iter().enumerate() {
                    if elem.name == i.to_string() {
                        dense.push(elem.value.clone());
                    } else {
                        sparse.push(elem);
                    }
                }

                if sparse.is_empty() {
                    Some(AmfValue::StrictArray(dense))
                } else {
                    let len = sparse.len() as u32;
                    Some(AmfValue::ECMAArray(dense, sparse, len))
                }
            } else if let Some(vec) = o.as_vector_storage() {
                let val_type = vec.value_type();
                if val_type == Some(activation.avm2().classes().int) {
                    let int_vec: Vec<_> = vec
                        .iter()
                        .map(|v| {
                            v.as_integer(activation.context.gc_context)
                                .expect("Unexpected non-int value in int vector")
                        })
                        .collect();
                    Some(AmfValue::VectorInt(int_vec, vec.is_fixed()))
                } else if val_type == Some(activation.avm2().classes().uint) {
                    let uint_vec: Vec<_> = vec
                        .iter()
                        .map(|v| {
                            v.as_u32(activation.context.gc_context)
                                .expect("Unexpected non-uint value in int vector")
                        })
                        .collect();
                    Some(AmfValue::VectorUInt(uint_vec, vec.is_fixed()))
                } else if val_type == Some(activation.avm2().classes().number) {
                    let num_vec: Vec<_> = vec
                        .iter()
                        .map(|v| {
                            v.as_number(activation.context.gc_context)
                                .expect("Unexpected non-uint value in int vector")
                        })
                        .collect();
                    Some(AmfValue::VectorDouble(num_vec, vec.is_fixed()))
                } else {
                    let obj_vec: Vec<_> = vec
                        .iter()
                        .map(|v| {
                            serialize_value(activation, v, amf_version)
                                .expect("Unexpected non-object value in object vector")
                        })
                        .collect();

                    let val_type = val_type.unwrap_or(activation.avm2().classes().object);

                    let name = class_to_alias(activation, val_type);
                    Some(AmfValue::VectorObject(obj_vec, name, vec.is_fixed()))
                }
            } else if let Some(date) = o.as_date_object() {
                date.date_time()
                    .map(|date_time| AmfValue::Date(date_time.timestamp_millis() as f64, None))
            } else if let Some(xml) = o.as_xml_object() {
                // `is_string` is `true` for the AS3 XML class
                Some(AmfValue::XML(
                    xml.node().xml_to_xml_string(activation).to_string(),
                    true,
                ))
            } else if let Some(bytearray) = o.as_bytearray() {
                Some(AmfValue::ByteArray(bytearray.bytes().to_vec()))
            } else {
                let class = o.instance_of().expect("Missing ClassObject");
                let name = class_to_alias(activation, class);

                let mut attributes = EnumSet::empty();
                if !class.inner_class_definition().read().is_sealed() {
                    attributes.insert(Attribute::Dynamic);
                }

                let mut object_body = Vec::new();
                let mut static_properties = Vec::new();
                recursive_serialize(
                    activation,
                    o,
                    &mut object_body,
                    Some(&mut static_properties),
                    amf_version,
                )
                .unwrap();
                Some(AmfValue::Object(
                    object_body,
                    if amf_version == AMFVersion::AMF3 {
                        Some(ClassDefinition {
                            name,
                            attributes,
                            // FIXME - implement this
                            static_properties,
                        })
                    } else {
                        None
                    },
                ))
            }
        }
    }
}

fn alias_to_class<'gc>(
    activation: &mut Activation<'_, 'gc>,
    alias: AvmString<'gc>,
) -> Result<ClassObject<'gc>, Error<'gc>> {
    let mut target_class = activation.avm2().classes().object;
    let ns = Namespace::package(
        "flash.net",
        ApiVersion::AllVersions,
        &mut activation.context.borrow_gc(),
    );
    let method = activation
        .avm2()
        .playerglobals_domain
        .get_defined_value(activation, QName::new(ns, "getClassByAlias"))?;

    let class = method
        .as_object()
        .unwrap()
        .as_function_object()
        .unwrap()
        .call(Value::Undefined, &[alias.into()], activation)?;
    if let Some(class_obj) = class.as_object().and_then(|o| o.as_class_object()) {
        target_class = class_obj;
    }
    Ok(target_class)
}

fn class_to_alias<'gc>(activation: &mut Activation<'_, 'gc>, class: ClassObject<'gc>) -> String {
    let qname = QName::new(activation.avm2().flash_net_internal, "_getAliasByClass");
    let method = activation
        .avm2()
        .playerglobals_domain
        .get_defined_value(activation, qname)
        .expect("Failed to lookup flash.net._getAliasByClass");

    let alias = method
        .as_object()
        .unwrap()
        .as_function_object()
        .unwrap()
        .call(Value::Undefined, &[class.into()], activation)
        .expect("Failed to call flash.net._getAliasByClass");

    if let Value::Null = alias {
        "".to_string()
    } else {
        alias.coerce_to_string(activation).unwrap().to_string()
    }
}

/// Serialize an Object and any children to a AMF object
pub fn recursive_serialize<'gc>(
    activation: &mut Activation<'_, 'gc>,
    obj: Object<'gc>,
    elements: &mut Vec<Element>,
    static_properties: Option<&mut Vec<String>>,
    amf_version: AMFVersion,
) -> Result<(), Error<'gc>> {
    if let Some(static_properties) = static_properties {
        if let Some(vtable) = obj.vtable() {
            let mut props = vtable.public_properties();
            // Flash appears to use vtable iteration order, but we sort ours
            // to make our test output consistent.
            props.sort_by_key(|(name, _)| name.to_utf8_lossy().to_string());
            for (name, prop) in props {
                if let Property::Virtual { get, set } = prop {
                    if !(get.is_some() && set.is_some()) {
                        continue;
                    }
                }
                let value = obj.get_public_property(name, activation)?;
                if let Some(value) = serialize_value(activation, value, amf_version) {
                    let name = name.to_utf8_lossy().to_string();
                    elements.push(Element::new(name.clone(), value));
                    static_properties.push(name);
                }
            }
        }
    }

    let mut last_index = obj.get_next_enumerant(0, activation)?;
    while let Some(index) = last_index {
        let name = obj
            .get_enumerant_name(index, activation)?
            .coerce_to_string(activation)?;
        let value = obj.get_public_property(name, activation)?;

        if let Some(value) = serialize_value(activation, value, amf_version) {
            elements.push(Element::new(name.to_utf8_lossy(), value));
        }
        last_index = obj.get_next_enumerant(index, activation)?;
    }
    Ok(())
}

/// Deserialize a AmfValue to a Value
pub fn deserialize_value<'gc>(
    activation: &mut Activation<'_, 'gc>,
    val: &AmfValue,
) -> Result<Value<'gc>, Error<'gc>> {
    Ok(match val {
        AmfValue::Null => Value::Null,
        AmfValue::Undefined => Value::Undefined,
        AmfValue::Number(f) => (*f).into(),
        AmfValue::Integer(num) => (*num).into(),
        AmfValue::String(s) => Value::String(AvmString::new_utf8(activation.context.gc_context, s)),
        AmfValue::Bool(b) => (*b).into(),
        AmfValue::ByteArray(bytes) => {
            let storage = ByteArrayStorage::from_vec(bytes.clone());
            let bytearray = ByteArrayObject::from_storage(activation, storage)?;
            bytearray.into()
        }
        AmfValue::ECMAArray(values, elements, _) => {
            // First let's create an array out of `values` (dense portion), then we add the elements onto it.
            let mut arr: Vec<Option<Value<'gc>>> = Vec::with_capacity(values.len());
            for value in values {
                arr.push(Some(deserialize_value(activation, value)?));
            }
            let storage = ArrayStorage::from_storage(arr);
            let array = ArrayObject::from_storage(activation, storage)?;
            // Now let's add each element as a property
            for element in elements {
                array.set_public_property(
                    AvmString::new_utf8(activation.context.gc_context, element.name()),
                    deserialize_value(activation, element.value())?,
                    activation,
                )?;
            }
            array.into()
        }
        AmfValue::StrictArray(values) => {
            let mut arr: Vec<Option<Value<'gc>>> = Vec::with_capacity(values.len());
            for value in values {
                arr.push(Some(deserialize_value(activation, value)?));
            }
            let storage = ArrayStorage::from_storage(arr);
            let array = ArrayObject::from_storage(activation, storage)?;
            array.into()
        }
        AmfValue::Object(elements, class) => {
            let target_class = if let Some(class) = class {
                let name = AvmString::new_utf8(activation.context.gc_context, &class.name);
                alias_to_class(activation, name)?
            } else {
                activation.avm2().classes().object
            };
            let obj = target_class.construct(activation, &[])?;

            for entry in elements {
                let value = deserialize_value(activation, entry.value())?;
                obj.set_public_property(
                    AvmString::new_utf8(activation.context.gc_context, entry.name()),
                    value,
                    activation,
                )?;
            }
            obj.into()
        }
        AmfValue::Date(time, _) => activation
            .avm2()
            .classes()
            .date
            .construct(activation, &[(*time).into()])?
            .into(),
        AmfValue::XML(content, _) => activation
            .avm2()
            .classes()
            .xml
            .construct(
                activation,
                &[Value::String(AvmString::new_utf8(
                    activation.context.gc_context,
                    content,
                ))],
            )?
            .into(),
        AmfValue::VectorDouble(vec, is_fixed) => {
            let storage = VectorStorage::from_values(
                vec.iter().map(|v| (*v).into()).collect(),
                *is_fixed,
                Some(activation.avm2().classes().number),
            );
            VectorObject::from_vector(storage, activation)?.into()
        }
        AmfValue::VectorUInt(vec, is_fixed) => {
            let storage = VectorStorage::from_values(
                vec.iter().map(|v| (*v).into()).collect(),
                *is_fixed,
                Some(activation.avm2().classes().uint),
            );
            VectorObject::from_vector(storage, activation)?.into()
        }
        AmfValue::VectorInt(vec, is_fixed) => {
            let storage = VectorStorage::from_values(
                vec.iter().map(|v| (*v).into()).collect(),
                *is_fixed,
                Some(activation.avm2().classes().int),
            );
            VectorObject::from_vector(storage, activation)?.into()
        }
        AmfValue::VectorObject(vec, ty_name, is_fixed) => {
            let name = AvmString::new_utf8(activation.context.gc_context, ty_name);
            let class = alias_to_class(activation, name)?;
            let storage = VectorStorage::from_values(
                vec.iter()
                    .map(|v| deserialize_value(activation, v))
                    .collect::<Result<Vec<_>, _>>()?,
                *is_fixed,
                Some(class),
            );
            VectorObject::from_vector(storage, activation)?.into()
        }
        AmfValue::Dictionary(..) | AmfValue::Custom(..) | AmfValue::Reference(_) => {
            tracing::error!("Deserialization not yet implemented: {:?}", val);
            Value::Undefined
        }
        AmfValue::AMF3(val) => deserialize_value(activation, val)?,
        AmfValue::Unsupported => Value::Undefined,
    })
}

/// Deserializes a Lso into an object containing the properties stored
pub fn deserialize_lso<'gc>(
    activation: &mut Activation<'_, 'gc>,
    lso: &Lso,
) -> Result<Object<'gc>, Error<'gc>> {
    let obj = activation
        .avm2()
        .classes()
        .object
        .construct(activation, &[])?;

    for child in &lso.body {
        obj.set_public_property(
            AvmString::new_utf8(activation.context.gc_context, &child.name),
            deserialize_value(activation, child.value())?,
            activation,
        )?;
    }

    Ok(obj)
}
