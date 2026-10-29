#![allow(unsafe_code)]

use jni::{
    Env, JavaVM, jni_sig, jni_str,
    objects::{JObject, JString, JValue},
};
use ndk_context::android_context;

fn with_clipboard<T>(
    operation: impl FnOnce(&mut Env<'_>, &JObject<'_>, &JObject<'_>) -> jni::errors::Result<T>,
) -> Result<T, String> {
    let context = android_context();
    let vm = JavaVM::singleton().map_err(|error| format!("get Android VM: {error}"))?;
    vm.attach_current_thread_for_scope(|env| {
        // ndk-context keeps this Android Context as a global JNI reference for
        // the lifetime of the NativeActivity.
        let context = unsafe { JObject::from_raw(env, context.context().cast()) };
        let service_name = JObject::from(env.new_string("clipboard")?);
        let manager = env
            .call_method(
                &context,
                jni_str!("getSystemService"),
                jni_sig!("(Ljava/lang/String;)Ljava/lang/Object;"),
                &[JValue::Object(&service_name)],
            )?
            .l()?;
        operation(env, &context, &manager)
    })
    .map_err(|error| format!("Android clipboard operation failed: {error}"))
}

pub(super) fn get_text() -> Result<Option<String>, String> {
    with_clipboard(|env, context, manager| {
        let clip = env
            .call_method(
                manager,
                jni_str!("getPrimaryClip"),
                jni_sig!("()Landroid/content/ClipData;"),
                &[],
            )?
            .l()?;
        if clip.is_null()
            || env
                .call_method(&clip, jni_str!("getItemCount"), jni_sig!("()I"), &[])?
                .i()?
                == 0
        {
            return Ok(None);
        }
        let item = env
            .call_method(
                &clip,
                jni_str!("getItemAt"),
                jni_sig!("(I)Landroid/content/ClipData$Item;"),
                &[JValue::Int(0)],
            )?
            .l()?;
        let text = env
            .call_method(
                &item,
                jni_str!("coerceToText"),
                jni_sig!("(Landroid/content/Context;)Ljava/lang/CharSequence;"),
                &[JValue::Object(context)],
            )?
            .l()?;
        if text.is_null() {
            return Ok(None);
        }
        let value = env
            .call_method(
                &text,
                jni_str!("toString"),
                jni_sig!("()Ljava/lang/String;"),
                &[],
            )?
            .l()?;
        let value = env.cast_local::<JString>(value)?;
        Ok(Some(value.try_to_string(env)?))
    })
}

pub(super) fn set_text(text: &str) -> Result<(), String> {
    with_clipboard(|env, _context, manager| {
        let label = JObject::from(env.new_string("Horizon")?);
        let text = JObject::from(env.new_string(text)?);
        let clip = env
            .call_static_method(
                jni_str!("android/content/ClipData"),
                jni_str!("newPlainText"),
                jni_sig!(
                    "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Landroid/content/ClipData;"
                ),
                &[JValue::Object(&label), JValue::Object(&text)],
            )?
            .l()?;
        env.call_method(
            manager,
            jni_str!("setPrimaryClip"),
            jni_sig!("(Landroid/content/ClipData;)V"),
            &[JValue::Object(&clip)],
        )?;
        Ok(())
    })
}
