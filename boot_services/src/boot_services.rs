#![cfg_attr(all(not(test), not(feature = "mockall")), no_std)]

#[cfg(feature = "global_allocator")]
pub mod global_allocator;

extern crate alloc;

pub mod allocation;
pub mod boxed;
pub mod event;
pub mod protocol_handler;
pub mod static_ptr;
pub mod tpl;

#[cfg(any(test, feature = "mockall"))]
use mockall::automock;

use alloc::vec::Vec;
use core::{
    any::{Any, TypeId},
    ffi::c_void,
    marker::PhantomData,
    mem::{self, MaybeUninit},
    option::Option,
    ptr,
    sync::atomic::{AtomicPtr, Ordering},
};
use static_ptr::{StaticPtr, StaticPtrMut};

use r_efi::efi;

use allocation::{AllocType, MemoryMap, MemoryType};
use boxed::BootServicesBox;
use event::{EventNotifyCallback, EventTimerType, EventType};
use protocol_handler::{HandleSearchType, Protocol, Registration};
use tpl::{Tpl, TplGuard};

/// This is the boot services used in the UEFI.
/// it wraps an atomic ptr to [`efi::BootServices`]
#[derive(Debug)]
pub struct StandardBootServices<'a> {
    efi_boot_services: AtomicPtr<efi::BootServices>,
    _lifetime_marker: PhantomData<&'a efi::BootServices>,
}

impl<'a> StandardBootServices<'a> {
    /// Create a new StandardBootServices with the provided [efi::BootServices].
    pub const fn new(efi_boot_services: &'a efi::BootServices) -> Self {
        // The efi::BootServices is only read, that is why we use a non mutable reference.
        Self {
            efi_boot_services: AtomicPtr::new(efi_boot_services as *const _ as *mut _),
            _lifetime_marker: PhantomData,
        }
    }

    /// Create a new StandardBootServices that is uninitialized.
    /// The struct need to be initialize later with [Self::initialize], otherwise, subsequent call will panic.
    pub const fn new_uninit() -> Self {
        Self { efi_boot_services: AtomicPtr::new(ptr::null_mut()), _lifetime_marker: PhantomData }
    }

    /// Initialize the StandardBootServices with a reference to [efi::BootServices].
    /// # Panics
    /// This function will panic if already initialize.
    pub fn initialize(&'a self, efi_boot_services: &'a efi::BootServices) {
        if self.efi_boot_services.load(Ordering::Relaxed).is_null() {
            // The efi::BootServices is only read, that is why we use a non mutable reference.
            self.efi_boot_services.store(efi_boot_services as *const _ as *mut _, Ordering::SeqCst)
        } else {
            panic!("Boot services is already initialize.")
        }
    }

    /// # Panics
    /// This function will panic if it was not initialize.
    fn efi_boot_services(&self) -> &efi::BootServices {
        // SAFETY: This pointer is assume to be a valid efi::BootServices pointer since the only way to set it was via an efi::BootServices reference.
        unsafe {
            self.efi_boot_services.load(Ordering::SeqCst).as_ref::<'a>().expect("Boot services is not initialize.")
        }
    }
}

///SAFETY: StandardBootServices uses an atomic ptr to access the BootServices.
unsafe impl Sync for StandardBootServices<'static> {}
///SAFETY: When the lifetime is `'static`, the pointer is guaranteed to stay valid.
unsafe impl Send for StandardBootServices<'static> {}

/// Functions that are available *before* a successful call to EFI_BOOT_SERVICES.ExitBootServices().
#[cfg_attr(any(test, feature = "mockall"), automock)]
pub trait BootServices: Sized {
    /// Create an event.
    ///
    /// UEFI Spec Documentation:
    /// <a href="https://uefi.org/specs/UEFI/2.10/07_Services_Boot_Services.html#efi-boot-services-createevent" target="_blank">
    ///   7.1.1. EFI_BOOT_SERVICES.CreateEvent()
    /// </a>
    fn create_event<T>(
        &self,
        event_type: EventType,
        notify_tpl: Tpl,
        notify_function: Option<EventNotifyCallback<T>>,
        notify_context: T,
    ) -> Result<efi::Event, efi::Status>
    where
        T: StaticPtr + 'static,
        <T as StaticPtr>::Pointee: Sized + 'static,
    {
        //SAFETY: ['StaticPtr`] generic is used to guaranteed that rust borowing and rules are meet.
        unsafe {
            self.create_event_unchecked(
                event_type,
                notify_tpl,
                mem::transmute(notify_function),
                notify_context.into_raw() as *mut <T as StaticPtr>::Pointee,
            )
        }
    }

