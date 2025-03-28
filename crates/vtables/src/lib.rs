#![feature(strict_provenance)]
#![feature(exposed_provenance)]
use std::{
    alloc::Layout,
    any::{Any, TypeId},
    ops::{Deref, DerefMut},
};

pub use vtable_macro::*;

const CUSTOM_VTABLE_MAGIC: u64 = u64::from_le_bytes(*b"VRTMANIP");

pub trait VTableAccessor {
    const HAS_TYPE_INFO: bool;
}

pub trait VirtualClass: 'static {
    const DYNAMIC_MODULE: Option<&'static str>;
    const VTABLE_OFFSET: usize;
    const DISABLE_OFFSET_CHECK: bool;

    type Accessor: VTableAccessor;
    type CustomData: Default + 'static;

    fn vtable_accessor(&self) -> &Self::Accessor;
    fn vtable_accessor_mut(&mut self) -> &mut Self::Accessor;

    fn main_address() -> usize {
        match Self::DYNAMIC_MODULE {
            Some(module_name) => {
                let object =
                    rtld::find_module_by_name(module_name).expect("module is not in memory!");
                object.get_address_range(rtld::Section::Text).start as usize
            }
            None => {
                // SAFETY: This code is intended to be run on a switch console, which will only happen
                // with skyline present, so this is safe
                unsafe { skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) }
                    .expose_provenance()
            }
        }
    }
}

#[repr(C)]
struct VTableContext {
    magic: u64,
    type_id: TypeId,
    old_vtable: *const (),
    data: Box<dyn Any>,
}

impl VTableContext {
    #[track_caller]
    fn storage<T: VirtualClass + 'static>(&self) -> &T::CustomData {
        if self.type_id != std::any::TypeId::of::<T>() {
            panic!("object is not instance of {}", std::any::type_name::<T>());
        }

        self.data.downcast_ref().unwrap()
    }

    #[track_caller]
    fn storage_mut<T: VirtualClass + 'static>(&mut self) -> &mut T::CustomData {
        if self.type_id != std::any::TypeId::of::<T>() {
            panic!("object is not instance of {}", std::any::type_name::<T>());
        }

        self.data.downcast_mut().unwrap()
    }
}

