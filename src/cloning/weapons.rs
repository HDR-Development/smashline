use std::{ffi::CString, sync::atomic::Ordering};

use locks::RwLock;
use skyline::hooks::InlineCtx;

use crate::{
    cloning::RESOLVE_AS_ORIGINAL,
    create_agent::{
        LOWERCASE_WEAPON_CATEGORY_NAMES,
        LOWERCASE_WEAPON_NAMES,
        LOWERCASE_WEAPON_OWNER_NAMES,
        WEAPON_OWNER_CATEGORIES,
        WEAPON_OWNER_IDS
    }
};

pub mod modules;
pub mod vtables;

pub struct NewAgent {
    pub article_id: i32,
    pub owner_id: i32,
    pub article_name: String,
    pub owner_name: String,
    pub original_article_id: i32,
    pub original_owner_id: i32,
    pub original_article_name: String,
    pub article_name_c: CString,
    pub owner_name_c: CString,
    pub original_article_name_c: CString,
    pub use_original_code: bool,
}

pub const VANILLA_WEAPON_COUNT: usize = 0x267;

pub static NEW_AGENTS: RwLock<Vec<NewAgent>> = RwLock::new(Vec::new());

pub fn try_get_new_agent(
    new_agents: &Vec<NewAgent>,
    weapon: i32
) -> Option<&NewAgent> {
    let index = (weapon as usize).checked_sub(VANILLA_WEAPON_COUNT)?;
    new_agents.get(index)
}

/// The original (vanilla) article kind a cloned kind was made from, if `kind` is a clone.
pub fn original_kind_of(kind: i32) -> Option<i32> {
    let new_agents = NEW_AGENTS.read();
    try_get_new_agent(&new_agents, kind).map(|agent| agent.original_article_id)
}

pub fn code_dependencies_of(owner_kind: i32) -> Vec<i32> {
    let new_agents = NEW_AGENTS.read();
    let mut deps: Vec<i32> = new_agents
        .iter()
        .filter(|agent| agent.use_original_code && agent.owner_id == owner_kind && agent.original_owner_id != owner_kind)
        .map(|agent| agent.original_owner_id)
        .collect();
    deps.sort_unstable();
    deps.dedup();
    deps
}

#[skyline::from_offset(0x33b01b0)]
fn get_weapon_base_kind(kind: i32) -> i32;

#[skyline::hook(offset = 0x33b01b0)]
fn get_weapon_base_kind_hook(kind: i32) -> i32 {
    let kind = original_kind_of(kind).unwrap_or(kind);
    call_original!(kind)
}

#[skyline::hook(offset = 0x33aa790)]
fn get_static_weapon_data_hook(kind: i32) -> *const u8 {
    let kind = original_kind_of(kind).unwrap_or(kind);
    call_original!(kind)
}

#[skyline::hook(offset = 0x33b64e4, inline)]
unsafe fn weapon_init_factory_kind(ctx: &mut InlineCtx) {
    if let Some(original) = original_kind_of(ctx.registers[28].x() as i32) {
        ctx.registers[8].set_x(get_weapon_base_kind(original) as u64);
    }
}

#[skyline::hook(offset = 0x33b6b1c, inline)]
unsafe fn weapon_init_owner_category(ctx: &mut InlineCtx) {
    if let Some(original) = original_kind_of(ctx.registers[21].x() as i32) {
        let category = WEAPON_OWNER_CATEGORIES.get(original as usize).unwrap();
        ctx.registers[22].set_x(category as i64 as u64);
    }
}

fn weapon_owner_hook(ctx: &mut InlineCtx, source_register: usize, source_shift: u32, dst_register: usize) {
    let new_agents = NEW_AGENTS.read();
    let weapon_id = unsafe { (ctx.registers[source_register].x() >> source_shift) as i32 };
    let owner_id = if let Some(agent) = try_get_new_agent(&new_agents, weapon_id) {
        if RESOLVE_AS_ORIGINAL.load(Ordering::Relaxed) {
            agent.original_owner_id
        } else {
            agent.owner_id
        }
    } else {
        WEAPON_OWNER_IDS.get(weapon_id as usize).unwrap()
    };

    unsafe {
        ctx.registers[dst_register].set_x(owner_id as u64);
    }
}

