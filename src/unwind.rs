use std::{ops::Range, cmp::Ordering, collections::BTreeMap};

use locks::RwLock;
use once_cell::sync::Lazy;
use rtld::{Section, ModuleObject};
use skyline::hooks::InlineCtx;

pub static MEMORY_REGIONS: RwLock<BTreeMap<MemoryRegionSearchKey, u64>> = RwLock::new(BTreeMap::new());

pub enum MemoryRegionSearchKey {
    Region(Range<u64>),
    Key(u64),
}

impl PartialEq for MemoryRegionSearchKey {
    fn eq(&self, other: &Self) -> bool {
        self.partial_cmp(other).unwrap() == Ordering::Equal
    }
}

impl Eq for MemoryRegionSearchKey {}

impl PartialOrd for MemoryRegionSearchKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self {
            Self::Region(range) => match other {
                Self::Region(other_range) => match range.start.cmp(&other_range.start) {
                    Ordering::Equal => Some(range.end.cmp(&other_range.end)),
                    other => Some(other),
                },
                Self::Key(key) => {
                    if range.start > *key {
                        Some(Ordering::Greater)
                    } else if range.end <= *key {
                        Some(Ordering::Less)
                    } else {
                        Some(Ordering::Equal)
                    }
                }
            },
            Self::Key(key) => match other {
                Self::Region(range) => {
                    if range.start > *key {
                        Some(Ordering::Less)
                    } else if range.end <= *key {
                        Some(Ordering::Greater)
                    } else {
                        Some(Ordering::Equal)
                    }
                }
                Self::Key(other_key) => Some(key.cmp(other_key)),
            },
        }
    }
}

impl Ord for MemoryRegionSearchKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

extern "C" {
    #[link_name = "_Unwind_GetIP"]
    fn get_ip(context: *const u64) -> u64;

    #[link_name = "_Unwind_SetIP"]
    fn set_ip(context: *const u64, ip: u64);
}

static NNSDK_MODULE: Lazy<&'static mut ModuleObject> = Lazy::new(|| {
    let Some(nnsdk) = rtld::find_module_for_address(get_ip as *const () as u64, Section::Text) else {
        panic!("Failed to find ModuleObject for nnSdk");
    };

    nnsdk
});

pub fn step_with_dwarf(address_space: *mut u64, ip: u64, unwind_info: *mut u64, registers: *mut u64) -> u64 {
    let ptr: extern "C" fn(*mut u64, u64, *mut u64, *mut u64) -> u64 = unsafe {
        std::mem::transmute(NNSDK_MODULE.get_address_range(Section::Text).start + 0x51ef30)
    };

    ptr(address_space, ip, unwind_info, registers)
}

pub fn set_info_based_on_ip_register(arg1: *mut u64, arg2: bool) {
    let ptr: extern "C" fn(*mut u64, bool) = unsafe {
        std::mem::transmute(NNSDK_MODULE.get_address_range(Section::Text).start + 0x51ed50)
    };

    ptr(arg1, arg2)
}

pub fn install_unwind_patches() {
    Lazy::force(&NNSDK_MODULE);

    unsafe {
        UNWIND_CURSOR_STEP = NNSDK_MODULE.get_address_range(Section::Text).start + 0x51ec68;
        BAD_INFO_CHECK = NNSDK_MODULE.get_address_range(Section::Text).start + 0x51e5dc;
    }

    skyline::install_hooks!(
        prevent_bad_info_check,
        step_replace
    );
}

static mut BAD_INFO_CHECK: u64 = 0;

#[skyline::hook(replace = BAD_INFO_CHECK, inline)]
pub fn prevent_bad_info_check(ctx: &mut InlineCtx) {
    extern "C" fn stub() {}

    let ip = unsafe {
        get_ip(*ctx.registers[0].x.as_ref() as *const u64)
    };

    if MEMORY_REGIONS.read().contains_key(&MemoryRegionSearchKey::Key(ip)) {
        unsafe {
            *ctx.registers[8].x.as_mut() = stub as *const () as u64;
        }
    }
}

static mut UNWIND_CURSOR_STEP: u64 = 0;

#[skyline::hook(replace = UNWIND_CURSOR_STEP)]
unsafe fn step_replace(this: *mut u64) -> u64 {
    const UNW_STEP_END: u64 = 0;
    const UNW_STEP_SUCCESS: u64 = 1;

    if *(this as *const bool).offset(0x268) {
        return UNW_STEP_END;
    }

    let address_space = *this.add(1) as *mut u64;
    let ip = get_ip(this);
    let unwind_info = *this.add(0x4B) as *mut u64;
    let registers = this.add(2);

    let result = step_with_dwarf(address_space, ip, unwind_info, registers);

    if result != UNW_STEP_SUCCESS {
        return result;
    }

    let ip = get_ip(this);
    
    let landing_pad = MEMORY_REGIONS.read().get(&MemoryRegionSearchKey::Key(ip)).copied();
    
    if let Some(landing_pad) = landing_pad {
        let unwind_info_ptr = this.add(0x44);
        *unwind_info_ptr.add(0) = ip as u64;
        *unwind_info_ptr.add(1) = landing_pad;
        *unwind_info_ptr.add(3) = custom_eh_personality as *const () as u64;
        result
    } else {
        set_info_based_on_ip_register(this, true);
        if *(this as *const bool).offset(0x268) {
            UNW_STEP_END
        } else {
            result
        }
    }
}

unsafe extern "C" fn custom_eh_personality(version: i32, actions: u64, _: u64, _: *mut u64, context: *mut u64) -> u64 {
    const _UA_SEARCH_PHASE: u64 = 1;
    const _URC_HANDLER_FOUND: u64 = 6;
    const _URC_INSTALL_CONTEXT: u64 = 7;

    if version != 1 {
        panic!("Custom EH personality routine called in the wrong context.");
    }

    if actions & _UA_SEARCH_PHASE != 0 {
        _URC_HANDLER_FOUND
    } else {
        let unwind_info = context.add(0x44);
        let landing_pad = *unwind_info.add(1) + 4;

        set_ip(context, landing_pad);
        _URC_INSTALL_CONTEXT
    }
}