/// Guard to be used when attempting to mutate a vtable's contents
///
/// This will perform checks on the vtable to ensure that it has been reallocated into a unique
/// memory location where mutation is safe.
///
/// # Compile-time Safety
/// This function does depend on the implementor of the `VirtualClass` trait to know how many
/// entries there are in the vtable.
///
/// Using the smashline vtable API, you'll notice that there **should** be a 1-1 correlation
/// between the types implementing the `VirtualClass` struct and the offset of the vtable.
///
/// This can be violated by the programmer at compile time, however hopefully this documentation
/// should serve as a sufficient troubleshooting resource.
///
/// # Runtime Safety
/// The methodology for vtable hijacking done here is designed to be safe at runtime, where panics
/// will occur instead of undefined behavior.
///
/// The `T` type parameter is passed in as a form of type safety:
/// - If this vtable has not yet been mutated, then the address of the vtable is compared against
/// the offset provided in the `VirtualClass` trait implementation. If successful, the vtable
/// will be relocated and the type id of `T` will be stored in the additional data store
/// provided by this API
/// - If this vtable has been mutated, then the type if of `T` will be compared to the one
/// stored at mutation time. If they do not match, then this function will panic.
#[track_caller]
pub fn vtable_mutation_guard<V, T: VirtualClass + DerefMut<Target = V>>(vtable: &mut &mut V) {
    let vtable_ptr = (*vtable as *const V).cast::<u64>();

    if !vtable_ptr.is_aligned() {
        panic!("object vtable is not aligned");
    }

    let needs_reloc = if T::DISABLE_OFFSET_CHECK {
        rtld::get_memory_state(vtable_ptr.expose_provenance() as u64)
            != rtld::MemoryState::DereferencableOutsideModule
    } else {
        vtable_ptr.expose_provenance() == T::main_address() + T::VTABLE_OFFSET
    };

    if needs_reloc {
        // Allocating zero bytes is undefined behavior, so just don't do it
        assert!(std::mem::size_of::<V>() > 0);

        // Total size is at least size of the vtable + 8 for our context pointer
        let mut total_size = std::mem::size_of::<V>() + 8;

        // If we have type info, that's at vtable.sub(1) so we need to add an extra 8 bytes so we
        // don't overwrite that
        if <T::Accessor as VTableAccessor>::HAS_TYPE_INFO {
            total_size += 8;
        }

        // SAFETY: Our layout is definitely non-zero size here, it is at least 8
        let new_memory =
            unsafe { std::alloc::alloc(Layout::from_size_align(total_size, 8).unwrap()) };

        assert!(!new_memory.is_null());

        let context = VTableContext {
            magic: CUSTOM_VTABLE_MAGIC,
            old_vtable: *vtable as *const _ as *const (),
            type_id: std::any::TypeId::of::<T>(),
            data: Box::new(T::CustomData::default()),
        };

        let leaked = Box::leak(Box::new(context)) as *mut VTableContext;

        // SAFETY: We ensured this is not null above, and we can rely on the allocator
        unsafe {
            *(new_memory as *mut *mut VTableContext) = leaked;
        }

        let new_vtable_offset = if <T::Accessor as VTableAccessor>::HAS_TYPE_INFO {
            // SAFETY: We've ensured this is aligned and it's definitely not null
            // unless skyline's reliable utility has failed
            let type_info_ptr = unsafe { *vtable_ptr.sub(1) };

            // SAFETY: Same as the previous deref of new_memory
            unsafe {
                *(new_memory as *mut u64).add(1) = type_info_ptr;
            }

            2
        } else {
            1
        };

        // SAFETY: Our memory is allocated, this is as safe as the last deref
        unsafe {
            let data = std::ptr::read(*vtable);
            std::ptr::write(new_memory.add(new_vtable_offset * 0x8).cast(), data);
            *(vtable as *mut &mut V as *mut *mut V) =
                new_memory.add(new_vtable_offset * 0x8).cast();
        }

        return;
    }

    // TODO: change null checks to checks if pointer is in mapped memory space
    if vtable_ptr.is_null() {
        panic!("object vtable is null");
    }

    let ctx = if <T::Accessor as VTableAccessor>::HAS_TYPE_INFO {
        // SAFETY: Switch's memory space cannot go above isize::MAX so this will be fine
        if unsafe { vtable_ptr.offset_from(std::ptr::null_mut()) } < 0x10 {
            panic!("object vtable pointer is invalid")
        }

        // SAFETY: This is safe because we've ensured that the pointer is aligned properly
        // and that it is not null, so this should not overflow
        unsafe { vtable_ptr.sub(2) }.cast::<*const VTableContext>()
    } else {
        // SAFETY: Same as above
        unsafe { vtable_ptr.sub(1) }.cast::<*const VTableContext>()
    };

    if ctx.is_null() {
        panic!("vtable context ptr is null (this should be unreachable)");
    }

    // SAFETY: This is safe because we've already ensured above that it is not null
    let context = unsafe { &**ctx };

    if context.magic != CUSTOM_VTABLE_MAGIC {
        panic!("vtable context is malformed (incorrect magic)");
    }

    if context.type_id != std::any::TypeId::of::<T>() {
        panic!(
            "object is not an instance of {}",
            std::any::type_name::<T>()
        );
    }
}

/// Guard to be used when attempting to read a vtable's contents
///
/// This method has the same stipulations and safety advice as [`vtable_mutation_guard`],
/// with the key difference being that this method will not mutate or relocate the vtable
/// in any way.
#[track_caller]
pub fn vtable_read_guard<V, T: VirtualClass + Deref<Target = V>>(vtable: &V) {
    let vtable_ptr = (vtable as *const V).cast::<u64>();

    let needs_reloc = if T::DISABLE_OFFSET_CHECK {
        rtld::get_memory_state(vtable_ptr.expose_provenance() as u64)
            != rtld::MemoryState::DereferencableOutsideModule
    } else {
        vtable_ptr.expose_provenance() == T::main_address() + T::VTABLE_OFFSET
    };

    // This vtable has not been relocated yet, so this is fine
    if needs_reloc {
        return;
    }

    if !vtable_ptr.is_aligned() {
        panic!("object vtable is not aligned");
    }

    // TODO: change null checks to checks if pointer is in mapped memory space
    if vtable_ptr.is_null() {
        panic!("object vtable is null");
    }

    let ctx = if <T::Accessor as VTableAccessor>::HAS_TYPE_INFO {
        // SAFETY: Switch's memory space cannot go above isize::MAX so this will be fine
        if unsafe { vtable_ptr.offset_from(std::ptr::null_mut()) } < 0x10 {
            panic!("object vtable pointer is invalid")
        }

        // SAFETY: This is safe because we've ensured that the pointer is aligned properly
        // and that it is not null, so this should not overflow
        unsafe { vtable_ptr.sub(2) }.cast::<*const VTableContext>()
    } else {
        // SAFETY: Same as above
        unsafe { vtable_ptr.sub(1) }.cast::<*const VTableContext>()
    };

    if ctx.is_null() {
        panic!("vtable context ptr is null (this should be unreachable)");
    }

    // SAFETY: This is safe because we've already ensured above that it is not null
    let context = unsafe { &**ctx };

    if context.magic != CUSTOM_VTABLE_MAGIC {
        panic!("vtable context is malformed (incorrect magic)");
    }

    if context.type_id != std::any::TypeId::of::<T>() {
        panic!(
            "object is not an instance of {}",
            std::any::type_name::<T>()
        );
    }
}

