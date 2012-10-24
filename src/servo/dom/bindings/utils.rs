use js::rust::{compartment, bare_compartment, methods};
use js::{JS_ARGV, JSCLASS_HAS_RESERVED_SLOTS, JSPROP_ENUMERATE, JSPROP_SHARED, JSVAL_NULL,
            JS_THIS_OBJECT, JS_SET_RVAL};
use js::jsapi::{JSContext, JSVal, JSObject, JSBool, jsid, JSClass, JSFreeOp};
use js::jsapi::bindgen::{JS_ValueToString, JS_GetStringCharsZAndLength, JS_ReportError,
                            JS_GetReservedSlot, JS_SetReservedSlot, JS_NewStringCopyN,
                            JS_DefineFunctions, JS_DefineProperty, JS_GetContextPrivate,
                            JS_GetClass, JS_GetPrototype};
use js::glue::{PROPERTY_STUB, STRICT_PROPERTY_STUB, ENUMERATE_STUB, CONVERT_STUB,
                  RESOLVE_STUB};
use js::glue::bindgen::*;
use ptr::null;
use content::content_task::{Content, task_from_context};

enum DOMString {
    str(~str),
    null_string
}

type rust_box<T> = {rc: uint, td: *sys::TypeDesc, next: *(), prev: *(), payload: T};

unsafe fn squirrel_away<T>(x: @T) -> *rust_box<T> {
    let y: *rust_box<T> = cast::reinterpret_cast(&x);
    cast::forget(x);
    y
}

unsafe fn squirrel_away_unique<T>(x: ~T) -> *rust_box<T> {
    let y: *rust_box<T> = cast::reinterpret_cast(&x);
    cast::forget(move x);
    y
}

//XXX very incomplete
fn jsval_to_str(cx: *JSContext, v: JSVal) -> Result<~str, ()> {
    let jsstr;
    if RUST_JSVAL_IS_STRING(v) == 1 {
        jsstr = RUST_JSVAL_TO_STRING(v)
    } else {
        jsstr = JS_ValueToString(cx, v);
        if jsstr.is_null() {
            return Err(());
        }
    }

    let len = 0;
    let chars = JS_GetStringCharsZAndLength(cx, jsstr, ptr::to_unsafe_ptr(&len));
    return if chars.is_null() {
        Err(())
    } else {
        unsafe {
            let buf = vec::raw::from_buf_raw(chars as *u8, len as uint);
            Ok(str::from_bytes(buf))
        }
    }
}

unsafe fn domstring_to_jsval(cx: *JSContext, str: &DOMString) -> JSVal {
    match *str {
      null_string => {
        JSVAL_NULL
      }
      str(s) => {
        str::as_buf(s, |buf, len| {
            let cbuf = cast::reinterpret_cast(&buf);
            RUST_STRING_TO_JSVAL(JS_NewStringCopyN(cx, cbuf, len as libc::size_t))
        })
      }
    }
}

pub fn get_compartment(cx: *JSContext) -> compartment {
    unsafe {
        let content = task_from_context(cx);
        let compartment = option::expect((*content).compartment,
                                         ~"Should always have compartment when \
                                           executing JS code");
        assert cx == compartment.cx.ptr;
        compartment
    }
}

extern fn has_instance(_cx: *JSContext, obj: **JSObject, v: *JSVal, bp: *mut JSBool) -> JSBool {
    //XXXjdm this is totally broken for non-object values
    let mut o = RUST_JSVAL_TO_OBJECT(unsafe {*v});
    let obj = unsafe {*obj};
    unsafe { *bp = 0; }
    while o.is_not_null() {
        if o == obj {
            unsafe { *bp = 1; }
            break;
        }
        o = JS_GetPrototype(o);
    }
    return 1;
}

pub fn prototype_jsclass(name: ~str) -> fn(compartment: &bare_compartment) -> JSClass {
    |compartment: &bare_compartment, move name| {
        {name: compartment.add_name(copy name),
         flags: 0,
         addProperty: GetJSClassHookStubPointer(PROPERTY_STUB) as *u8,
         delProperty: GetJSClassHookStubPointer(PROPERTY_STUB) as *u8,
         getProperty: GetJSClassHookStubPointer(PROPERTY_STUB) as *u8,
         setProperty: GetJSClassHookStubPointer(STRICT_PROPERTY_STUB) as *u8,
         enumerate: GetJSClassHookStubPointer(ENUMERATE_STUB) as *u8,
         resolve: GetJSClassHookStubPointer(RESOLVE_STUB) as *u8,
         convert: GetJSClassHookStubPointer(CONVERT_STUB) as *u8,
         finalize: null(),
         checkAccess: null(),
         call: null(),
         hasInstance: has_instance,
         construct: null(),
         trace: null(),
         reserved: (null(), null(), null(), null(), null(),  // 05
                    null(), null(), null(), null(), null(),  // 10
                    null(), null(), null(), null(), null(),  // 15
                    null(), null(), null(), null(), null(),  // 20
                    null(), null(), null(), null(), null(),  // 25
                    null(), null(), null(), null(), null(),  // 30
                    null(), null(), null(), null(), null(),  // 35
                    null(), null(), null(), null(), null())} // 40
    }
}

pub fn instance_jsclass(name: ~str, finalize: *u8)
    -> fn(compartment: &bare_compartment) -> JSClass {
    |compartment: &bare_compartment, move name| {
        {name: compartment.add_name(copy name),
         flags: JSCLASS_HAS_RESERVED_SLOTS(1),
         addProperty: GetJSClassHookStubPointer(PROPERTY_STUB) as *u8,
         delProperty: GetJSClassHookStubPointer(PROPERTY_STUB) as *u8,
         getProperty: GetJSClassHookStubPointer(PROPERTY_STUB) as *u8,
         setProperty: GetJSClassHookStubPointer(STRICT_PROPERTY_STUB) as *u8,
         enumerate: GetJSClassHookStubPointer(ENUMERATE_STUB) as *u8,
         resolve: GetJSClassHookStubPointer(RESOLVE_STUB) as *u8,
         convert: GetJSClassHookStubPointer(CONVERT_STUB) as *u8,
         finalize: finalize,
         checkAccess: null(),
         call: null(),
         hasInstance: has_instance,
         construct: null(),
         trace: null(),
         reserved: (null(), null(), null(), null(), null(),  // 05
                    null(), null(), null(), null(), null(),  // 10
                    null(), null(), null(), null(), null(),  // 15
                    null(), null(), null(), null(), null(),  // 20
                    null(), null(), null(), null(), null(),  // 25
                    null(), null(), null(), null(), null(),  // 30
                    null(), null(), null(), null(), null(),  // 35
                    null(), null(), null(), null(), null())} // 40
    }
}

// FIXME: A lot of string copies here
pub fn define_empty_prototype(name: ~str, proto: Option<~str>, compartment: &bare_compartment)
    -> js::rust::jsobj {
    compartment.register_class(utils::prototype_jsclass(copy name));

    //TODO error checking
    let obj = result::unwrap(
        match move proto {
            Some(move s) => compartment.new_object_with_proto(copy name, move s, 
                                                        compartment.global_obj.ptr),
            None => compartment.new_object(copy name, null(), compartment.global_obj.ptr)
        });

    compartment.define_property(copy name, RUST_OBJECT_TO_JSVAL(obj.ptr),
                                GetJSClassHookStubPointer(PROPERTY_STUB) as *u8,
                                GetJSClassHookStubPointer(STRICT_PROPERTY_STUB) as *u8,
                                JSPROP_ENUMERATE);
    compartment.stash_global_proto(move name, obj);
    return obj;
}
