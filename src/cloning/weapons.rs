use std::{
    collections::BTreeMap,
    ffi::CString,
    sync::atomic::{AtomicBool, AtomicI32, Ordering},
};

use locks::RwLock;
use skyline::hooks::InlineCtx;
use smashline::{skyline_smash::app::BattleObjectModuleAccessor, Hash40};

use crate::create_agent::{LOWERCASE_FIGHTER_NAMES, LOWERCASE_WEAPON_CATEGORY_NAMES, LOWERCASE_WEAPON_NAMES, LOWERCASE_WEAPON_OWNER_NAMES, WEAPON_OWNER_CATEGORIES, WEAPON_OWNER_IDS};

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

pub struct NewArticle {
    pub original_owner: i32,
    pub original_weapon_id: i32,
    pub new_weapon_id: i32
}

pub const VANILLA_WEAPON_COUNT: usize = 0x267;

/// Highest `FIGHTER_<name>_GENERATE_ARTICLE_*` id for each vanilla fighter kind,
/// indexed by `FIGHTER_KIND_*`. `-1` means the fighter has no generated articles.
pub static MAX_GENERATE_ARTICLE_IDS: [i32; 94] = [
     5, // 0x00 mario
     0, // 0x01 donkey
     8, // 0x02 link
     8, // 0x03 samus
     9, // 0x04 samusd
     3, // 0x05 yoshi
    38, // 0x06 kirby
     5, // 0x07 fox
     5, // 0x08 pikachu
     3, // 0x09 luigi
     7, // 0x0a ness
     1, // 0x0b captain
     1, // 0x0c purin
     3, // 0x0d peach
     3, // 0x0e daisy
     1, // 0x0f koopa
     4, // 0x10 sheik
     3, // 0x11 zelda
     4, // 0x12 mariod
     5, // 0x13 pichu
     5, // 0x14 falco
    -1, // 0x15 marth
     0, // 0x16 lucina
     7, // 0x17 younglink
     1, // 0x18 ganon
     3, // 0x19 mewtwo
     0, // 0x1a roy
     0, // 0x1b chrom
     8, // 0x1c gamewatch
     2, // 0x1d metaknight
     5, // 0x1e pit
     1, // 0x1f pitb
     7, // 0x20 szerosuit
     2, // 0x21 wario
    14, // 0x22 snake
     0, // 0x23 ike
     0, // 0x24 pzenigame
     2, // 0x25 pfushigisou
     2, // 0x26 plizardon
     9, // 0x27 diddy
     9, // 0x28 lucas
     3, // 0x29 sonic
     8, // 0x2a dedede
     4, // 0x2b pikmin
     2, // 0x2c lucario
     9, // 0x2d robot
     8, // 0x2e toonlink
     4, // 0x2f wolf
    20, // 0x30 murabito
    16, // 0x31 rockman
     7, // 0x32 wiifit
     3, // 0x33 rosetta
     4, // 0x34 littlemac
     3, // 0x35 gekkouga
     8, // 0x36 palutena
     5, // 0x37 pacman
     6, // 0x38 reflet
     2, // 0x39 shulk
     8, // 0x3a koopajr
    11, // 0x3b duckhunt
     2, // 0x3c ryu
     2, // 0x3d ken
     1, // 0x3e cloud
     4, // 0x3f kamui
     5, // 0x40 bayonetta
    11, // 0x41 inkling
     1, // 0x42 ridley
     9, // 0x43 simon
     9, // 0x44 richter
     6, // 0x45 krool
    19, // 0x46 shizue
     3, // 0x47 gaogaen
     1, // 0x48 miifighter
     4, // 0x49 miiswordsman
    13, // 0x4a miigunner
     7, // 0x4b popo
     7, // 0x4c nana
     0, // 0x4d koopag
    -1, // 0x4e miienemyf
    -1, // 0x4f miienemys
     1, // 0x50 miienemyg
     3, // 0x51 packun
     7, // 0x52 jack
     9, // 0x53 brave
     9, // 0x54 buddy
     3, // 0x55 dolly
     8, // 0x56 master
    21, // 0x57 tantan
    23, // 0x58 pickel
     4, // 0x59 edge
     4, // 0x5a eflame
     7, // 0x5b elight
     4, // 0x5c demon
     7, // 0x5d trail
];

pub static NEW_ARTICLES: RwLock<BTreeMap<i32, Vec<NewArticle>>> = RwLock::new(BTreeMap::new());
pub static NEW_AGENTS: RwLock<Vec<NewAgent>> = RwLock::new(Vec::new());

pub static WEAPON_COUNT_UPDATE: RwLock<BTreeMap<i32, BTreeMap<i32, i32>>> = RwLock::new(BTreeMap::new());

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

