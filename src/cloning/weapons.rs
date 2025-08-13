use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, AtomicI32, Ordering},
};

use locks::RwLock;
use skyline::hooks::InlineCtx;
use smashline::{skyline_smash::app::BattleObjectModuleAccessor, Hash40};

pub struct NewAgent {
    pub old_owner_id: i32,
    pub owner_id: i32,
    pub owner_name_ffi: String,
    pub new_name_ffi: String,
    pub owner_name: String,
    pub new_name: String,
    pub old_name: String,
    pub use_original_code: bool,
}

pub struct NewArticle {
    pub original_owner: i32,
    pub weapon_id: i32,
}

pub static NEW_ARTICLES: RwLock<BTreeMap<i32, Vec<NewArticle>>> = RwLock::new(BTreeMap::new());
pub static NEW_AGENTS: RwLock<BTreeMap<i32, Vec<NewAgent>>> = RwLock::new(BTreeMap::new());
pub static IGNORE_NEW_AGENTS: AtomicBool = AtomicBool::new(false);

pub static WEAPON_COUNT_UPDATE: RwLock<BTreeMap<i32, i32>> = RwLock::new(BTreeMap::new());

pub fn try_get_new_agent(
    map: &BTreeMap<i32, Vec<NewAgent>>,
    weapon: i32,
    owner: i32,
) -> Option<&NewAgent> {
    map.get(&weapon)
        .and_then(|v| v.iter().find(|a| a.owner_id == owner))
}

pub static CURRENT_OWNER_KIND: AtomicI32 = AtomicI32::new(-1);

pub static IS_KIRBY_COPYING: AtomicBool = AtomicBool::new(false);
pub static CURRENT_KIRBY_COPY: AtomicI32 = AtomicI32::new(-1);