use thiserror::Error;
#[derive(Error, Debug)]
pub enum CustomDataAccessError {
    #[error("Vtable has not been relocated")]
    NotRelocated,

    #[error("Vtable is not aligned")]
    NotAligned,

    #[error("Vtable is null")]
    Null,

    #[error("Vtable pointer is invalid")]
    PointerInvalid,

    #[error("Vtable context pointer is null")]
    NullContext,

    #[error("Vtable magic is invalid (malformed)")]
    InvalidMagic,
}

/// Method for accessing custom data stores created for each vtable
///
/// The storage is created when the vtable is relocated, along with other identifying
/// information.
///
/// # Panics
/// - The vtable address is null
/// - The vtable address is not aligned
/// - The vtable has not already been relocated
/// - The object is not an instance of `T`
#[track_caller]
pub fn vtable_custom_data<V, T: VirtualClass + Deref<Target = V>>(
    vtable: &V,
) -> Result<&T::CustomData, CustomDataAccessError> {
    let vtable_ptr = (vtable as *const V).cast::<u64>();

    let needs_reloc = if T::DISABLE_OFFSET_CHECK {
        rtld::get_memory_state(vtable_ptr.expose_provenance() as u64)
            != rtld::MemoryState::DereferencableOutsideModule
    } else {
        vtable_ptr.expose_provenance() == T::main_address() + T::VTABLE_OFFSET
    };

    if needs_reloc {
        return Err(CustomDataAccessError::NotRelocated);
    }

    if !vtable_ptr.is_aligned() {
        return Err(CustomDataAccessError::NotAligned);
    }

    // TODO: change null checks to checks if pointer is in mapped memory space
    if vtable_ptr.is_null() {
        return Err(CustomDataAccessError::Null);
    }

    let ctx = if <T::Accessor as VTableAccessor>::HAS_TYPE_INFO {
        // SAFETY: Switch's memory space cannot go above isize::MAX so this will be fine
        if unsafe { vtable_ptr.offset_from(std::ptr::null_mut()) } < 0x10 {
            return Err(CustomDataAccessError::PointerInvalid);
        }

        // SAFETY: This is safe because we've ensured that the pointer is aligned properly
        // and that it is not null, so this should not overflow
        unsafe { vtable_ptr.sub(2) }.cast::<*const VTableContext>()
    } else {
        // SAFETY: Same as above
        unsafe { vtable_ptr.sub(1) }.cast::<*const VTableContext>()
    };

    if ctx.is_null() {
        return Err(CustomDataAccessError::NullContext);
    }

    // SAFETY: This is safe because we've already ensured above that it is not null
    let context = unsafe { &**ctx };

    if context.magic != CUSTOM_VTABLE_MAGIC {
        return Err(CustomDataAccessError::InvalidMagic);
    }

    Ok(context.storage::<T>())
}

