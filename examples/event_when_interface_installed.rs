use core::{
    ffi::c_void,
    mem::MaybeUninit,
    ptr::{self, NonNull},
};

use boot_services::{
    event::EventType,
    protocol_handler::{DriverBinding, Protocol, Registration},
    tpl::Tpl,
    BootServices, MockBootServices,
};
use r_efi::efi;

fn main() {
    let mut boot_services = MockBootServices::new();

    let _ = boot_services
        .expect_create_event::<Box<MaybeUninit<Registration>>>()
        .withf(|_, _, _, _| true)
        .returning(|_, _, _, _| Ok(ptr::null_mut()));

    let _ = boot_services
        .expect_register_protocol_notify()
        .withf(|_, _| true)
        .returning(|_, _| Ok(NonNull::<c_void>::dangling()));

    extern "efiapi" fn event_notify_callback(_event: efi::Event, context: Box<MaybeUninit<Registration>>) {
        println!("{context:?}")
    }

    let mut registration = Box::new(MaybeUninit::uninit());
    let registration_ptr = registration.as_mut_ptr();

    let event = boot_services
        .create_event(EventType::NOTIFY_SIGNAL, Tpl::CALLBACK, Some(event_notify_callback), registration)
        .unwrap();

    unsafe {
        registration_ptr.write(boot_services.register_protocol_notify(DriverBinding.protocol_guid(), event).unwrap())
    }
}
