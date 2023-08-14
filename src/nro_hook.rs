use locks::RwLock;
use skyline::hooks::A64HookFunction;

extern "C" {
    #[link_name = "_ZN2nn2ro18LoadModuleInternalEPNS0_6ModuleEPKvPvmib"]
    fn load_module_internal(
        module: &mut skyline::nn::ro::Module,
        image: *const u8,
        buffer: *mut u8,
        buffer_size: usize,
        flag: i32,
        check: bool,
    ) -> u32;
}

struct InlineHook {
    offset: usize,
    code_cave_offset: usize,
    function: extern "C" fn(&mut skyline::hooks::InlineCtx),
}

struct Hook {
    offset: usize,
    original: &'static mut *const (),
    function: *const (),
}

static HOOKS: RwLock<Vec<InlineHook>> = RwLock::new(Vec::new());
static NORMAL_HOOKS: RwLock<Vec<Hook>> = RwLock::new(Vec::new());

#[skyline::hook(replace = load_module_internal)]
unsafe fn nro_hook(
    module: &mut skyline::nn::ro::Module,
    image: *const u8,
    buffer: *mut u8,
    buffer_size: usize,
    flag: i32,
    check: bool,
) -> u32 {
    let result = call_original!(module, image, buffer, buffer_size, flag, check);

    if result != 0 {
        println!("Failed to load module with {result:#x}");
        return result;
    }

    let name = unsafe {
        skyline::try_from_c_str(&module.Name as *const u8)
            .unwrap_or_else(|_| String::from("unknown"))
    };

    if name != "common" {
        return result;
    }

    for hook in NORMAL_HOOKS.read().iter() {
        A64HookFunction((hook.offset as u64 + (*module.ModuleObject).module_base) as _, hook.function as _, (hook.original as *const *const ()).cast_mut().cast());
    }

    for hook in HOOKS.read().iter() {
        let base = unsafe { (*module.ModuleObject).module_base };

        // Fill extra instructions with nop so inline hooker doesn't try to do anything fancy
        for x in 1..6 {
            unsafe {
                skyline::patching::sky_memcpy(
                    (base + hook.code_cave_offset as u64 + x * 4) as _,
                    &0xD503201Fu32 as *const u32 as _,
                    0x4,
                );
            }
        }

        let original_instruction = unsafe { *((base + hook.offset as u64) as *const u32) };

        if original_instruction & 0xFFFFFC1F == 0xD63F0000 {
            // Patch the first offset to be br x9
            unsafe {
                skyline::patching::sky_memcpy(
                    (base + hook.code_cave_offset as u64) as _,
                    &(original_instruction & !0x00200000) as *const u32 as _,
                    0x4,
                );
            }
        } else {
            // It's a bl instruction
            let signed_offset = original_instruction & 0x3FFFFFF;

            // Perform sign extension
            let signed_offset = if signed_offset & 0x2000000 != 0 {
                signed_offset | 0xFC000000
            } else {
                signed_offset
            };

            let signed_offset = signed_offset as i32;

            let target_offset = hook.offset as isize + (signed_offset * 4) as isize;
            let new_target_offset = target_offset - hook.code_cave_offset as isize;
            let signed_offset = ((new_target_offset / 4) as i32) & 0x3FFFFFF as i32;
            let instruction = 0x14000000 | signed_offset as u32;
            skyline::patching::sky_memcpy(
                (base + hook.code_cave_offset as u64) as _,
                &instruction as *const u32 as _,
                0x4,
            );
        }

        // bl to the offset that we care about
        unsafe {
            let signed_offset = hook.code_cave_offset as isize - hook.offset as isize;
            let signed_offset = ((signed_offset / 4) as i32) & 0x3FFFFFF as i32;
            let instruction = 0x94000000 | signed_offset as u32;
            skyline::patching::sky_memcpy(
                (base + hook.offset as u64) as _,
                &instruction as *const u32 as _,
                0x4,
            );
        }

        unsafe {
            skyline::hooks::A64InlineHook(
                (base + hook.code_cave_offset as u64) as _,
                hook.function as _,
            );
        }
    }

    result
}

pub fn install() {
    skyline::install_hook!(nro_hook);
}

pub fn add_hook(
    offset: usize,
    code_cave_offset: usize,
    function: unsafe extern "C" fn(&skyline::hooks::InlineCtx),
) {
    HOOKS.write().push(InlineHook {
        offset,
        code_cave_offset,
        function: unsafe { std::mem::transmute(function) },
    });
}

pub fn add_normal_hook(
    offset: usize,
    original: &'static mut *const (),
    function: *const (),
) {
    NORMAL_HOOKS.write().push(Hook {
        offset, 
        original,
        function
    });
}

