extern crate alloc;

use alloc::boxed::Box;
use core::{ffi::c_void, mem::MaybeUninit, ptr};

use mu_rust_helpers::{
    boot_services::{event::EventType, tpl::Tpl, BootServices, StandardBootServices},
    tpl_mutex::TplMutex,
};

use r_efi::efi;

#[derive(Debug)]
struct MyContext {
    _some_immutable_state: usize,
    _some_other_immutable_state: efi::Handle,
    some_mutable_state: TplMutex<'static, i32>,
    _some_other_mutable_state: TplMutex<'static, String>,
}
unsafe impl Sync for MyContext {}

extern "efiapi" fn event_notify_callback_tpl_mutex(_event: efi::Event, context: &'static MyContext) {
    let mut some_mutable_state = context.some_mutable_state.lock();
    *some_mutable_state += 1;
}

extern "efiapi" fn event_notify_callback_tpl_mutex_2(_event: efi::Event, context: Option<&'static MyContext>) {
    println!("{context:?}")
}

extern "efiapi" fn event_notify_callback_void(_event: efi::Event, context: Box<()>) {
    println!("{context:?}")
}

fn main() {
    static BOOT_SERVICE: StandardBootServices = StandardBootServices::new_uninit();
    let efi_boot_services = unsafe {
        let mut bs = MaybeUninit::<efi::BootServices>::zeroed();
        bs.assume_init_mut().create_event = efi_create_event;
        bs.assume_init_mut().raise_tpl = efi_raise_tpl;
        bs.assume_init_mut().restore_tpl = efi_restore_tpl;
        bs.assume_init()
    };
    BOOT_SERVICE.initialize(&efi_boot_services);

    let ctx = Box::new(MyContext {
        _some_immutable_state: 0,
        _some_other_immutable_state: ptr::null_mut(),
        some_mutable_state: TplMutex::new(&BOOT_SERVICE, Tpl::CALLBACK, 0),
        _some_other_mutable_state: TplMutex::new(&BOOT_SERVICE, Tpl::CALLBACK, String::new()),
    });

    let ctx = Box::leak::<'static>(ctx) as &MyContext;

    match BOOT_SERVICE.create_event(
        EventType::RUNTIME | EventType::NOTIFY_SIGNAL,
        Tpl::CALLBACK,
        Some(event_notify_callback_tpl_mutex),
        ctx,
    ) {
        Ok(_event) => (),
        Err(_status) => (),
    };

    match BOOT_SERVICE.create_event(
        EventType::RUNTIME | EventType::NOTIFY_SIGNAL,
        Tpl::CALLBACK,
        Some(event_notify_callback_tpl_mutex_2),
        Some(ctx),
    ) {
        Ok(_event) => (),
        Err(_status) => (),
    };

    match BOOT_SERVICE.create_event(EventType::RUNTIME, Tpl::CALLBACK, Some(event_notify_callback_void), Box::new(())) {
        Ok(_event) => (),
        Err(_status) => (),
    };

    drop(unsafe { Box::from_raw(ctx as *const _ as *mut MyContext) });
}

extern "efiapi" fn efi_create_event(
    _event_type: u32,
    _notify_tpl: efi::Tpl,
    notify_function: Option<efi::EventNotify>,
    notify_context: *mut c_void,
    _event: *mut efi::Event,
) -> efi::Status {
    if let Some(notify_function) = notify_function {
        notify_function(ptr::null_mut(), notify_context);
    }
    efi::Status::SUCCESS
}

extern "efiapi" fn efi_raise_tpl(_tpl: efi::Tpl) -> efi::Tpl {
    efi::TPL_APPLICATION
}

extern "efiapi" fn efi_restore_tpl(_tpl: efi::Tpl) {}
