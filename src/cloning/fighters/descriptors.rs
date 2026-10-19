use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, AtomicI32, Ordering},
};

use locks::RwLock;
use skyline::hooks::InlineCtx;
use smashline::{skyline_smash::app::BattleObjectModuleAccessor, Hash40};

use crate::cloning::RESOLVE_AS_ORIGINAL;

pub struct NewArticle {
    pub original_owner: i32,
    pub original_weapon_id: i32,
    pub new_weapon_id: i32,
    pub skip_patching: bool
}

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

// --- Article table: tell weapon and item articles apart by the descriptor flag, not by position ---
//
// `ArticleModuleInternal::Initialize` (0x3a6270) sets `article_kind_count` (+0x220) to the length of
// the leading run of weapon descriptors: it stops at the first row whose flags byte (+0x18) has bit 0
// set, which marks an item article. Every consumer then treats `index >= count` as "item article".
// That only works for tables shaped `[weapons..., items...]`; a weapon descriptor appended after an
// item one (a clone registered after an item descriptor clone, or after a fighter's vanilla item
// article) is generated through ItemManager with a weapon kind and hangs the game.
//
// The hooks below make each consumer read the row's own flag instead:
// - `Initialize` counts every row (the pool then also reserves item rows' `max_active_count`, which
//   keeps the prefix-sum slot math in the other functions consistent);
// - `InitializeArticles` / `DestroyArticles` skip item rows (they must not be instantiated as weapons);
// - `GenerateArticle` / `IsGeneratable` / `GetActiveNum` / the fini dispatch get a count of 0 (item)
//   or u32::MAX (weapon) substituted for the compare that follows the load.
// Each hook sits on the instruction right after the relevant load so the loaded register can be
// rewritten before it is used; register assignments are from the 13.0.5 disassembly.

const ARTICLE_DESCRIPTOR_SIZE: u64 = 0x20;
const ARTICLE_DESCRIPTOR_FLAGS: u64 = 0x18;
/// `ArticleModuleInternal` -> `StaticArticleInfo*`
const ARTICLE_INTERNAL_INFO: u64 = 0x228;
/// `StaticArticleInfo` -> `ArticleDescriptor*`
const ARTICLE_INFO_DESCRIPTORS: u64 = 0x0;

unsafe fn is_item_descriptor(descriptor: u64) -> bool {
    *((descriptor + ARTICLE_DESCRIPTOR_FLAGS) as *const u8) & 1 != 0
}

unsafe fn descriptor_from_info(article_info: u64, index: u64) -> u64 {
    *((article_info + ARTICLE_INFO_DESCRIPTORS) as *const u64) + index * ARTICLE_DESCRIPTOR_SIZE
}

unsafe fn descriptor_from_internal(internal: u64, index: u64) -> u64 {
    descriptor_from_info(*((internal + ARTICLE_INTERNAL_INFO) as *const u64), index)
}

/// Value substituted for `article_kind_count` so that the following `index >= count` compare takes
/// the item path for item rows and the weapon path for weapon rows.
fn article_count_for(is_item: bool) -> u64 {
    if is_item { 0 } else { u32::MAX as u64 }
}

/// `Initialize`, `tbnz w11, #0` on the row's flags byte: clear the bit so the count covers every row.
#[skyline::hook(offset = 0x3a62c0, inline)]
unsafe fn article_init_count_all_rows(ctx: &mut InlineCtx) {
    let flags = ctx.registers[11].x();
    ctx.registers[11].set_x(flags & !1);
}

/// `Initialize` pool sizing, after `ldr w12, [x11, #4]` (x11 = row, w12 = `max_active_count`): item
/// rows may use -1 for "unlimited", which the vanilla loop never saw; reserve 0 slots for those
/// instead of 4 billion. Non-negative item rows keep reserving their (unused) slots so the prefix-sum
/// slot math of the other consumers stays consistent.
#[skyline::hook(offset = 0x3a62fc, inline)]
unsafe fn article_init_pool_skip_unlimited_items(ctx: &mut InlineCtx) {
    let row = ctx.registers[11].x();
    if is_item_descriptor(row) && (ctx.registers[12].x() as u32 as i32) < 0 {
        ctx.registers[12].set_x(0);
    }
}