pub static KIRBY_COPY_ARTICLE_WHITELIST: RwLock<BTreeMap<i32, Vec<i32>>> = RwLock::new(BTreeMap::new());

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
    let original_data: *const StaticFighterData = call_original!(kind);

    if IS_KIRBY_COPYING.load(Ordering::Relaxed) {
        return original_data;
    }

    let mut new_descriptors = vec![];

    new_descriptors.extend_from_slice(unsafe { (*original_data).articles_as_slice() });

    for article in new_descriptors.iter_mut() {
        let weapon_count = WEAPON_COUNT_UPDATE.read();
        if let Some(new_count) = weapon_count.get(&article.weapon_id) {
            article.max_count = *new_count;
        }
    }

    if let Some(new_articles) = NEW_ARTICLES.read().get(&kind) {

        for article in new_articles.iter() {
            let source_data = call_original!(article.original_owner);

            unsafe {
                let Some(article) = (*source_data).get_article(article.weapon_id) else {
                    panic!("Failed to append article table");
                };

                new_descriptors.push(article);
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
    Box::leak(new_fighter_data)
}

fn weapon_owner_hook(ctx: &mut InlineCtx, source_register: usize, dst_register: usize) {
    if IGNORE_NEW_AGENTS.load(Ordering::Relaxed) {
        return;
    }

    let owner = CURRENT_OWNER_KIND.load(Ordering::Relaxed);
    let agents = NEW_AGENTS.read();
    let Some(agent) = try_get_new_agent(&agents, unsafe { ctx.registers[source_register].x() as i32 }, owner) else {
        return;
    };

    unsafe {
        ctx.registers[dst_register].set_x(agent.owner_id as u64);
    }
}

fn weapon_owner_name_hook(ctx: &mut InlineCtx, source_register: usize, dst_register: usize) {
    if IGNORE_NEW_AGENTS.load(Ordering::Relaxed) {
        return;
    }

    let owner = CURRENT_OWNER_KIND.load(Ordering::Relaxed);
    let agents = NEW_AGENTS.read();
    let Some(agent) = try_get_new_agent(&agents, unsafe { ctx.registers[source_register].x() as i32 }, owner) else {
        return;
    };

    unsafe {
        ctx.registers[dst_register].set_x(agent.owner_name_ffi.as_ptr() as u64);
    }
}

fn weapon_name_hook(ctx: &mut InlineCtx, source_register: usize, dst_register: usize) {
    if IGNORE_NEW_AGENTS.load(Ordering::Relaxed) {
        return;
    }

    let owner = CURRENT_OWNER_KIND.load(Ordering::Relaxed);
    let agents = NEW_AGENTS.read();
    let Some(agent) = try_get_new_agent(&agents, unsafe { ctx.registers[source_register].x() as i32 }, owner) else {
        return;
    };

    unsafe {
        ctx.registers[dst_register].set_x(agent.new_name_ffi.as_ptr() as u64);
    }
}

macro_rules! decl_hooks {
    ($install_fn:ident => $func:expr; $($name:ident($src:expr, $dst:expr, $offset:expr));*) => {
        $(
            #[skyline::hook(offset = $offset, inline)]
            unsafe fn $name(ctx: &mut InlineCtx) {
                $func(ctx, $src, $dst);
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

decl_hooks! {
    install_weapon_owner_hooks => weapon_owner_hook;
    params(21, 26, 0x33b6628);
    game_animcmd_owner(22, 8, 0x33acf78);
    sound_animcmd_owner(22, 8, 0x33aee38);
    effect_animcmd_owner(22, 8, 0x33aded8);
    status_script_owner(22, 8, 0x33ac040)
}

decl_hooks! {
    install_weapon_owner_name_hooks => weapon_owner_name_hook;
    get_file(26, 25, 0x17e0a4c);
    game_animcmd_owner_name(8, 2, 0x33ace7c);
    sound_animcmd_owner_name(8, 2, 0x33aed3c);
    effect_animcmd_owner_name(8, 2, 0x33adddc);
    status_script_owner_name(8, 2, 0x33abf54)
}

decl_hooks! {
    install_weapon_name_hooks => weapon_name_hook;
    get_file_weapon_name(23, 22, 0x17e098c);
    normal_param_data(21, 27, 0x33b6830);
    map_collision_param_data(21, 2, 0x33b69f0);
    visibility_param_data(21, 2, 0x33b6d14);
    game_animcmd_weapon_name(8, 3, 0x33ace8c);
    sound_animcmd_weapon_name(8, 3, 0x33aed4c);
    effect_animcmd_weapon_name(8, 3, 0x33addec);
    status_script_weapon_name(8, 3, 0x33abf64)
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
    let kirby_copy_whitelist = KIRBY_COPY_ARTICLE_WHITELIST.read();
    if let Some(whitelist) = kirby_copy_whitelist.get(&kind) {
        // println!("Fighter {:#x} is in the whitelist!", kind);
        let original_descriptors = ctx.registers[store_reg].x() as *const StaticArticleData;
        IS_KIRBY_COPYING.store(true, Ordering::Relaxed);
        let fighter_data = get_static_fighter_data(kind);
        CURRENT_KIRBY_COPY.store(-1, Ordering::Relaxed);
        IS_KIRBY_COPYING.store(false, Ordering::Relaxed);
    
        let mut new_descriptors = vec![];

        for article in  (*original_descriptors).articles_as_slice().iter() {
            new_descriptors.push(*article);
        }

        for article in (*fighter_data).articles_as_slice().iter() {
            if whitelist.contains(&article.weapon_id) {
                // println!("Whitelist contains article {:#x}", article.weapon_id);
                for descriptor in new_descriptors.iter_mut() {
                    if article.weapon_id == descriptor.weapon_id {
                        *descriptor = *article;
                    }
                }
            }
        }
    
        let count = new_descriptors.len();
        let ptr = new_descriptors.leak().as_ptr();
        let static_article_info = Box::leak(Box::new(StaticArticleData {
            descriptors: ptr,
            count,
        }));
    
        ctx.registers[store_reg].set_x(static_article_info as *const StaticArticleData as u64);
    }
}

pub fn install() {
    install_weapon_name_hooks();
    install_weapon_owner_hooks();
    install_weapon_owner_name_hooks();

    skyline::install_hooks!(
        get_static_fighter_data
    );

    install_kirby_copy_kind_hooks();
    install_kirby_copy_hooks();
}
