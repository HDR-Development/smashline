use std::sync::atomic::{AtomicUsize, Ordering};

use locks::RwLock;
use skyline::hooks::InlineCtx;
use smash::app::BattleObject;
use smashline::Hash40;


static FIGHTER_CLONES: RwLock<[Option<Hash40>; 8]> = RwLock::new([None; 8]);

pub static NEW_FIGHTERS: RwLock<Vec<NewFighter>> = RwLock::new(Vec::new());

#[repr(C)]
pub struct FighterLinkedListNode {
    pub next: *mut FighterLinkedListNode,
    pub prev: *mut FighterLinkedListNode,
    pub info: FighterSelectionInfo,
    pub fighter_data_ptr: *mut u8,
    pub bigger_fighter_data_ptr: *mut u8,
}

#[repr(C)]
pub struct FighterSelectionInfo {
    pub fighter_kind: i32,
    pub costume_slot: i32,
    pub css_entry: i32,
    pub mii_related: [i32; 4],
    pub flags: [u8; 4]
}

// Very important structure !
#[repr(C)]
pub struct SelectedFighterInfo {

}

pub struct NewFighter {
    pub base_id: i32,
    pub fighter_kind_hash: Hash40,
    pub name: String,
    pub name_ffi: String,
    pub hash: Hash40,
}

pub(super) static CURRENT_PLAYER_ID: AtomicUsize = AtomicUsize::new(usize::MAX);

#[skyline::from_offset(0x3261490)]
fn lookup_fighter_kind_from_ui_hash(database: u64, hash: u64) -> i32;

#[skyline::hook(offset = 0x2310204, inline)]
unsafe fn set_current_player_id(ctx: &mut InlineCtx) {
    CURRENT_PLAYER_ID.store(*ctx.registers[21].x.as_ref() as usize, Ordering::Relaxed);

    let result = lookup_fighter_kind_from_ui_hash(*ctx.registers[0].x.as_ref(), *ctx.registers[1].x.as_ref());

    CURRENT_PLAYER_ID.store(usize::MAX, Ordering::Relaxed);

    *ctx.registers[0].x.as_mut() = result as u64;
}

#[skyline::hook(offset = 0x66cd20)]
unsafe fn lookup_fighter_kind_hash(hash: Hash40) -> i32 {
    let id = CURRENT_PLAYER_ID.load(Ordering::Relaxed);
    let mut players = FIGHTER_CLONES.write();
    for fighter in NEW_FIGHTERS.read().iter() {
        if fighter.fighter_kind_hash == hash {
            if id != usize::MAX {
                players[id] = Some(Hash40::new(&fighter.name));
            }
            return fighter.base_id;
        }
    }

    if id != usize::MAX {
        players[id] = None;
    }

    call_original!(hash)
}

// #[skyline::hook(offset = 0x66deb0)]
// unsafe fn select_fighter(manager: *const u64, info: &FighterSelectionInfo) {
//     CURRENT_PLAYER_ID.store(info.css_entry as usize, Ordering::Relaxed);
//     call_original!(manager, info);
//     CURRENT_PLAYER_ID.store(usize::MAX, Ordering::Relaxed);
// }

#[skyline::hook(offset = 0x3310760)]
unsafe fn update_selected_fighter(arg: u64, id: u32, info: *const u32) {
    CURRENT_PLAYER_ID.store(*info as usize - 1, Ordering::Relaxed);
    call_original!(arg, id, info);
    CURRENT_PLAYER_ID.store(usize::MAX, Ordering::Relaxed);
}

#[skyline::hook(offset = 0x64f884, inline)]
unsafe fn initialize_ai(ctx: &InlineCtx) {
    let fighter = *ctx.registers[21].x.as_ref() as *const i32;
    let kind = *fighter.add(0x74 / 4);
    let entry_id = *fighter.add(0x2);
    CURRENT_PLAYER_ID.store(entry_id as usize, Ordering::Relaxed);
}

#[skyline::hook(offset = 0x64f894, inline)]
unsafe fn finish_ai(ctx: &InlineCtx) {
    CURRENT_PLAYER_ID.store(usize::MAX, Ordering::Relaxed);
}

macro_rules! decl_hooks {
    ($install_fn:ident => $func:expr; $($name:ident($offset:expr, $dst:expr));*) => {
        $(
            #[skyline::hook(offset = $offset, inline)]
            unsafe fn $name(ctx: &mut InlineCtx) {
                $func(ctx, $dst);
            }
        )*

        fn $install_fn() {
            skyline::install_hooks!(
                $(
                    $name,
                )*
            );
        }
    }
}

unsafe fn handle_fighter_name(ctx: &mut InlineCtx, dst: usize) {
    let current_player = CURRENT_PLAYER_ID.load(Ordering::Relaxed);
    if current_player == usize::MAX {
        return;
    }

    let clones = FIGHTER_CLONES.read();
    let Some(clone) = clones[current_player] else {
        return;
    };

    for new_fighter in NEW_FIGHTERS.read().iter() {
        if new_fighter.hash == clone {
            *ctx.registers[dst].x.as_mut() = new_fighter.name_ffi.as_ptr() as u64;
        }
    }
}

decl_hooks! {
    install_fighter_name_hooks => handle_fighter_name;
    kirby_copy_fit(0xba446c, 2);
    kirby_copy_fit2(0xba4e70, 2);
    get_fighter_path1(0x17de9d8, 2);
    get_fighter_path2(0x17deb58, 2);
    get_fighter_path3(0x17debc4, 2);
    get_fighter_path20(0x17df220, 2);
    get_fighter_path21(0x17df864, 2);
    get_fighter_path22(0x17df928, 2);
    // get_fighter_path23(0x17e40f0, 22); // This one is for the loading fighter module
    get_fighter_path24(0x17e6a08, 2);
    get_fighter_path25(0x17e8120, 2);
    get_fighter_path26(0x17e8140, 2);
    get_fighter_path27(0x17e8268, 2);
    get_fighter_path28(0x17e8288, 2);
    get_fighter_path29(0x17e8314, 2);
    get_fighter_path30(0x17e8334, 2);
    get_fighter_path31(0x17e8518, 2);
    get_fighter_path32(0x17e86dc, 2);
    get_fighter_path35(0x17e927c, 2);
    get_fighter_path36(0x17ef578, 2);
    get_motion_list_name(0x60c158, 2);
    get_fighter_path4(0x17decb0, 2);
    get_fighter_path5(0x17decc4, 2);
    get_fighter_path6(0x17debe8, 2);
    get_fighter_path7(0x17dece0, 2);
    get_fighter_path8(0x17decf8, 2);
    get_fighter_path9(0x17ded0c, 2);
    get_fighter_path10(0x17ded28, 2);
    get_fighter_path11(0x17dec00, 2);
    get_fighter_path12(0x17dec18, 2);
    get_fighter_path13(0x17dec30, 2);
    get_fighter_path14(0x17ded64, 2);
    get_fighter_path15(0x17ded7c, 2);
    get_fighter_path16(0x17ded90, 2);
    get_fighter_path17(0x17dec54, 2);
    get_fighter_path18(0x17dec6c, 2);
    get_fighter_path19(0x17dedac, 2);
    model_path(0x17e8f90, 2);
    model_path2(0x17e90b8, 2)
}

pub fn install() {
    skyline::patching::Patch::in_text(0x2310204).nop().unwrap();
    skyline::install_hooks!(
        set_current_player_id,
        lookup_fighter_kind_hash,
        update_selected_fighter,
        initialize_ai,
        finish_ai
    );

    install_fighter_name_hooks();
}