/// `InitializeArticles`, right after `bl GetArticleUseTypeMask`: x19 = current descriptor. Zero the
/// returned use-type mask (w0) for item rows so they are not instantiated as weapons.
#[skyline::hook(offset = 0x3a64e0, inline)]
unsafe fn article_init_skip_item_rows(ctx: &mut InlineCtx) {
    if is_item_descriptor(ctx.registers[19].x()) {
        ctx.registers[0].set_x(0);
    }
}

/// `DestroyArticles`, right after `bl GetArticleUseTypeMask`: x21 = `ArticleModuleInternal`, x22 = row.
#[skyline::hook(offset = 0x3d3ffc, inline)]
unsafe fn article_destroy_skip_item_rows(ctx: &mut InlineCtx) {
    if is_item_descriptor(descriptor_from_internal(ctx.registers[21].x(), ctx.registers[22].x())) {
        ctx.registers[0].set_x(0);
    }
}

/// `GenerateArticle`, after `ldr w9, [x0, #0x220]`: x8 = descriptor of the requested row.
#[skyline::hook(offset = 0x3a6d18, inline)]
unsafe fn article_generate_route_by_flag(ctx: &mut InlineCtx) {
    let is_item = is_item_descriptor(ctx.registers[8].x());
    ctx.registers[9].set_x(article_count_for(is_item));
}

/// `IsGeneratable`, after `ldr w10, [x21, #0x220]`: x11 = descriptor of the requested row.
#[skyline::hook(offset = 0x3d65b4, inline)]
unsafe fn article_is_generatable_route_by_flag(ctx: &mut InlineCtx) {
    let is_item = is_item_descriptor(ctx.registers[11].x());
    ctx.registers[10].set_x(article_count_for(is_item));
}

/// `GetActiveNum`, after `ldr w8, [x9, #0x220]`: x10 = `StaticArticleInfo`, w2 = row.
#[skyline::hook(offset = 0x3d677c, inline)]
unsafe fn article_active_num_route_by_flag(ctx: &mut InlineCtx) {
    let row = ctx.registers[2].x() & 0xffff_ffff;
    let is_item = is_item_descriptor(descriptor_from_info(ctx.registers[10].x(), row));
    ctx.registers[8].set_x(article_count_for(is_item));
}

/// Article fini dispatch (0x3d76c0), after `ldr w11, [x8, #0x220]`: x9 = `StaticArticleInfo`, x10 = row.
#[skyline::hook(offset = 0x3d772c, inline)]
unsafe fn article_fini_route_by_flag(ctx: &mut InlineCtx) {
    let is_item = is_item_descriptor(descriptor_from_info(ctx.registers[9].x(), ctx.registers[10].x()));
    ctx.registers[11].set_x(article_count_for(is_item));
}

pub static IS_KIRBY_COPYING: AtomicBool = AtomicBool::new(false);
pub static CURRENT_KIRBY_COPY: AtomicI32 = AtomicI32::new(-1);

pub static KIRBY_COPY_ARTICLE_WHITELIST: RwLock<BTreeMap<i32, Vec<i32>>> = RwLock::new(BTreeMap::new());

pub static FIGHTER_DATA_CACHE: RwLock<BTreeMap<i32, &'static StaticFighterData>> =
    RwLock::new(BTreeMap::new());
pub static KIRBY_COPY_DATA_CACHE: RwLock<BTreeMap<i32, &'static StaticArticleData>> =
    RwLock::new(BTreeMap::new());

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
    pub on_init_callback: unsafe extern "C" fn(*mut u64, *mut BattleObjectModuleAccessor) -> i32,
    // could also be on shoot
    pub on_fini_callback: unsafe extern "C" fn(*mut u64, *mut BattleObjectModuleAccessor) -> i32,
    pub extra: u64,
}

pub static ARTICLE_DESCRIPTOR_REPLACE: RwLock<BTreeMap<i32, BTreeMap<i32, ArticleDescriptorReplace>>> = RwLock::new(BTreeMap::new());