fn weapon_owner_name_hook(ctx: &mut InlineCtx, source_register: usize, source_shift: u32, dst_register: usize) {
    let new_agents = NEW_AGENTS.read();
    let weapon_id = unsafe { (ctx.registers[source_register].x() >> source_shift) as i32 };
    let owner_name = if let Some(agent) = try_get_new_agent(&new_agents, weapon_id) {
        if RESOLVE_AS_ORIGINAL.load(Ordering::Relaxed) {
            LOWERCASE_WEAPON_OWNER_NAMES.get(agent.original_article_id as usize).unwrap().as_ptr()
        } else {
            agent.owner_name_c.as_ptr() as *const u8
        }
    } else {
        LOWERCASE_WEAPON_OWNER_NAMES.get(weapon_id as usize).unwrap().as_ptr()
    };

    unsafe {
        ctx.registers[dst_register].set_x(owner_name as u64);
    }
}

fn weapon_name_hook(ctx: &mut InlineCtx, source_register: usize, source_shift: u32, dst_register: usize) {
    let new_agents = NEW_AGENTS.read();
    let weapon_id = unsafe { (ctx.registers[source_register].x() >> source_shift) as i32 };
    let article_name = if let Some(agent) = try_get_new_agent(&new_agents, weapon_id) {
        if RESOLVE_AS_ORIGINAL.load(Ordering::Relaxed) {
            agent.original_article_name_c.as_ptr() as *const u8
        } else {
            agent.article_name_c.as_ptr() as *const u8
        }
    } else {
        LOWERCASE_WEAPON_NAMES.get(weapon_id as usize).unwrap().as_ptr()
    };

    unsafe {
        ctx.registers[dst_register].set_x(article_name as u64);
    }
}

/// Replaces a direct `ldrb` of the owner category table (0x455e57c) indexed by an object's kind.
fn weapon_owner_category_hook(ctx: &mut InlineCtx, source_register: usize, source_shift: u32, dst_register: usize) {
    let kind = unsafe { (ctx.registers[source_register].x() >> source_shift) as i32 };
    let kind = original_kind_of(kind).unwrap_or(kind);
    let category = WEAPON_OWNER_CATEGORIES.get(kind as usize).unwrap() as u8;

    unsafe {
        ctx.registers[dst_register].set_x(category as u64);
    }
}

/// Replaces the resource path builder's read of the weapon category name table (0x5187f08).
fn weapon_category_name_hook(ctx: &mut InlineCtx, source_register: usize, source_shift: u32, dst_register: usize) {
    let kind = unsafe { (ctx.registers[source_register].x() >> source_shift) as i32 };
    let kind = original_kind_of(kind).unwrap_or(kind);
    let name = LOWERCASE_WEAPON_CATEGORY_NAMES.get(kind as usize).unwrap();

    unsafe {
        ctx.registers[dst_register].set_x(name.as_ptr() as u64);
    }
}

macro_rules! decl_hooks {
    ($install_fn:ident => $func:expr; $($name:ident($src:expr, $shift:expr, $dst:expr, $offset:expr));*) => {
        $(
            #[skyline::hook(offset = $offset, inline)]
            unsafe fn $name(ctx: &mut InlineCtx) {
                $func(ctx, $src, $shift, $dst);
            }
        )*

        fn $install_fn() {
            $(
                let _ = skyline::patching::Patch::in_text($offset).nop();
            )*
            skyline::install_hooks!(
                $(
                    $name,
                )*
            );
        }
    }
}

