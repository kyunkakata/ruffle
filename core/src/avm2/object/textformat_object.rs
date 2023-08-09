//! Object representation for TextFormat

use crate::avm2::activation::Activation;
use crate::avm2::object::script_object::ScriptObjectData;
use crate::avm2::object::{ClassObject, Object, ObjectPtr, TObject};
use crate::avm2::value::Value;
use crate::avm2::Error;
use crate::html::TextFormat;
use core::fmt;
use gc_arena::{Collect, GcCell, GcWeakCell, MutationContext};
use std::cell::{Ref, RefMut};

/// A class instance allocator that allocates TextFormat objects.
pub fn textformat_allocator<'gc>(
    class: ClassObject<'gc>,
    activation: &mut Activation<'_, 'gc>,
) -> Result<Object<'gc>, Error<'gc>> {
    let base = ScriptObjectData::new(class);

    Ok(TextFormatObject(GcCell::new(
        activation.context.gc_context,
        TextFormatObjectData {
            base,
            text_format: Default::default(),
        },
    ))
    .into())
}

#[derive(Clone, Collect, Copy)]
#[collect(no_drop)]
pub struct TextFormatObject<'gc>(pub GcCell<'gc, TextFormatObjectData<'gc>>);

#[derive(Clone, Collect, Copy, Debug)]
#[collect(no_drop)]
pub struct TextFormatObjectWeak<'gc>(pub GcWeakCell<'gc, TextFormatObjectData<'gc>>);

impl fmt::Debug for TextFormatObject<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextFormatObject")
            .field("ptr", &self.0.as_ptr())
            .finish()
    }
}

#[derive(Clone, Collect)]
#[collect(no_drop)]
pub struct TextFormatObjectData<'gc> {
    /// Base script object
    base: ScriptObjectData<'gc>,

    #[collect(require_static)]
    text_format: TextFormat,
}

impl<'gc> TextFormatObject<'gc> {
    pub fn from_text_format(
        activation: &mut Activation<'_, 'gc>,
        text_format: TextFormat,
    ) -> Result<Object<'gc>, Error<'gc>> {
        let class = activation.avm2().classes().textformat;
        let base = ScriptObjectData::new(class);

        let mut this: Object<'gc> = Self(GcCell::new(
            activation.context.gc_context,
            TextFormatObjectData { base, text_format },
        ))
        .into();
        this.install_instance_slots(activation.context.gc_context);

        Ok(this)
    }
}

impl<'gc> TObject<'gc> for TextFormatObject<'gc> {
    fn base(&self) -> Ref<ScriptObjectData<'gc>> {
        Ref::map(self.0.read(), |read| &read.base)
    }

    fn base_mut(&self, mc: MutationContext<'gc, '_>) -> RefMut<ScriptObjectData<'gc>> {
        RefMut::map(self.0.write(mc), |write| &mut write.base)
    }

    fn as_ptr(&self) -> *const ObjectPtr {
        self.0.as_ptr() as *const ObjectPtr
    }

    fn value_of(&self, _mc: MutationContext<'gc, '_>) -> Result<Value<'gc>, Error<'gc>> {
        Ok(Value::Object(Object::from(*self)))
    }

    /// Unwrap this object as a text format.
    fn as_text_format(&self) -> Option<Ref<TextFormat>> {
        Some(Ref::map(self.0.read(), |d| &d.text_format))
    }

    /// Unwrap this object as a mutable text format.
    fn as_text_format_mut(&self, mc: MutationContext<'gc, '_>) -> Option<RefMut<TextFormat>> {
        Some(RefMut::map(self.0.write(mc), |d| &mut d.text_format))
    }
}