#[repr(C)]
pub struct ArticleDescriptorReplace {
    pub max_count: Option<i32>,
    pub on_init_callback: Option<unsafe extern "C" fn(*mut u64, *mut BattleObjectModuleAccessor) -> i32>,
    pub on_fini_callback: Option<unsafe extern "C" fn(*mut u64, *mut BattleObjectModuleAccessor) -> i32>,
}

impl ArticleDescriptorReplace {
    pub fn new() -> Self {
        Self {
            max_count: None,
            on_init_callback: None,
            on_fini_callback: None
        }
    }
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

    if RESOLVE_AS_ORIGINAL.load(Ordering::Relaxed) {
        return original_data;
    }

    if let Some(cached) = FIGHTER_DATA_CACHE.read().get(&kind) {
        return *cached as *const StaticFighterData;
    }

    let mut new_descriptors = vec![];

    new_descriptors.extend_from_slice(unsafe { (*original_data).articles_as_slice() });

    if let Some(new_articles) = NEW_ARTICLES.read().get(&kind) {

        for new_article in new_articles.iter() {
            println!("[smashline::weapons] Attempting to append article {:#x} to the article table", new_article.new_weapon_id);
            RESOLVE_AS_ORIGINAL.store(true, Ordering::Relaxed);
            let source_data = call_original!(new_article.original_owner);
            RESOLVE_AS_ORIGINAL.store(false, Ordering::Relaxed);

            unsafe {
                // bad
                if new_article.skip_patching {
                    println!(
                        "[smashline::weapons] Cloning ArticleDescriptor index {} directly from kind {:#x}",
                        new_article.original_weapon_id,
                        new_article.original_owner
                    );
                    let mut original_articles = vec![];
                    original_articles.extend_from_slice((*source_data).articles_as_slice());
                    let article = original_articles[new_article.original_weapon_id as usize];
                    new_descriptors.push(article);
                }
                else {
                    let Some(mut article) = (*source_data).get_article(new_article.original_weapon_id) else {
                        panic!("Failed to append article table");
                    };
                    println!(
                        "[smashline::weapons] Success"
                    );
                    article.weapon_id = new_article.new_weapon_id;

                    new_descriptors.push(article);
                }
            }
        }
    }

    if let Some(descriptors) = ARTICLE_DESCRIPTOR_REPLACE.read().get(&kind) {
        for (index, article) in new_descriptors.iter_mut().enumerate() {
            if let Some(descriptor) = descriptors.get(&(index as i32)) {
                if let Some(new_count) = descriptor.max_count {
                    article.max_count = new_count;
                }
                if let Some(init_callback) = descriptor.on_init_callback {
                    article.on_init_callback = init_callback;
                }
                if let Some(fini_callback) = descriptor.on_fini_callback {
                    article.on_fini_callback = fini_callback;
                }
            }
        }
    }

    // The flag-based article routing (see the inline hooks above) sizes the weapon pool with the
    // same prefix sums the game uses, but an item row with an "unlimited" (-1) count cannot be
    // represented there: a weapon row placed after one would alias an earlier weapon's pool slots.
    let mut unlimited_item_before = false;
    for (index, article) in new_descriptors.iter().enumerate() {
        let is_item = article.extra & 1 != 0;
        if !is_item && unlimited_item_before {
            println!(
                "[smashline::weapons] Fighter {:#x}: weapon article {} (kind {:#x}) comes after an item article with an unlimited count; its pool slots will overlap another article's",
                kind,
                index,
                article.weapon_id
            );
        }
        if is_item && article.max_count < 0 {
            unlimited_item_before = true;
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

pub fn install() {
    skyline::install_hooks!(
        get_static_fighter_data,
        article_init_count_all_rows,
        article_init_pool_skip_unlimited_items,
        article_init_skip_item_rows,
        article_destroy_skip_item_rows,
        article_generate_route_by_flag,
        article_is_generatable_route_by_flag,
        article_active_num_route_by_flag,
        article_fini_route_by_flag,
    );

    install_kirby_copy_kind_hooks();
    install_kirby_copy_hooks();
}