decl_hooks! {
    install_weapon_owner_hooks => weapon_owner_hook;
    params(21, 0, 26, 0x33b6624 + 0x5B0);
    game_animcmd_owner(22, 0, 8, 0x33acf74 + 0x5B0);
    sound_animcmd_owner(22, 0, 8, 0x33aee34 + 0x5B0);
    effect_animcmd_owner(22, 0, 8, 0x33aded4 + 0x5B0);
    status_script_owner(22, 0, 8, 0x33ac03c + 0x5B0)
}

decl_hooks! {
    install_weapon_owner_name_hooks => weapon_owner_name_hook;
    get_file(26, 0, 25, 0x17e0a48 - 0x40);
    game_animcmd_owner_name(8, 3, 2, 0x33ace78 + 0x5B0);
    sound_animcmd_owner_name(8, 3, 2, 0x33aed38 + 0x5B0);
    effect_animcmd_owner_name(8, 3, 2, 0x33addd8 + 0x5B0);
    status_script_owner_name(8, 3, 2, 0x33abf50 + 0x5B0)
}

decl_hooks! {
    install_weapon_name_hooks => weapon_name_hook;
    get_file_weapon_name(23, 0, 22, 0x17e0894 - 0x40);
    normal_param_data(21, 0, 27, 0x33b682c + 0x5B0);
    map_collision_param_data(21, 0, 2, 0x33b69ec + 0x5B0);
    visibility_param_data(21, 0, 2, 0x33b6d10 + 0x5B0);
    game_animcmd_weapon_name(8, 3, 3, 0x33ace88 + 0x5B0);
    sound_animcmd_weapon_name(8, 3, 3, 0x33aed48 + 0x5B0);
    effect_animcmd_weapon_name(8, 3, 3, 0x33adde8 + 0x5B0);
    status_script_weapon_name(8, 3, 3, 0x33abf60 + 0x5B0)
}

decl_hooks! {
    install_weapon_category_name_hooks => weapon_category_name_hook;
    get_file_category(26, 0, 25, 0x17e0964)
}

decl_hooks! {
    install_weapon_owner_category_hooks => weapon_owner_category_hook;
    owner_category_1(8, 0, 8, 0x3db774);
    owner_category_2(8, 0, 8, 0x641b44);
    owner_category_3(9, 0, 8, 0x645890);
    owner_category_4(0, 0, 8, 0x33a0544);
    owner_category_5(9, 0, 8, 0x33a4e20);
    status_script_owner_category(22, 0, 23, 0x33ac4d0);
    game_animcmd_owner_category(22, 0, 23, 0x33ad3d8);
    effect_animcmd_owner_category(22, 0, 23, 0x33ae338);
    sound_animcmd_owner_category(22, 0, 23, 0x33af298)
}

#[repr(C)]
pub struct PocketUIParams {
    pub category: u8,
    pub padding: [u8; 3],
    pub kind: i32,
    pub type_hash: u64
}

#[skyline::hook(offset = 0x1422f50)]
unsafe extern "C" fn get_pocket_ui_param(param_1: *mut PocketUIParams) -> u64 {
    // println!("category: {}", (*param_1).category);
    // println!("padding: {:#?}", (*param_1).padding);
    // println!("kind: {:#x}", (*param_1).kind);
    // println!("padding2: {:#x}", (*param_1).type_hash);
    if (*param_1).category == 1
    && (*param_1).kind >= VANILLA_WEAPON_COUNT as i32 {
        let kind = original_kind_of((*param_1).kind).unwrap();
        (*param_1).kind = kind;
    }
    original!()(param_1)
}

pub fn install() {
    install_weapon_name_hooks();
    install_weapon_owner_hooks();
    install_weapon_owner_name_hooks();
    install_weapon_category_name_hooks();
    install_weapon_owner_category_hooks();

    vtables::install();
    modules::install();

    skyline::install_hooks!(
        get_weapon_base_kind_hook,
        get_static_weapon_data_hook,
        weapon_init_factory_kind,
        weapon_init_owner_category,
        get_pocket_ui_param,
    );
}
