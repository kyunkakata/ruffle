use crate::avm2::Activation;
use crate::avm2::TObject;
use crate::avm2::Value;
use crate::avm2::{Error, Object};

pub fn upload_from_bitmap_data<'gc>(
    activation: &mut Activation<'_, 'gc>,
    this: Option<Object<'gc>>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    if let Some(texture) = this.and_then(|this| this.as_texture()) {
        if let Some(source) = args[0].coerce_to_object(activation)?.as_bitmap_data() {
            texture.context3d().copy_bitmap_to_texture(
                source.bitmap_handle(activation.context.gc_context, activation.context.renderer),
                texture.handle(),
                0,
                activation,
            );
        } else {
            panic!("Invalid source: {:?}", args[0]);
        }
    }
    Ok(Value::Undefined)
}