/// Method for mutably accessing custom data stores created for each vtable
///
/// See [`vtable_custom_data`] for more information regarding data access.
#[track_caller]
pub fn vtable_custom_data_mut<V, T: VirtualClass + DerefMut<Target = V>>(
    vtable: &mut V,
) -> &mut T::CustomData {
    let vtable_ptr = (vtable as *mut V).cast::<u64>();

    let needs_reloc = if T::DISABLE_OFFSET_CHECK {
        rtld::get_memory_state(vtable_ptr.expose_provenance() as u64)
            != rtld::MemoryState::DereferencableOutsideModule
    } else {
        vtable_ptr.expose_provenance() == T::main_address() + T::VTABLE_OFFSET
    };

    if needs_reloc {
        panic!("vtable has not been relocated");
    }

    if !vtable_ptr.is_aligned() {
        panic!("object vtable is not aligned");
    }

    // TODO: change null checks to checks if pointer is in mapped memory space
    if vtable_ptr.is_null() {
        panic!("object vtable is null");
    }

    let ctx = if <T::Accessor as VTableAccessor>::HAS_TYPE_INFO {
        // SAFETY: Switch's memory space cannot go above isize::MAX so this will be fine
        if unsafe { vtable_ptr.offset_from(std::ptr::null_mut()) } < 0x10 {
            panic!("object vtable pointer is invalid")
        }

        // SAFETY: This is safe because we've ensured that the pointer is aligned properly
        // and that it is not null, so this should not overflow
        unsafe { vtable_ptr.sub(2) }.cast::<*mut VTableContext>()
    } else {
        // SAFETY: Same as above
        unsafe { vtable_ptr.sub(1) }.cast::<*mut VTableContext>()
    };

    if ctx.is_null() {
        panic!("vtable context ptr is null (this should be unreachable)");
    }

    // SAFETY: This is safe because we've already ensured above that it is not null
    let context = unsafe { &mut **ctx };

    if context.magic != CUSTOM_VTABLE_MAGIC {
        panic!("vtable context is malformed (incorrect magic)");
    }

    context.storage_mut::<T>()
}

/// Method for mutably accessing custom data stores created for each vtable
///
/// See [`vtable_custom_data`] for more information regarding data access.
#[track_caller]
pub fn vtable_restore_vtable<'a, 'b, V, T: VirtualClass + DerefMut<Target = V>>(
    vtable: &'a mut &'b mut V,
) -> &'a mut &'b mut V {
    let vtable_ptr = (*vtable as *mut V).cast::<u64>();

    let needs_reloc = if T::DISABLE_OFFSET_CHECK {
        rtld::get_memory_state(vtable_ptr.expose_provenance() as u64)
            != rtld::MemoryState::DereferencableOutsideModule
    } else {
        vtable_ptr.expose_provenance() == T::main_address() + T::VTABLE_OFFSET
    };

    if needs_reloc {
        panic!("vtable has not been relocated");
    }

    if !vtable_ptr.is_aligned() {
        panic!("object vtable is not aligned");
    }

    // TODO: change null checks to checks if pointer is in mapped memory space
    if vtable_ptr.is_null() {
        panic!("object vtable is null");
    }

    let ctx = if <T::Accessor as VTableAccessor>::HAS_TYPE_INFO {
        // SAFETY: Switch's memory space cannot go above isize::MAX so this will be fine
        if unsafe { vtable_ptr.offset_from(std::ptr::null_mut()) } < 0x10 {
            panic!("object vtable pointer is invalid")
        }

        // SAFETY: This is safe because we've ensured that the pointer is aligned properly
        // and that it is not null, so this should not overflow
        unsafe { vtable_ptr.sub(2) }.cast::<*mut VTableContext>()
    } else {
        // SAFETY: Same as above
        unsafe { vtable_ptr.sub(1) }.cast::<*mut VTableContext>()
    };

    if ctx.is_null() {
        panic!("vtable context ptr is null (this should be unreachable)");
    }

    // SAFETY: This is safe because we've already ensured above that it is not null
    let context = unsafe { &mut **ctx };

    if context.magic != CUSTOM_VTABLE_MAGIC {
        panic!("vtable context is malformed (incorrect magic)");
    }

    // Allocating zero bytes is undefined behavior, so just don't do it
    assert!(std::mem::size_of::<V>() > 0);

    // Total size is at least size of the vtable + 8 for our context pointer
    let mut total_size = std::mem::size_of::<V>() + 8;

    // If we have type info, that's at vtable.sub(1) so we need to add an extra 8 bytes so we
    // don't overwrite that
    if <T::Accessor as VTableAccessor>::HAS_TYPE_INFO {
        total_size += 8;
    }

    unsafe {
        *vtable = std::mem::transmute(context.old_vtable);
        drop(Box::from_raw(context));
        std::alloc::dealloc(
            ctx.cast(),
            Layout::from_size_align(total_size, 0x8).unwrap(),
        );
    }

    vtable
}
