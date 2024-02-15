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

pub fn try_get_new_agent(
    map: &BTreeMap<i32, Vec<NewAgent>>,
    weapon: i32,
    owner: i32,
) -> Option<&NewAgent> {
    map.get(&weapon)
        .and_then(|v| v.iter().find(|a| a.owner_id == owner))
}

pub static CURRENT_OWNER_KIND: AtomicI32 = AtomicI32::new(-1);

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

#[skyline::hook(offset = 0x64b730)]
fn get_static_fighter_data(kind: i32) -> *const StaticFighterData {
    let original_data: *const StaticFighterData = call_original!(kind);

    if let Some(new_articles) = NEW_ARTICLES.read().get(&kind) {
        let mut new_descriptors = vec![];

        new_descriptors.extend_from_slice(unsafe { (*original_data).articles_as_slice() });

        for article in new_articles.iter() {
            let source_data = call_original!(article.original_owner);

            unsafe {
                let Some(article) = (*source_data).get_article(article.weapon_id) else {
                    panic!("Failed to append article table");
                };

                new_descriptors.push(article);
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
    } else {
        original_data
    }
}

fn weapon_owner_hook(ctx: &mut InlineCtx, source_register: usize, dst_register: usize) {
    if IGNORE_NEW_AGENTS.load(Ordering::Relaxed) {
        return;
    }

    let owner = CURRENT_OWNER_KIND.load(Ordering::Relaxed);
    let agents = NEW_AGENTS.read();
    let Some(agent) = try_get_new_agent(&agents, unsafe { *ctx.registers[source_register].x.as_ref() as i32 }, owner) else {
        return;
    };

    unsafe {
        *ctx.registers[dst_register].x.as_mut() = agent.owner_id as u64;
    }
}

fn weapon_owner_name_hook(ctx: &mut InlineCtx, source_register: usize, dst_register: usize) {
    if IGNORE_NEW_AGENTS.load(Ordering::Relaxed) {
        return;
    }

    let owner = CURRENT_OWNER_KIND.load(Ordering::Relaxed);
    let agents = NEW_AGENTS.read();
    let Some(agent) = try_get_new_agent(&agents, unsafe { *ctx.registers[source_register].x.as_ref() as i32 }, owner) else {
        return;
    };

    unsafe {
        *ctx.registers[dst_register].x.as_mut() = agent.owner_name_ffi.as_ptr() as u64;
    }
}

fn weapon_name_hook(ctx: &mut InlineCtx, source_register: usize, dst_register: usize) {
    if IGNORE_NEW_AGENTS.load(Ordering::Relaxed) {
        return;
    }

    let owner = CURRENT_OWNER_KIND.load(Ordering::Relaxed);
    let agents = NEW_AGENTS.read();
    let Some(agent) = try_get_new_agent(&agents, unsafe { *ctx.registers[source_register].x.as_ref() as i32 }, owner) else {
        return;
    };

    unsafe {
        *ctx.registers[dst_register].x.as_mut() = agent.new_name_ffi.as_ptr() as u64;
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
    params(21, 26, 0x33b6878);
    game_animcmd_owner(22, 8, 0x33ad1c8);
    sound_animcmd_owner(22, 8, 0x33af088);
    effect_animcmd_owner(22, 8, 0x33ae128);
    status_script_owner(22, 8, 0x33ac290)
}

decl_hooks! {
    install_weapon_owner_name_hooks => weapon_owner_name_hook;
    get_file(26, 25, 0x17e0a4c);
    game_animcmd_owner_name(8, 2, 0x33ad0cc);
    sound_animcmd_owner_name(8, 2, 0x33aef8c);
    effect_animcmd_owner_name(8, 2, 0x33ae02c);
    status_script_owner_name(8, 2, 0x33ac1a4)
}

decl_hooks! {
    install_weapon_name_hooks => weapon_name_hook;
    get_file_weapon_name(23, 22, 0x17e098c);
    normal_param_data(21, 27, 0x33b6a80);
    map_collision_param_data(21, 2, 0x33b6c40);
    visibility_param_data(21, 2, 0x33b6f64);
    game_animcmd_weapon_name(8, 3, 0x33ad0dc);
    sound_animcmd_weapon_name(8, 3, 0x33aef9c);
    effect_animcmd_weapon_name(8, 3, 0x33ae03c);
    status_script_weapon_name(8, 3, 0x33ac1b4)
}

pub fn install() {
    install_weapon_name_hooks();
    install_weapon_owner_hooks();
    install_weapon_owner_name_hooks();
    skyline::install_hooks!(get_static_fighter_data);
}