#[skyline::hook(offset = 0x33bed40)]
fn get_weapon_specializer_hook(kind: i32) -> *const u8 {
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

pub static RESOLVE_AS_ORIGINAL: AtomicBool = AtomicBool::new(false);

pub static IS_KIRBY_COPYING: AtomicBool = AtomicBool::new(false);
pub static CURRENT_KIRBY_COPY: AtomicI32 = AtomicI32::new(-1);

pub static KIRBY_COPY_ARTICLE_WHITELIST: RwLock<BTreeMap<i32, Vec<i32>>> = RwLock::new(BTreeMap::new());

static FIGHTER_DATA_CACHE: RwLock<BTreeMap<i32, &'static StaticFighterData>> =
    RwLock::new(BTreeMap::new());
static KIRBY_COPY_DATA_CACHE: RwLock<BTreeMap<i32, &'static StaticArticleData>> =
    RwLock::new(BTreeMap::new());

pub fn invalidate_article_cache() {
    FIGHTER_DATA_CACHE.write().clear();
    KIRBY_COPY_DATA_CACHE.write().clear();
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct StaticFighterData {
    pub id: i32,
    pub static_article_info: *const StaticArticleData,
    pub unk_ptr1: *const u64,
    pub unk_ptr2: *const u64,
    pub unk_ptr3: *const u64,
    pub unk_ptr4: *const u64,
    pub unk_hash1: Hash40,
    pub unk_ptr5: *const u64,
    pub unk_uint: u32,
    pub unk_hash4: *const u64,
    pub unk_ulong: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ArticleDescriptor {
    pub weapon_id: i32,
    pub max_count: i32,
    pub on_init_callback: extern "C" fn(*const u64, *mut BattleObjectModuleAccessor) -> i32,
    // could also be on shoot
    pub on_fini_callback: extern "C" fn(*const u64, *mut BattleObjectModuleAccessor) -> i32,
    pub extra: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct StaticArticleData {
    pub descriptors: *const ArticleDescriptor,
    pub count: usize,
}

impl StaticFighterData {
    pub fn articles_as_slice(&self) -> &[ArticleDescriptor] {
        unsafe {
            if self.static_article_info.is_null() {
                return &[];
            }

            let ptr = (*self.static_article_info).descriptors;
            let count = (*self.static_article_info).count;
            if count == 0 || ptr.is_null() {
                return &[];
            }

            std::slice::from_raw_parts(ptr, count)
        }
    }

    pub fn get_article(&self, weapon_id: i32) -> Option<ArticleDescriptor> {
        self.articles_as_slice()
            .iter()
            .find(|a| a.weapon_id == weapon_id)
            .copied()
    }
}

impl StaticArticleData {
    pub fn articles_as_slice(&self) -> &[ArticleDescriptor] {
        unsafe {
            if self.count == 0 {
                return &[];
            }

            let ptr = (*self).descriptors;
            let count = (*self).count;
            if count == 0 || ptr.is_null() {
                return &[];
            }

            std::slice::from_raw_parts(ptr, count)
        }
    }

    pub fn get_article(&self, weapon_id: i32) -> Option<ArticleDescriptor> {
        self.articles_as_slice()
            .iter()
            .find(|a| a.weapon_id == weapon_id)
            .copied()
    }
}

#[skyline::hook(offset = 0x64b730)]
fn get_static_fighter_data(kind: i32) -> *const StaticFighterData {
    if let Some(cached) = FIGHTER_DATA_CACHE.read().get(&kind) {
        return *cached as *const StaticFighterData;
    }

    let original_data: *const StaticFighterData = call_original!(kind);

    let mut new_descriptors = vec![];

    new_descriptors.extend_from_slice(unsafe { (*original_data).articles_as_slice() });

    if let Some(new_articles) = NEW_ARTICLES.read().get(&kind) {

        for new_article in new_articles.iter() {
            let source_data = call_original!(new_article.original_owner);

            unsafe {
                let Some(mut article) = (*source_data).get_article(new_article.original_weapon_id) else {
                    panic!("Failed to append article table");
                };
                article.weapon_id = new_article.new_weapon_id;

                new_descriptors.push(article);
            }
        }
    }

    if let Some(count_updates) = WEAPON_COUNT_UPDATE.read().get(&kind) {
        for (index, article) in new_descriptors.iter_mut().enumerate() {
            if let Some(new_count) = count_updates.get(&(index as i32)) {
                article.max_count = *new_count;
            }
        }
    }

    let count = new_descriptors.len();
    let ptr = new_descriptors.leak().as_ptr();
    let static_article_info = Box::leak(Box::new(StaticArticleData {
        descriptors: ptr,
        count,
    }));

    let mut new_fighter_data = Box::new(unsafe { *original_data });
    new_fighter_data.static_article_info = static_article_info as *const StaticArticleData;
    let leaked: &'static StaticFighterData = Box::leak(new_fighter_data);

    FIGHTER_DATA_CACHE.write().insert(kind, leaked);

    leaked
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

macro_rules! decl_hooks_kirby_get_kind {
    ($install_fn:ident; $($name:ident($knd:expr, $offset:expr));*) => {
        $(
            #[skyline::hook(offset = $offset, inline)]
            unsafe fn $name(ctx: &mut InlineCtx) {
                let kind = ctx.registers[$knd].x() as i32;
                CURRENT_KIRBY_COPY.store(kind, Ordering::Relaxed);
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

macro_rules! decl_hooks_kirby {
    ($install_fn:ident => $func:expr; $($name:ident($str:expr, $offset:expr));*) => {
        $(
            #[skyline::hook(offset = $offset + 0x4, inline)]
            unsafe fn $name(ctx: &mut InlineCtx) {
                $func(ctx, $str);
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

decl_hooks_kirby_get_kind! {
    install_kirby_copy_kind_hooks;
    copy_setup_hook_get_kind(20, 0xba14f4);
    copy_hook_1_get_kind(9, 0xba3e0c);
    copy_hook_2_get_kind(9, 0xba400c);
    copy_hook_3_get_kind(9, 0xba405c);
    copy_hook_4_get_kind(20, 0xba5434)
}

decl_hooks_kirby! {
    install_kirby_copy_hooks => kirby_get_copy_articles;
    copy_setup_hook(23, 0xba14f4);
    copy_hook_1(9, 0xba3e0c);
    copy_hook_2(9, 0xba400c);
    copy_hook_3(9, 0xba405c);
    copy_hook_4(12, 0xba5434)
}

unsafe fn kirby_get_copy_articles(ctx: &mut InlineCtx, store_reg: usize) {
    let kind = CURRENT_KIRBY_COPY.load(Ordering::Relaxed);

    if let Some(cached) = KIRBY_COPY_DATA_CACHE.read().get(&kind) {
        ctx.registers[store_reg].set_x(*cached as *const StaticArticleData as u64);
        return;
    }

    let kirby_copy_whitelist = KIRBY_COPY_ARTICLE_WHITELIST.read();

    let kirby_descriptors = ctx.registers[store_reg].x() as *const StaticArticleData;

    let fighter_data = get_static_fighter_data(kind);
    CURRENT_KIRBY_COPY.store(-1, Ordering::Relaxed);

    let mut new_descriptors: Vec<ArticleDescriptor> = (*kirby_descriptors).articles_as_slice().to_vec();
    let fighter_articles: Vec<ArticleDescriptor> = (*fighter_data).articles_as_slice().to_vec();

    let whitelist = kirby_copy_whitelist.get(&kind);

    for (index, article) in fighter_articles.iter().enumerate() {
        let generate_article_id = index as i32;
        let whitelisted = whitelist.map_or(false, |list| list.contains(&generate_article_id));

        match new_descriptors.iter().position(|d| d.weapon_id == article.weapon_id) {
            // Kirby already has this article; a whitelisted entry takes the fighter's full descriptor.
            Some(position) => {
                if whitelisted {
                    new_descriptors[position] = *article;
                }
            }
            // Kirby lacks this article. Only fighters with a whitelist get missing entries appended.
            None => {
                if whitelist.is_none() {
                    continue;
                }
                if whitelisted {
                    new_descriptors.push(*article);
                } else {
                    // Placeholder with no callbacks; never give it a count.
                    new_descriptors.push(ArticleDescriptor {
                        weapon_id: article.weapon_id,
                        max_count: 0,
                        on_init_callback: std::mem::transmute(0u64),
                        on_fini_callback: std::mem::transmute(0u64),
                        extra: 0,
                    });
                    continue;
                }
            }
        }
    }

    let count = new_descriptors.len();
    let ptr = new_descriptors.leak().as_ptr();
    let static_article_info: &'static StaticArticleData = Box::leak(Box::new(StaticArticleData {
        descriptors: ptr,
        count,
    }));

    KIRBY_COPY_DATA_CACHE.write().insert(kind, static_article_info);

    ctx.registers[store_reg].set_x(static_article_info as *const StaticArticleData as u64);
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

    skyline::install_hooks!(
        get_static_fighter_data,
        get_weapon_base_kind_hook,
        get_static_weapon_data_hook,
        get_weapon_specializer_hook,
        weapon_init_factory_kind,
        weapon_init_owner_category,
        get_pocket_ui_param,
    );

    install_kirby_copy_kind_hooks();
    install_kirby_copy_hooks();
}