    /// Prefer normal [`BootServices::create_event`] when possible.
    ///
    /// # Safety
    ///
    /// When calling this method, you have to make sure that *notify_context* pointer is **null** or all of the following is true:
    /// * The pointer must be properly aligned.
    /// * It must be "dereferenceable" into type `T`
    /// * It must remain a valid pointer for the lifetime of the event.
    /// * You must enforce Rust’s borrowing[^borrowing rules] rules rules.
    ///
    /// [^borrowing rules]:
    /// Rust By Example Book:
    /// <a href="https://doc.rust-lang.org/beta/rust-by-example/scope/borrow.html" target="_blank">
    ///   15.3. Borrowing
    /// </a>
    unsafe fn create_event_unchecked<T: Sized + 'static>(
        &self,
        event_type: EventType,
        notify_tpl: Tpl,
        notify_function: Option<EventNotifyCallback<*mut T>>,
        notify_context: *mut T,
    ) -> Result<efi::Event, efi::Status>;

    /// Create an event in a group.
    ///
    /// UEFI Spec Documentation:
    /// <a href="https://uefi.org/specs/UEFI/2.10/07_Services_Boot_Services.html#efi-boot-services-createeventex" target="_blank">
    ///   7.1.2. EFI_BOOT_SERVICES.CreateEventEx()
    /// </a>
    fn create_event_ex<T>(
        &self,
        event_type: EventType,
        notify_tpl: Tpl,
        notify_function: Option<EventNotifyCallback<T>>,
        notify_context: T,
        event_group: &'static efi::Guid,
    ) -> Result<efi::Event, efi::Status>
    where
        T: StaticPtr + 'static,
        <T as StaticPtr>::Pointee: Sized + 'static,
    {
        //SAFETY: [`StaticPtr`] generic is used to guaranteed that rust borowing and rules are meet.
        unsafe {
            self.create_event_ex_unchecked(
                event_type,
                notify_tpl.into(),
                mem::transmute(notify_function),
                notify_context.into_raw() as *mut <T as StaticPtr>::Pointee,
                event_group,
            )
        }
    }

    /// Prefer normal [`BootServices::create_event_ex`] when possible.
    ///
    /// # Safety
    ///
    /// Make sure to comply to the same constraint as [`BootServices::create_event_unchecked`]
    unsafe fn create_event_ex_unchecked<T: Sized + 'static>(
        &self,
        event_type: EventType,
        notify_tpl: Tpl,
        notify_function: EventNotifyCallback<*mut T>,
        notify_context: *mut T,
        event_group: &'static efi::Guid,
    ) -> Result<efi::Event, efi::Status>;

    /// Close an event.
    ///
    /// UEFI Spec Documentation:
    /// <a href="https://uefi.org/specs/UEFI/2.10/07_Services_Boot_Services.html#efi-boot-services-closeevent" target="_blank">
    ///   7.1.3. EFI_BOOT_SERVICES.CloseEvent()
    /// </a>
    ///
    /// [^note]: It is safe to call *close_event* in the notify function.
    fn close_event(&self, event: efi::Event) -> Result<(), efi::Status>;

    /// Signals an event.
    ///
    /// UEFI Spec Documentation:
    /// <a href="https://uefi.org/specs/UEFI/2.10/07_Services_Boot_Services.html#efi-boot-services-signalevent" target="_blank">
    ///   7.1.4. EFI_BOOT_SERVICES.SignalEvent()
    /// </a>
    fn signal_event(&self, event: efi::Event) -> Result<(), efi::Status>;

    /// Stops execution until an event is signaled.
    ///
    /// UEFI Spec Documentation:
    /// <a href="https://uefi.org/specs/UEFI/2.10/07_Services_Boot_Services.html#efi-boot-services-waitforevent" target="_blank">
    ///   7.1.5. EFI_BOOT_SERVICES.WaitForEvent()
    /// </a>
    fn wait_for_event(&self, events: &mut [efi::Event]) -> Result<usize, efi::Status>;

    /// Checks whether an event is in the signaled state.
    ///
    /// UEFI Spec Documentation:
    /// <a href="https://uefi.org/specs/UEFI/2.10/07_Services_Boot_Services.html#efi-boot-services-checkevent" target="_blank">
    ///   7.1.6. EFI_BOOT_SERVICES.CheckEvent()
    /// </a>
    fn check_event(&self, event: efi::Event) -> Result<(), efi::Status>;

    /// Sets the type of timer and the trigger time for a timer event.
    ///
    /// UEFI Spec Documentation:
    /// <a href="https://uefi.org/specs/UEFI/2.10/07_Services_Boot_Services.html#efi-boot-services-settimer" target="_blank">
    ///   7.1.7. EFI_BOOT_SERVICES.SetTimer()
    /// </a>
    fn set_timer(&self, event: efi::Event, timer_type: EventTimerType, trigger_time: u64) -> Result<(), efi::Status>;

    /// Raises a task's priority level and returns a [`TplGuard`] that will restore the tpl when dropped.
    ///
    /// See [`BootServices::raise_tpl`] and [`BootServices::restore_tpl`] for more details.
    fn raise_tpl_guarded<'a>(&'a self, tpl: Tpl) -> TplGuard<'a, Self> {
        TplGuard { boot_services: self, retore_tpl: self.raise_tpl(tpl) }
    }

    /// Raises a task’s priority level and returns its previous level.
    ///
    /// UEFI Spec Documentation:
    /// <a href="https://uefi.org/specs/UEFI/2.10/07_Services_Boot_Services.html#efi-boot-services-raisetpl" target="_blank">
    ///   7.1.8. EFI_BOOT_SERVICES.RaiseTPL()
    /// </a>
    fn raise_tpl(&self, tpl: Tpl) -> Tpl;

    /// Restores a task’s priority level to its previous value.
    ///
    /// UEFI Spec Documentation:
    /// <a href="https://uefi.org/specs/UEFI/2.10/07_Services_Boot_Services.html#efi-boot-services-restoretpl" target="_blank">
    ///   7.1.9. EFI_BOOT_SERVICES.RestoreTPL()
    /// </a>
    fn restore_tpl(&self, tpl: Tpl);

    /// Allocates memory pages from the system.
    ///
    /// UEFI Spec Documentation:
    /// <a href="https://uefi.org/specs/UEFI/2.10/07_Services_Boot_Services.html#efi-boot-services-allocatepages" target="_blank">
    ///   7.2.1. EFI_BOOT_SERVICES.AllocatePages()
    /// </a>
    fn allocate_pages(
        &self,
        alloc_type: AllocType,
        memory_type: MemoryType,
        nb_pages: usize,
    ) -> Result<usize, efi::Status>;

    fn free_pages(&self, address: usize, nb_pages: usize) -> Result<(), efi::Status>;

    fn get_memory_map<'a>(&'a self) -> Result<MemoryMap<'a, Self>, (efi::Status, usize)>;

    fn allocate_pool(&self, pool_type: MemoryType, size: usize) -> Result<*mut u8, efi::Status>;

    fn allocate_pool_for_type<T: 'static>(&self, pool_type: MemoryType) -> Result<*mut T, efi::Status> {
        let ptr = self.allocate_pool(pool_type, mem::size_of::<T>())?;
        Ok(ptr as *mut T)
    }

    fn free_pool(&self, buffer: *mut u8) -> Result<(), efi::Status>;

    fn install_protocol_interface<P: Protocol<Interface = I> + 'static, I: Any + 'static>(
        &self,
        handle: Option<efi::Handle>,
        protocol: &P,
        interface: &'static mut I,
    ) -> Result<efi::Handle, efi::Status> {
        let interface_ptr = match (interface as &dyn Any).downcast_ref::<()>() {
            Some(()) => ptr::null_mut(),
            None => interface as *mut _ as *mut c_void,
        };
        //SAFETY: The generic Protocol ensure that the interface is the right type for the specified protocol.
        unsafe { self.install_protocol_interface_unchecked(handle, protocol.protocol_guid(), interface_ptr) }
    }

    unsafe fn install_protocol_interface_unchecked(
        &self,
        handle: Option<efi::Handle>,
        protocol: &'static efi::Guid,
        interface: *mut c_void,
    ) -> Result<efi::Handle, efi::Status>;

    fn uninstall_protocol_interface<P: Protocol<Interface = I> + 'static, I: Any + 'static>(
        &self,
        handle: efi::Handle,
        protocol: &P,
        interface: &'static mut I,
    ) -> Result<(), efi::Status> {
        let interface_ptr = match (interface as &dyn Any).downcast_ref::<()>() {
            Some(()) => ptr::null_mut(),
            None => interface as *mut _ as *mut c_void,
        };
        //SAFETY: The generic Protocol ensure that the interface is the right type for the specified protocol.
        unsafe { self.uninstall_protocol_interface_unchecked(handle, protocol.protocol_guid(), interface_ptr) }
    }

    unsafe fn uninstall_protocol_interface_unchecked(
        &self,
        handle: efi::Handle,
        protocol: &'static efi::Guid,
        interface: *mut c_void,
    ) -> Result<(), efi::Status>;

    fn reinstall_protocol_interface<P: Protocol<Interface = I> + 'static, I: 'static>(
        &self,
        handle: efi::Handle,
        protocol: &P,
        old_protocol_interface: &'static mut I,
        new_protocol_interface: &'static mut I,
    ) -> Result<(), efi::Status> {
        let old_protocol_interface_ptr;
        let new_protocol_interface_ptr;
        if TypeId::of::<I>() == TypeId::of::<()>() {
            old_protocol_interface_ptr = ptr::null_mut();
            new_protocol_interface_ptr = ptr::null_mut();
        } else {
            old_protocol_interface_ptr = old_protocol_interface as *mut _ as *mut c_void;
            new_protocol_interface_ptr = new_protocol_interface as *mut _ as *mut c_void;
        }
        //SAFETY: The generic Protocol ensure that the interfaces is the right type for the specified protocol.
        unsafe {
            self.reinstall_protocol_interface_unchecked(
                handle,
                protocol.protocol_guid(),
                old_protocol_interface_ptr,
                new_protocol_interface_ptr,
            )
        }
    }

    unsafe fn reinstall_protocol_interface_unchecked(
        &self,
        handle: efi::Handle,
        protocol: &'static efi::Guid,
        old_protocol_interface: *mut c_void,
        new_protocol_interface: *mut c_void,
    ) -> Result<(), efi::Status>;

    fn register_protocol_notify(
        &self,
        protocol: &'static efi::Guid,
        event: efi::Event,
    ) -> Result<Registration, efi::Status>;

    fn locate_handle<'a>(
        &'a self,
        search_type: HandleSearchType,
    ) -> Result<BootServicesBox<'a, [efi::Handle], Self>, efi::Status>;

    fn handle_protocol<P: Protocol<Interface = I> + 'static, I: 'static>(
        &self,
        handle: efi::Handle,
        protocol: &P,
    ) -> Result<&'static mut I, efi::Status> {
        //SAFETY: The generic Protocol ensure that the interfaces is the right type for the specified protocol.
        unsafe {
            self.handle_protocol_unchecked(handle, protocol.protocol_guid()).map(|i| (i as *mut I).as_mut().unwrap())
        }
    }

    fn handle_protocol_unchecked(&self, handle: efi::Handle, protocol: &efi::Guid) -> Result<*mut c_void, efi::Status>;

    unsafe fn locate_device_path(
        &self,
        protocol: &efi::Guid,
        device_path: *mut *mut efi::protocols::device_path::Protocol,
    ) -> Result<efi::Handle, efi::Status>;

    fn open_protocol<P: Protocol<Interface = I> + 'static, I: 'static>(
        &self,
        handle: efi::Handle,
        protocol: &P,
        agent_handle: efi::Handle,
        controller_handle: efi::Handle,
        attribute: u32,
    ) -> Result<Option<&'static mut I>, efi::Status> {
        //SAFETY: The generic Protocol ensure that the interfaces is the right type for the specified protocol.
        unsafe {
            self.open_protocol_unchecked(handle, protocol, agent_handle, controller_handle, attribute)
                .map(|i| (i as *mut I).as_mut())
        }
    }

    fn open_protocol_unchecked(
        &self,
        handle: efi::Handle,
        protocol: &efi::Guid,
        agent_handle: efi::Handle,
        controller_handle: efi::Handle,
        attribute: u32,
    ) -> Result<*mut c_void, efi::Status>;

    fn close_protocol(
        &self,
        handle: efi::Handle,
        protocol: &efi::Guid,
        agent_handle: efi::Handle,
        controller_handle: efi::Handle,
    ) -> Result<(), efi::Status>;

    fn open_protocol_information<'a>(
        &'a self,
        handle: efi::Handle,
        protocol: &efi::Guid,
    ) -> Result<BootServicesBox<'a, [efi::OpenProtocolInformationEntry], Self>, efi::Status>;

    unsafe fn connect_controller(
        &self,
        controller_handle: efi::Handle,
        driver_image_handle: Vec<efi::Handle>,
        remaining_device_path: *mut efi::protocols::device_path::Protocol,
        recursive: bool,
    ) -> Result<(), efi::Status>;

    fn disconnect_controller(
        &self,
        controller_handle: efi::Handle,
        driver_image_handle: Option<efi::Handle>,
        child_handle: Option<efi::Handle>,
    ) -> Result<(), efi::Status>;

    fn protocols_per_handle<'a>(
        &'a self,
        handle: efi::Handle,
    ) -> Result<BootServicesBox<'a, [efi::Guid], Self>, efi::Status>;

    fn locate_handle_buffer<'a>(
        &'a self,
        search_type: HandleSearchType,
    ) -> Result<BootServicesBox<'a, [efi::Handle], Self>, efi::Status>;

    fn locate_protocol<P: Protocol<Interface = I> + 'static, I: 'static>(
        &self,
        protocol: &P,
        registration: Option<Registration>,
    ) -> Result<&'static mut I, efi::Status> {
        //SAFETY: The generic Protocol ensure that the interfaces is the right type for the specified protocol.
        unsafe {
            self.locate_protocol_unchecked(
                protocol.protocol_guid(),
                registration.map_or(ptr::null_mut(), |r| r.as_ptr()),
            )
            .map(|ptr| (ptr as *mut I).as_mut().unwrap())
        }
    }

    fn locate_protocol_unchecked(
        &self,
        protocol: &'static efi::Guid,
        registration: *mut c_void,
    ) -> Result<*mut c_void, efi::Status>;

    fn install_configuration_table<T: StaticPtrMut + 'static>(
        &self,
        guid: &efi::Guid,
        table: T,
    ) -> Result<(), efi::Status> {
        unsafe { self.install_configuration_table_unchecked(guid, table.into_raw_mut() as *mut c_void) }
    }
    unsafe fn install_configuration_table_unchecked(
        &self,
        guid: &efi::Guid,
        table: *mut c_void,
    ) -> Result<(), efi::Status>;
}

