use resources::smash_arc::{self, SearchLookup};
use skyline::hooks::InlineCtx;
use smashline::Hash40;

#[allow(unused)]
fn effect_manager() -> *mut u64 {
    let text = unsafe { skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as *mut u8 };
    unsafe { **(text.add(0x5333920).cast::<*mut *mut u64>()) }
}

#[skyline::from_offset(0x3563720)]
fn unload_effects(manager: *mut u64, handle: u32);

#[skyline::from_offset(0x355f8f0)]
fn load_effects(manager: *mut u64, handle: u32, search_index: &u32) -> u32;

#[skyline::hook(offset = 0x60bfd8, inline)]
unsafe fn load_fighter_effects(ctx: &InlineCtx) {
    let search_index_begin = &*(ctx.registers[2].x() as *const u32);
    let _result = load_effects(
        ctx.registers[0].x() as _,
        ctx.registers[1].x() as u32,
        search_index_begin,
    );

    let info = resources::types::FilesystemInfo::instance().unwrap();

    let search = info.search();

    let path_name = search.get_path_list()[*search_index_begin as usize];

    let full_name = Hash40(path_name.path.hash40().0).concat_str("/transplant");

    let Ok(first_child) = search.get_first_child_in_folder(smash_arc::Hash40(full_name.0)) else {
        return
    };

    let mut current_child = first_child;

    let mut num_transplants = 0;
    loop {
        let Ok(path_index) = search.get_path_index_from_hash(smash_arc::Hash40(current_child.path.hash40().0)) else {
            break;
        };

        let index = path_index.index();

        num_transplants += 1;

        let _result = load_effects(
            ctx.registers[0].x() as _,
            ctx.registers[1].x() as u32 + num_transplants * 2000,
            &index,
        );

        let Ok(next_child) = search.get_next_child_in_folder(current_child) else {
            break;
        };

        current_child = next_child;
    }
}

pub fn install_effect_transplant_hooks() {
    skyline::patching::Patch::in_text(0x60bfd8).nop().unwrap();
    skyline::install_hook!(load_fighter_effects);
}