impl BootServices for StandardBootServices<'_> {
    unsafe fn create_event_unchecked<T: Sized + 'static>(
        &self,
        event_type: EventType,
        notify_tpl: Tpl,
        notify_function: Option<EventNotifyCallback<*mut T>>,
        notify_context: *mut T,
    ) -> Result<efi::Event, efi::Status> {
        let create_event = self.efi_boot_services().create_event;
        if create_event as usize == 0 {
            panic!("function not initialize.")
        }

        let mut event = MaybeUninit::zeroed();
        let status = create_event(
            event_type.into(),
            notify_tpl.into(),
            mem::transmute(notify_function),
            notify_context as *mut c_void,
            event.as_mut_ptr(),
        );
        if status.is_error() {
            Err(status)
        } else {
            Ok(event.assume_init())
        }
    }

    unsafe fn create_event_ex_unchecked<T: Sized + 'static>(
        &self,
        event_type: EventType,
        notify_tpl: Tpl,
        notify_function: EventNotifyCallback<*mut T>,
        notify_context: *mut T,
        event_group: &'static efi::Guid,
    ) -> Result<efi::Event, efi::Status> {
        let create_event_ex = self.efi_boot_services().create_event_ex;
        if create_event_ex as usize == 0 {
            panic!("function not initialize.")
        }

        let mut event = MaybeUninit::zeroed();
        let status = create_event_ex(
            event_type.into(),
            notify_tpl.into(),
            mem::transmute(notify_function),
            notify_context as *mut c_void,
            event_group as *const _,
            event.as_mut_ptr(),
        );
        if status.is_error() {
            Err(status)
        } else {
            Ok(event.assume_init())
        }
    }

    fn close_event(&self, event: efi::Event) -> Result<(), efi::Status> {
        let close_event = self.efi_boot_services().close_event;
        if close_event as usize == 0 {
            panic!("function not initialize.")
        }
        match close_event(event) {
            s if s.is_error() => Err(s),
            _ => Ok(()),
        }
    }

    fn signal_event(&self, event: efi::Event) -> Result<(), efi::Status> {
        let signal_event = self.efi_boot_services().signal_event;
        if signal_event as usize == 0 {
            panic!("function not initialize.")
        }
        match signal_event(event) {
            s if s.is_error() => Err(s),
            _ => Ok(()),
        }
    }

    fn wait_for_event(&self, events: &mut [efi::Event]) -> Result<usize, efi::Status> {
        let wait_for_event = self.efi_boot_services().wait_for_event;
        if wait_for_event as usize == 0 {
            panic!("function not initialize.")
        }
        let mut index = MaybeUninit::zeroed();
        let status = wait_for_event(events.len(), events.as_mut_ptr(), index.as_mut_ptr());
        if status.is_error() {
            Err(status)
        } else {
            Ok(unsafe { index.assume_init() })
        }
    }

    fn check_event(&self, event: efi::Event) -> Result<(), efi::Status> {
        let check_event = self.efi_boot_services().check_event;
        if check_event as usize == 0 {
            panic!("function not initialize.")
        }
        match check_event(event) {
            s if s.is_error() => Err(s),
            _ => Ok(()),
        }
    }

    fn set_timer(&self, event: efi::Event, timer_type: EventTimerType, trigger_time: u64) -> Result<(), efi::Status> {
        let set_timer = self.efi_boot_services().set_timer;
        if set_timer as usize == 0 {
            panic!("function not initialize.")
        }
        match set_timer(event, timer_type.into(), trigger_time) {
            s if s.is_error() => Err(s),
            _ => Ok(()),
        }
    }

    fn raise_tpl(&self, new_tpl: Tpl) -> Tpl {
        let raise_tpl = self.efi_boot_services().raise_tpl;
        if raise_tpl as usize == 0 {
            panic!("function not initialize.")
        }
        raise_tpl(new_tpl.into()).into()
    }

    fn restore_tpl(&self, old_tpl: Tpl) {
        let restore_tpl = self.efi_boot_services().restore_tpl;
        if restore_tpl as usize == 0 {
            panic!("function not initialize.")
        }
        restore_tpl(old_tpl.into())
    }

    fn allocate_pages(
        &self,
        alloc_type: AllocType,
        memory_type: MemoryType,
        nb_pages: usize,
    ) -> Result<usize, efi::Status> {
        let allocate_pages = self.efi_boot_services().allocate_pages;
        if allocate_pages as usize == 0 {
            panic!("function not initialize.")
        }

        let mut memory_address = match alloc_type {
            AllocType::Address(address) => address,
            AllocType::MaxAddress(address) => address,
            _ => 0,
        };
        match allocate_pages(
            alloc_type.into(),
            memory_type.into(),
            nb_pages,
            ptr::addr_of_mut!(memory_address) as *mut u64,
        ) {
            s if s.is_error() => Err(s),
            _ => Ok(memory_address),
        }
    }

    fn free_pages(&self, address: usize, nb_pages: usize) -> Result<(), efi::Status> {
        let free_pages = self.efi_boot_services().free_pages;
        if free_pages as usize == 0 {
            panic!("function not initialize.")
        }
        match free_pages(address as u64, nb_pages) {
            s if s.is_error() => Err(s),
            _ => Ok(()),
        }
    }

    fn get_memory_map<'a>(&'a self) -> Result<MemoryMap<'a, Self>, (efi::Status, usize)> {
        let get_memory_map = self.efi_boot_services().get_memory_map;
        if get_memory_map as usize == 0 {
            panic!("function not initialize.")
        }

        let mut memory_map_size = 0;
        let mut map_key = 0;
        let mut descriptor_size = 0;
        let mut descriptor_version = 0;

        match get_memory_map(
            ptr::addr_of_mut!(memory_map_size),
            ptr::null_mut(),
            ptr::addr_of_mut!(map_key),
            ptr::addr_of_mut!(descriptor_size),
            ptr::addr_of_mut!(descriptor_version),
        ) {
            s if s == efi::Status::BUFFER_TOO_SMALL => memory_map_size += 0x400, // add more space in case allocation makes the memory map bigger.
            _ => (),
        };

        let buffer = self.allocate_pool(MemoryType::BOOT_SERVICES_DATA, memory_map_size).map_err(|s| (s, 0))?;

        match get_memory_map(
            ptr::addr_of_mut!(memory_map_size),
            buffer as *mut _,
            ptr::addr_of_mut!(map_key),
            ptr::addr_of_mut!(descriptor_size),
            ptr::addr_of_mut!(descriptor_version),
        ) {
            s if s == efi::Status::BUFFER_TOO_SMALL => return Err((s, memory_map_size)),
            s if s.is_error() => return Err((s, 0)),
            _ => (),
        }
        Ok(MemoryMap {
            descriptors: unsafe { BootServicesBox::from_raw_parts(buffer as *mut _, descriptor_size, self) },
            map_key,
            descriptor_version,
        })
    }

    fn allocate_pool(&self, memory_type: MemoryType, size: usize) -> Result<*mut u8, efi::Status> {
        let allocate_pool = self.efi_boot_services().allocate_pool;
        if allocate_pool as usize == 0 {
            panic!("function not initialize.")
        }
        let mut buffer = ptr::null_mut();
        match allocate_pool(memory_type.into(), size, ptr::addr_of_mut!(buffer)) {
            s if s.is_error() => return Err(s),
            _ => Ok(buffer as *mut u8),
        }
    }

    fn free_pool(&self, buffer: *mut u8) -> Result<(), efi::Status> {
        let free_pool = self.efi_boot_services().free_pool;
        if free_pool as usize == 0 {
            panic!("function not initialize.")
        }
        match free_pool(buffer as *mut c_void) {
            s if s.is_error() => return Err(s),
            _ => Ok(()),
        }
    }

    unsafe fn install_protocol_interface_unchecked(
        &self,
        handle: Option<efi::Handle>,
        protocol: &'static efi::Guid,
        interface: *mut c_void,
    ) -> Result<efi::Handle, efi::Status> {
        let install_protocol_interface = self.efi_boot_services().install_protocol_interface;
        if install_protocol_interface as usize == 0 {
            panic!("function not initialize.")
        }

        let mut handle = handle.unwrap_or(ptr::null_mut());
        match install_protocol_interface(
            ptr::addr_of_mut!(handle),
            protocol as *const _ as *mut _,
            efi::NATIVE_INTERFACE,
            interface,
        ) {
            s if s.is_error() => Err(s),
            _ => Ok(handle),
        }
    }

    unsafe fn uninstall_protocol_interface_unchecked(
        &self,
        handle: efi::Handle,
        protocol: &'static efi::Guid,
        interface: *mut c_void,
    ) -> Result<(), efi::Status> {
        let uninstall_protocol_interface = self.efi_boot_services().uninstall_protocol_interface;
        if uninstall_protocol_interface as usize == 0 {
            panic!("function not initialize.")
        }
        match uninstall_protocol_interface(handle, protocol as *const _ as *mut _, interface) {
            s if s.is_error() => Err(s),
            _ => Ok(()),
        }
    }

    unsafe fn reinstall_protocol_interface_unchecked(
        &self,
        handle: efi::Handle,
        protocol: &'static efi::Guid,
        old_protocol_interface: *mut c_void,
        new_protocol_interface: *mut c_void,
    ) -> Result<(), efi::Status> {
        let reinstall_protocol_interface = self.efi_boot_services().reinstall_protocol_interface;
        if reinstall_protocol_interface as usize == 0 {
            panic!("function not initialize.")
        }
        match reinstall_protocol_interface(
            handle,
            protocol as *const _ as *mut _,
            old_protocol_interface,
            new_protocol_interface,
        ) {
            s if s.is_error() => Err(s),
            _ => Ok(()),
        }
    }

    fn register_protocol_notify(&self, protocol: &efi::Guid, event: efi::Event) -> Result<Registration, efi::Status> {
        let register_protocol_notify = self.efi_boot_services().register_protocol_notify;
        if register_protocol_notify as usize == 0 {
            panic!("function not initialize.")
        }
        let mut registration = MaybeUninit::uninit();
        match register_protocol_notify(protocol as *const _ as *mut _, event, registration.as_mut_ptr() as *mut _) {
            s if s.is_error() => Err(s),
            _ => Ok(unsafe { registration.assume_init() }),
        }
    }

    fn locate_handle(
        &self,
        search_type: HandleSearchType,
    ) -> Result<BootServicesBox<[efi::Handle], Self>, efi::Status> {
        let locate_handle = self.efi_boot_services().locate_handle;
        if locate_handle as usize == 0 {
            panic!("function not initialize.")
        }
        let protocol = match search_type {
            HandleSearchType::ByProtocol(p) => p as *const _ as *mut _,
            _ => ptr::null_mut(),
        };
        let search_key = match search_type {
            HandleSearchType::ByRegisterNotify(r) => r.as_ptr(),
            _ => ptr::null_mut(),
        };

        // Use to get the buffer_size
        let mut buffer_size = 0;
        locate_handle(search_type.into(), protocol, search_key, ptr::addr_of_mut!(buffer_size), ptr::null_mut());

        let buffer = self.allocate_pool(MemoryType::BOOT_SERVICES_DATA, buffer_size)?;

        match locate_handle(
            search_type.into(),
            protocol,
            search_key,
            ptr::addr_of_mut!(buffer_size),
            buffer as *mut efi::Handle,
        ) {
            s if s.is_error() => Err(s),
            _ => Ok(unsafe {
                BootServicesBox::from_raw_parts(buffer as *mut _, buffer_size / mem::size_of::<efi::Handle>(), &self)
            }),
        }
    }

    fn handle_protocol_unchecked(&self, handle: efi::Handle, protocol: &efi::Guid) -> Result<*mut c_void, efi::Status> {
        let handle_protocol = self.efi_boot_services().handle_protocol;
        if handle_protocol as usize == 0 {
            panic!("function not initialize.")
        }
        let mut interface = ptr::null_mut();
        match handle_protocol(handle, protocol as *const _ as *mut _, ptr::addr_of_mut!(interface)) {
            s if s.is_error() => Err(s),
            _ => Ok(interface),
        }
    }

    unsafe fn locate_device_path(
        &self,
        protocol: &efi::Guid,
        device_path: *mut *mut efi::protocols::device_path::Protocol,
    ) -> Result<efi::Handle, efi::Status> {
        let locate_device_path = self.efi_boot_services().locate_device_path;
        if locate_device_path as usize == 0 {
            panic!("function not initialize.")
        }
        let mut device = ptr::null_mut();
        match locate_device_path(protocol as *const _ as *mut _, device_path, ptr::addr_of_mut!(device)) {
            s if s.is_error() => Err(s),
            _ => Ok(device),
        }
    }

    fn open_protocol_unchecked(
        &self,
        handle: efi::Handle,
        protocol: &efi::Guid,
        agent_handle: efi::Handle,
        controller_handle: efi::Handle,
        attribute: u32,
    ) -> Result<*mut c_void, efi::Status> {
        let open_protocol = self.efi_boot_services().open_protocol;
        if open_protocol as usize == 0 {
            panic!("function not initialize.")
        }
        let mut interface = ptr::null_mut();
        match open_protocol(
            handle,
            protocol as *const _ as *mut _,
            ptr::addr_of_mut!(interface),
            agent_handle,
            controller_handle,
            attribute,
        ) {
            s if s.is_error() => Err(s),
            _ => Ok(interface),
        }
    }

    fn close_protocol(
        &self,
        handle: efi::Handle,
        protocol: &efi::Guid,
        agent_handle: efi::Handle,
        controller_handle: efi::Handle,
    ) -> Result<(), efi::Status> {
        let close_protocol = self.efi_boot_services().close_protocol;
        if close_protocol as usize == 0 {
            panic!("function not initialize.")
        }
        match close_protocol(handle, protocol as *const _ as *mut _, agent_handle, controller_handle) {
            s if s.is_error() => Err(s),
            _ => Ok(()),
        }
    }

    fn open_protocol_information(
        &self,
        handle: efi::Handle,
        protocol: &efi::Guid,
    ) -> Result<BootServicesBox<[efi::OpenProtocolInformationEntry], Self>, efi::Status>
    where
        Self: Sized,
    {
        let open_protocol_information = self.efi_boot_services().open_protocol_information;
        if open_protocol_information as usize == 0 {
            panic!("function not initialize.")
        }

        let mut entry_buffer = ptr::null_mut();
        let mut entry_count = 0;
        match open_protocol_information(
            handle,
            protocol as *const _ as *mut _,
            ptr::addr_of_mut!(entry_buffer),
            ptr::addr_of_mut!(entry_count),
        ) {
            s if s.is_error() => Err(s),
            _ => Ok(unsafe { BootServicesBox::from_raw_parts(entry_buffer, entry_count, self) }),
        }
    }

    unsafe fn connect_controller(
        &self,
        controller_handle: efi::Handle,
        mut driver_image_handle: Vec<efi::Handle>,
        remaining_device_path: *mut efi::protocols::device_path::Protocol,
        recursive: bool,
    ) -> Result<(), efi::Status> {
        let connect_controller = self.efi_boot_services().connect_controller;
        if connect_controller as usize == 0 {
            panic!("function not initialize.")
        }

        let driver_image_handle = if driver_image_handle.is_empty() {
            ptr::null_mut()
        } else {
            driver_image_handle.push(ptr::null_mut());
            driver_image_handle.as_mut_ptr()
        };
        match connect_controller(controller_handle, driver_image_handle, remaining_device_path, recursive.into()) {
            s if s.is_error() => Err(s),
            _ => Ok(()),
        }
    }

    fn disconnect_controller(
        &self,
        controller_handle: efi::Handle,
        driver_image_handle: Option<efi::Handle>,
        child_handle: Option<efi::Handle>,
    ) -> Result<(), efi::Status> {
        let disconnect_controller = self.efi_boot_services().disconnect_controller;
        if disconnect_controller as usize == 0 {
            panic!("function not initialize.")
        }
        match disconnect_controller(
            controller_handle,
            driver_image_handle.unwrap_or(ptr::null_mut()),
            child_handle.unwrap_or(ptr::null_mut()),
        ) {
            s if s.is_error() => Err(s),
            _ => Ok(()),
        }
    }

    fn protocols_per_handle(&self, handle: efi::Handle) -> Result<BootServicesBox<[efi::Guid], Self>, efi::Status> {
        let protocols_per_handle = self.efi_boot_services().protocols_per_handle;
        if protocols_per_handle as usize == 0 {
            panic!("function not initialize.")
        }

        let mut protocol_buffer = ptr::null_mut();
        let mut protocol_buffer_count = 0;
        match protocols_per_handle(handle, ptr::addr_of_mut!(protocol_buffer), ptr::addr_of_mut!(protocol_buffer_count))
        {
            s if s.is_error() => Err(s),
            _ => Ok(unsafe {
                BootServicesBox::<[_], _>::from_raw_parts(protocol_buffer as *mut _, protocol_buffer_count, self)
            }),
        }
    }

    fn locate_handle_buffer(
        &self,
        search_type: HandleSearchType,
    ) -> Result<BootServicesBox<[efi::Handle], Self>, efi::Status>
    where
        Self: Sized,
    {
        let locate_handle_buffer = self.efi_boot_services().locate_handle_buffer;
        if locate_handle_buffer as usize == 0 {
            panic!("function not initialize.")
        }

        let mut buffer = ptr::null_mut();
        let mut buffer_count = 0;
        let protocol = match search_type {
            HandleSearchType::ByProtocol(p) => p as *const _ as *mut _,
            _ => ptr::null_mut(),
        };
        let search_key = match search_type {
            HandleSearchType::ByRegisterNotify(r) => r.as_ptr(),
            _ => ptr::null_mut(),
        };
        match locate_handle_buffer(
            search_type.into(),
            protocol,
            search_key,
            ptr::addr_of_mut!(buffer_count),
            ptr::addr_of_mut!(buffer),
        ) {
            s if s.is_error() => Err(s),
            _ => {
                Ok(unsafe { BootServicesBox::<[_], _>::from_raw_parts(buffer as *mut efi::Handle, buffer_count, self) })
            }
        }
    }

    fn locate_protocol_unchecked(
        &self,
        protocol: &'static efi::Guid,
        registration: *mut c_void,
    ) -> Result<*mut c_void, efi::Status> {
        let locate_protocol = self.efi_boot_services().locate_protocol;
        if locate_protocol as usize == 0 {
            panic!("function not initialize.")
        }
        let mut interface = ptr::null_mut();
        match locate_protocol(protocol as *const _ as *mut _, registration, ptr::addr_of_mut!(interface)) {
            s if s.is_error() => Err(s),
            _ => Ok(interface),
        }
    }

    unsafe fn install_configuration_table_unchecked(
        &self,
        guid: &efi::Guid,
        table: *mut c_void,
    ) -> Result<(), efi::Status> {
        let install_configuration_table = self.efi_boot_services().install_configuration_table;
        if install_configuration_table as usize == 0 {
            panic!("function not initialize.")
        }
        match install_configuration_table(guid as *const _ as *mut _, table) {
            s if s.is_error() => Err(s),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod test {
    use efi;

    use super::*;
    use core::{mem::MaybeUninit, sync::atomic::AtomicUsize};

    macro_rules! boot_services {
    ($($efi_services:ident = $efi_service_fn:ident),*) => {{
      static BOOT_SERVICE: StandardBootServices = StandardBootServices::new_uninit();
      let efi_boot_services = unsafe {
        #[allow(unused_mut)]
        let mut bs = MaybeUninit::<efi::BootServices>::zeroed();
        $(
          bs.assume_init_mut().$efi_services = $efi_service_fn;
        )*
        bs.assume_init()
      };
      BOOT_SERVICE.initialize(&efi_boot_services);
      &BOOT_SERVICE
    }};
  }

    #[test]
    #[should_panic(expected = "Boot services is not initialize.")]
    fn test_that_accessing_uninit_boot_services_should_panic() {
        let bs = StandardBootServices::new_uninit();
        bs.efi_boot_services();
    }

    #[test]
    #[should_panic(expected = "Boot services is already initialize.")]
    fn test_that_initializing_boot_services_multiple_time_should_panic() {
        let efi_bs = unsafe { MaybeUninit::<efi::BootServices>::zeroed().as_ptr().as_ref().unwrap() };
        let bs = StandardBootServices::new_uninit();
        bs.initialize(efi_bs);
        bs.initialize(efi_bs);
    }

    #[test]
    #[should_panic = "function not initialize."]
    fn test_create_event_not_init() {
        let boot_services = boot_services!();
        let _ = boot_services.create_event(EventType::RUNTIME, Tpl::APPLICATION, None, &());
    }

    #[test]
    fn test_create_event() {
        let boot_services = boot_services!(create_event = efi_create_event);

        extern "efiapi" fn notify_callback(_e: efi::Event, ctx: Box<i32>) {
            assert_eq!(10, *ctx)
        }

        extern "efiapi" fn efi_create_event(
            event_type: u32,
            notify_tpl: efi::Tpl,
            notify_function: Option<efi::EventNotify>,
            notify_context: *mut c_void,
            event: *mut efi::Event,
        ) -> efi::Status {
            assert_eq!(efi::EVT_RUNTIME | efi::EVT_NOTIFY_SIGNAL, event_type);
            assert_eq!(efi::TPL_APPLICATION, notify_tpl);
            assert_eq!(notify_callback as *const fn(), unsafe { mem::transmute(notify_function) });
            assert_ne!(ptr::null_mut(), notify_context);
            assert_ne!(ptr::null_mut(), event);

            if let Some(notify_function) = notify_function {
                notify_function(ptr::null_mut(), notify_context);
            }
            efi::Status::SUCCESS
        }

        let ctx = Box::new(10);
        let status = boot_services.create_event(
            EventType::RUNTIME | EventType::NOTIFY_SIGNAL,
            Tpl::APPLICATION,
            Some(notify_callback),
            ctx,
        );

        assert!(matches!(status, Ok(_)));
    }

    #[test]
    fn test_create_event_no_notify() {
        let boot_services = boot_services!(create_event = efi_create_event);

        extern "efiapi" fn efi_create_event(
            event_type: u32,
            notify_tpl: efi::Tpl,
            notify_function: Option<efi::EventNotify>,
            notify_context: *mut c_void,
            event: *mut efi::Event,
        ) -> efi::Status {
            assert_eq!(efi::EVT_RUNTIME | efi::EVT_NOTIFY_SIGNAL, event_type);
            assert_eq!(efi::TPL_APPLICATION, notify_tpl);
            assert_eq!(None, notify_function);
            assert_ne!(ptr::null_mut(), notify_context);
            assert_ne!(ptr::null_mut(), event);
            efi::Status::SUCCESS
        }

        let status =
            boot_services.create_event(EventType::RUNTIME | EventType::NOTIFY_SIGNAL, Tpl::APPLICATION, None, &());

        assert!(matches!(status, Ok(_)));
    }

    #[test]
    #[should_panic = "function not initialize."]
    fn test_create_event_ex_not_init() {
        static GUID: efi::Guid = efi::Guid::from_fields(0, 0, 0, 0, 0, &[0; 6]);
        let boot_services = boot_services!();
        let _ = boot_services.create_event_ex(EventType::RUNTIME, Tpl::APPLICATION, None, &(), &GUID);
    }

    #[test]
    fn test_create_event_ex() {
        let boot_services = boot_services!(create_event_ex = efi_create_event_ex);

        extern "efiapi" fn notify_callback(_e: efi::Event, ctx: Box<i32>) {
            assert_eq!(10, *ctx)
        }

        extern "efiapi" fn efi_create_event_ex(
            event_type: u32,
            notify_tpl: efi::Tpl,
            notify_function: Option<efi::EventNotify>,
            notify_context: *const c_void,
            event_group: *const efi::Guid,
            event: *mut efi::Event,
        ) -> efi::Status {
            assert_eq!(efi::EVT_RUNTIME | efi::EVT_NOTIFY_SIGNAL, event_type);
            assert_eq!(efi::TPL_APPLICATION, notify_tpl);
            assert_eq!(notify_callback as *const fn(), unsafe { mem::transmute(notify_function) });
            assert_ne!(ptr::null(), notify_context);
            assert_eq!(ptr::addr_of!(GUID), event_group);
            assert_ne!(ptr::null_mut(), event);

            if let Some(notify_function) = notify_function {
                notify_function(ptr::null_mut(), notify_context as *mut _);
            }
            efi::Status::SUCCESS
        }
        static GUID: efi::Guid = efi::Guid::from_fields(0, 0, 0, 0, 0, &[0; 6]);
        let ctx = Box::new(10);
        let status = boot_services.create_event_ex(
            EventType::RUNTIME | EventType::NOTIFY_SIGNAL,
            Tpl::APPLICATION,
            Some(notify_callback),
            ctx,
            &GUID,
        );

        assert!(matches!(status, Ok(_)));
    }

    #[test]
    fn test_create_event_ex_no_notify() {
        let boot_services = boot_services!(create_event_ex = efi_create_event_ex);

        extern "efiapi" fn efi_create_event_ex(
            event_type: u32,
            notify_tpl: efi::Tpl,
            notify_function: Option<efi::EventNotify>,
            notify_context: *const c_void,
            event_group: *const efi::Guid,
            event: *mut efi::Event,
        ) -> efi::Status {
            assert_eq!(efi::EVT_RUNTIME | efi::EVT_NOTIFY_SIGNAL, event_type);
            assert_eq!(efi::TPL_APPLICATION, notify_tpl);
            assert_eq!(None, notify_function);
            assert_ne!(ptr::null(), notify_context);
            assert_eq!(ptr::addr_of!(GUID), event_group);
            assert_ne!(ptr::null_mut(), event);
            efi::Status::SUCCESS
        }
        static GUID: efi::Guid = efi::Guid::from_fields(0, 0, 0, 0, 0, &[0; 6]);
        let status = boot_services.create_event_ex(
            EventType::RUNTIME | EventType::NOTIFY_SIGNAL,
            Tpl::APPLICATION,
            None,
            &(),
            &GUID,
        );

        assert!(matches!(status, Ok(_)));
    }

    #[test]
    #[should_panic = "function not initialize."]
    fn test_close_event_not_init() {
        let boot_services = boot_services!();
        let _ = boot_services.close_event(ptr::null_mut());
    }

    #[test]
    fn test_close_event() {
        let boot_services = boot_services!(close_event = efi_close_event);

        extern "efiapi" fn efi_close_event(event: efi::Event) -> efi::Status {
            assert_eq!(1, event as usize);
            efi::Status::SUCCESS
        }

        let event = 1_usize as efi::Event;
        let status = boot_services.close_event(event);
        assert!(matches!(status, Ok(())));
    }

    #[test]
    #[should_panic = "function not initialize."]
    fn test_signal_event_not_init() {
        let boot_services = boot_services!();
        let _ = boot_services.signal_event(ptr::null_mut());
    }

    #[test]
    fn test_signal_event() {
        let boot_services = boot_services!(signal_event = efi_signal_event);

        extern "efiapi" fn efi_signal_event(event: efi::Event) -> efi::Status {
            assert_eq!(1, event as usize);
            efi::Status::SUCCESS
        }

        let event = 1_usize as efi::Event;
        let status = boot_services.signal_event(event);
        assert!(matches!(status, Ok(())));
    }

    #[test]
    #[should_panic = "function not initialize."]
    fn test_wait_for_event_not_init() {
        let boot_services = boot_services!();
        let mut events = vec![];
        let _ = boot_services.wait_for_event(&mut events);
    }

    #[test]
    fn test_wait_for_event() {
        let boot_services = boot_services!(wait_for_event = efi_wait_for_event);

        extern "efiapi" fn efi_wait_for_event(
            number_of_event: usize,
            events: *mut efi::Event,
            index: *mut usize,
        ) -> efi::Status {
            assert_eq!(2, number_of_event);
            assert_ne!(ptr::null_mut(), events);

            unsafe { ptr::write(index, 1) }
            efi::Status::SUCCESS
        }

        let mut events = [1_usize as efi::Event, 2_usize as efi::Event];
        let status = boot_services.wait_for_event(&mut events);
        assert!(matches!(status, Ok(1)));
    }

    #[test]
    #[should_panic = "function not initialize."]
    fn test_check_event_not_init() {
        let boot_services = boot_services!();
        let _ = boot_services.check_event(ptr::null_mut());
    }

    #[test]
    fn test_check_event() {
        let boot_services = boot_services!(check_event = efi_check_event);

        extern "efiapi" fn efi_check_event(event: efi::Event) -> efi::Status {
            assert_eq!(1, event as usize);
            efi::Status::SUCCESS
        }

        let event = 1_usize as efi::Event;
        let status = boot_services.check_event(event);
        assert!(matches!(status, Ok(())));
    }

    #[test]
    #[should_panic = "function not initialize."]
    fn test_set_timer_not_init() {
        let boot_services = boot_services!();
        let _ = boot_services.set_timer(ptr::null_mut(), EventTimerType::Relative, 0);
    }

    #[test]
    fn test_set_timer() {
        let boot_services = boot_services!(set_timer = efi_set_timer);

        extern "efiapi" fn efi_set_timer(event: efi::Event, r#type: efi::TimerDelay, trigger_time: u64) -> efi::Status {
            assert_eq!(1, event as usize);
            assert_eq!(efi::TIMER_PERIODIC, r#type);
            assert_eq!(200, trigger_time);
            efi::Status::SUCCESS
        }

        let event = 1_usize as efi::Event;
        let status = boot_services.set_timer(event, EventTimerType::Periodic, 200);
        assert!(matches!(status, Ok(())));
    }

    #[test]
    fn test_raise_tpl_guarded() {
        let boot_services = boot_services!(raise_tpl = efi_raise_tpl, restore_tpl = efi_restore_tpl);

        static CURRENT_TPL: AtomicUsize = AtomicUsize::new(efi::TPL_APPLICATION);

        extern "efiapi" fn efi_raise_tpl(tpl: efi::Tpl) -> efi::Tpl {
            assert_eq!(efi::TPL_NOTIFY, tpl);
            CURRENT_TPL.swap(tpl, Ordering::Relaxed)
        }

        extern "efiapi" fn efi_restore_tpl(tpl: efi::Tpl) {
            assert_eq!(efi::TPL_APPLICATION, tpl);
            CURRENT_TPL.swap(tpl, Ordering::Relaxed);
        }

        let guard = boot_services.raise_tpl_guarded(Tpl::NOTIFY);
        assert_eq!(Tpl::APPLICATION, guard.retore_tpl);
        assert_eq!(efi::TPL_NOTIFY, CURRENT_TPL.load(Ordering::Relaxed));
        drop(guard);
        assert_eq!(efi::TPL_APPLICATION, CURRENT_TPL.load(Ordering::Relaxed));
    }

    #[test]
    #[should_panic = "function not initialize."]
    fn test_raise_tpl_not_init() {
        let boot_services = boot_services!();
        let _ = boot_services.raise_tpl(Tpl::CALLBACK);
    }

    #[test]
    fn test_raise_tpl() {
        let boot_services = boot_services!(raise_tpl = efi_raise_tpl);

        extern "efiapi" fn efi_raise_tpl(tpl: efi::Tpl) -> efi::Tpl {
            assert_eq!(efi::TPL_NOTIFY, tpl);
            efi::TPL_APPLICATION
        }

        let status = boot_services.raise_tpl(Tpl::NOTIFY);
        assert_eq!(Tpl::APPLICATION, status);
    }

    #[test]
    #[should_panic = "function not initialize."]
    fn test_restore_tpl_not_init() {
        let boot_services = boot_services!();
        let _ = boot_services.restore_tpl(Tpl::APPLICATION);
    }

    #[test]
    fn test_restore_tpl() {
        let boot_services = boot_services!(restore_tpl = efi_restore_tpl);

        extern "efiapi" fn efi_restore_tpl(tpl: efi::Tpl) {
            assert_eq!(efi::TPL_APPLICATION, tpl);
        }

        boot_services.restore_tpl(Tpl::APPLICATION);
    }

    #[test]
    #[should_panic = "function not initialize."]
    fn test_allocate_pages_not_init() {
        let boot_services = boot_services!();
        let _ = boot_services.allocate_pages(AllocType::AnyPage, MemoryType::ACPI_MEMORY_NVS, 0);
    }

    #[test]
    fn test_allocate_pages() {
        let boot_services = boot_services!(allocate_pages = efi_allocate_pages);

        extern "efiapi" fn efi_allocate_pages(
            alloc_type: u32,
            mem_type: u32,
            nb_pages: usize,
            memory: *mut u64,
        ) -> efi::Status {
            let expected_alloc_type: efi::AllocateType = AllocType::AnyPage.into();
            assert_eq!(expected_alloc_type, alloc_type);
            let expected_mem_type: efi::MemoryType = MemoryType::MEMORY_MAPPED_IO.into();
            assert_eq!(expected_mem_type, mem_type);
            assert_eq!(4, nb_pages);
            assert_ne!(ptr::null_mut(), memory);
            assert_eq!(0, unsafe { *memory });
            unsafe { ptr::write(memory, 17) }
            efi::Status::SUCCESS
        }

        let status = boot_services.allocate_pages(AllocType::AnyPage, MemoryType::MEMORY_MAPPED_IO, 4);

        assert!(matches!(status, Ok(17)));
    }

    #[test]
    fn test_allocate_pages_at_specific_address() {
        let boot_services = boot_services!(allocate_pages = efi_allocate_pages);

        extern "efiapi" fn efi_allocate_pages(
            alloc_type: u32,
            mem_type: u32,
            nb_pages: usize,
            memory: *mut u64,
        ) -> efi::Status {
            let expected_alloc_type: efi::AllocateType = AllocType::Address(17).into();
            assert_eq!(expected_alloc_type, alloc_type);
            let expected_mem_type: efi::MemoryType = MemoryType::MEMORY_MAPPED_IO.into();
            assert_eq!(expected_mem_type, mem_type);
            assert_eq!(4, nb_pages);
            assert_ne!(ptr::null_mut(), memory);
            assert_eq!(17, unsafe { *memory });
            efi::Status::SUCCESS
        }

        let status = boot_services.allocate_pages(AllocType::Address(17), MemoryType::MEMORY_MAPPED_IO, 4);
        assert!(matches!(status, Ok(17)));
    }

    #[test]
    fn test_free_pages() {
        let boot_services = boot_services!(free_pages = efi_free_pages);

        extern "efiapi" fn efi_free_pages(address: efi::PhysicalAddress, nb_pages: usize) -> efi::Status {
            assert_eq!(address, 0x100000);
            assert_eq!(nb_pages, 10);

            efi::Status::SUCCESS
        }

        let status = boot_services.free_pages(0x100000, 10);
        assert!(matches!(status, Ok(())));
    }

    #[test]
    fn test_allocate_pool() {
        let boot_services = boot_services!(allocate_pool = efi_allocate_pool);

        extern "efiapi" fn efi_allocate_pool(
            mem_type: efi::MemoryType,
            size: usize,
            buffer: *mut *mut c_void,
        ) -> efi::Status {
            let expected_mem_type: efi::MemoryType = MemoryType::MEMORY_MAPPED_IO.into();
            assert_eq!(mem_type, expected_mem_type);
            assert_eq!(size, 10);
            unsafe { ptr::write(buffer, 0x55AA as *mut c_void) };
            efi::Status::SUCCESS
        }

        let status = boot_services.allocate_pool(MemoryType::MEMORY_MAPPED_IO, 10);
        assert_eq!(status, Ok(0x55AA as *mut u8));
    }
}
