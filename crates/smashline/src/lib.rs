use std::num::{NonZeroU64, NonZeroUsize};
use std::ptr::NonNull;
use std::str::Utf8Error;
use std::sync::atomic::{AtomicUsize, Ordering};

pub use smashline_macro::*;

mod builder;
mod vtable_slots;
mod weapon_modules;

pub use builder::*;
pub use vtable_slots::*;
pub use weapon_modules::*;

#[cfg(all(not(feature = "smash-rs"), feature = "skyline_smash"))]
pub use smash::{
    lib::{utility::Variadic, L2CValue},
    lua2cpp::{L2CAgentBase, L2CFighterBase, L2CFighterCommon, L2CWeaponCommon},
    phx::Hash40,
};

#[cfg(feature = "skyline_smash")]
pub use smash as skyline_smash;

#[cfg(feature = "smash-rs")]
pub use smash_rs::{
    self,
    lib::{utility::Variadic, L2CValueHack as L2CValue},
    lua2cpp::{L2CAgentBase, L2CFighterBase, L2CFighterCommon, L2CWeaponCommon},
    phx::Hash40,
};

pub use locks;

#[repr(C)]
#[derive(Copy, Clone, PartialEq, PartialOrd)]
pub enum Priority {
    Low,
    Default,
    High,
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
       match self {
           Priority::Low => write!(f, "Low Priority"),
           Priority::Default => write!(f, "Default Priority"),
           Priority::High => write!(f, "High Priority"),
       }
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct AgentEntry {
    pub hash: u64,
    pub costume_data: Vec<usize>,
}
impl AgentEntry {
    pub fn new(agent: u64, costume: Costume) -> Self {
        Self {
            hash: agent,
            costume_data: costume.as_slice().to_vec()
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct Costume {
    pub data: *const usize,
    pub len: usize,
}
impl Default for Costume {
    fn default() -> Self {
        Costume { data: std::ptr::null(), len: 0 }
    }
}

impl Costume {
    pub fn from_vec(vec: Vec<usize>) -> Self {
        let len = vec.len();
        let data = vec.as_ptr();
        std::mem::forget(vec);
        Costume { data, len }
    }

    pub fn as_slice(self) -> &'static [usize] {
        unsafe {
            if self.data.is_null() || self.len == 0 {
                return &[];
            }

            std::slice::from_raw_parts(self.data, self.len)
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum Acmd {
    Game,
    Effect,
    Sound,
    Expression,
}

impl std::fmt::Display for Acmd {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
       match self {
           Acmd::Game => write!(f, "Category Game"),
           Acmd::Effect => write!(f, "Category Effect"),
           Acmd::Sound => write!(f, "Category Sound"),
           Acmd::Expression => write!(f, "Category Expression"),
       }
    }
}

impl PartialEq<acmd_engine::asset::Category> for Acmd {
    fn eq(&self, other: &acmd_engine::asset::Category) -> bool {
        use acmd_engine::asset::Category;
        match other {
            Category::Effect => matches!(self, Acmd::Effect),
            Category::Game => matches!(self, Acmd::Game),
            Category::Sound => matches!(self, Acmd::Sound),
            Category::Expression => matches!(self, Acmd::Expression),
        }
    }
}

#[repr(i32)]
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum StatusLine {
    Pre,
    Main,
    End,
    Init,
    Exec,
    ExecStop,
    Post,
    Exit,
    MapCorrection,
    FixCamera,
    FixPosSlow,
    CheckDamage,
    CheckAttack,
    OnChangeLr,
    LeaveStop,
    NotifyEventGimmick,
    CalcParam,

    MainLoop = -1,
}

#[repr(i32)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ObjectEvent {
    Initialize,
    Finalize,
    Start,
    End,
}

#[derive(Debug)]
pub enum StringFFIError {
    Null,
    UTF8(Utf8Error),
}

#[repr(C)]
pub struct StringFFI {
    ptr: *mut u8,
    len: usize,
}

impl StringFFI {
    pub fn as_str(&self) -> Result<&str, StringFFIError> {
        if self.len == 0 {
            return Ok("");
        }

        if self.ptr.is_null() {
            return Err(StringFFIError::Null);
        }

        unsafe {
            let slice = std::slice::from_raw_parts(self.ptr, self.len);
            std::str::from_utf8(slice).map_err(StringFFIError::UTF8)
        }
    }

    pub fn from_str(value: impl Into<String>) -> Self {
        let mut bytes: String = value.into();
        bytes.shrink_to_fit();
        let leaked = bytes.into_bytes().leak();
        Self {
            ptr: leaked.as_mut_ptr(),
            len: leaked.len(),
        }
    }
}

impl Drop for StringFFI {
    fn drop(&mut self) {
        if self.ptr.is_null() && self.len != 0 {
            panic!("cannot drop null string");
        }

        unsafe {
            drop(String::from_raw_parts(self.ptr, self.len, self.len));
        }
    }
}

#[repr(C)]
pub enum LuaConst {
    Resolved(i32),
    UnresolvedHash(u64),
    UnresolvedStr(StringFFI),
}

pub trait AsHash40 {
    fn as_hash40(self) -> Hash40;
}

impl AsHash40 for String {
    fn as_hash40(self) -> Hash40 {
        Hash40::new(self.as_str())
    }
}

impl AsHash40 for &str {
    fn as_hash40(self) -> Hash40 {
        Hash40::new(self)
    }
}

#[cfg(all(not(feature = "smash-rs"), feature = "skyline_smash"))]
impl AsHash40 for u64 {
    fn as_hash40(self) -> Hash40 {
        Hash40::new_raw(self)
    }
}

#[cfg(feature = "smash-rs")]
impl AsHash40 for u64 {
    fn as_hash40(self) -> Hash40 {
        Hash40(self)
    }
}

impl AsHash40 for Hash40 {
    fn as_hash40(self) -> Hash40 {
        self
    }
}

pub trait IntoLuaConst {
    fn into_lua_const(self) -> LuaConst;
}

impl IntoLuaConst for i32 {
    fn into_lua_const(self) -> LuaConst {
        LuaConst::Resolved(self)
    }
}

impl IntoLuaConst for u64 {
    fn into_lua_const(self) -> LuaConst {
        LuaConst::UnresolvedHash(self)
    }
}

impl IntoLuaConst for &str {
    fn into_lua_const(self) -> LuaConst {
        LuaConst::UnresolvedStr(StringFFI::from_str(self))
    }
}

impl IntoLuaConst for String {
    fn into_lua_const(self) -> LuaConst {
        LuaConst::UnresolvedStr(StringFFI::from_str(self))
    }
}

#[cfg(feature = "skyline_smash")]
impl IntoLuaConst for smash::lib::LuaConst {
    fn into_lua_const(self) -> LuaConst {
        LuaConst::Resolved(*self)
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum BattleObjectCategory {
    Fighter = 0,
    Weapon,
    Enemy,
    Gimmick,
    Item,
}

impl BattleObjectCategory {
    pub fn from_battle_object_id(id: u32) -> Option<Self> {
        match id >> 0x1C {
            0x0 => Some(Self::Fighter),
            0x1 => Some(Self::Weapon),
            0x2 => Some(Self::Enemy),
            0x3 => Some(Self::Gimmick),
            0x4 => Some(Self::Item),
            _ => None,
        }
    }
}

#[repr(C)]
struct WeaponShootMsg<T> {
    size: u32,
    payload: T,
}

pub unsafe extern "C" fn send_weapon_shoot_msg<T>(weapon: *const u64, payload: T) {
    let msg = WeaponShootMsg {
        size: std::mem::size_of::<T>().next_multiple_of(8) as u32,
        payload,
    };
    // vtable slot 0x50 on the weapon
    let vtable = *(weapon as *const *const u64);
    let f: extern "C" fn(*const u64, *const WeaponShootMsg<T>) =
        std::mem::transmute(*vtable.add(0x50 / 8));
    f(weapon, &msg);
}

macro_rules! decl_imports {
    ($($V:vis fn $name:ident($($arg:ident: $T:ty),*) $(-> $Ret:ty)?;)*) => {
        $(
            $V fn $name($($arg: $T),*) $(-> $Ret)? {
                static CACHED_ADDR: AtomicUsize = AtomicUsize::new(0);
                if CACHED_ADDR.load(Ordering::Acquire) == 0 {
                    let mut value = 0usize;
                    let res = unsafe { skyline::nn::ro::LookupSymbol(&mut value, concat!(stringify!($name), "\0").as_ptr() as _) };
                    if res != 0 {
                        panic!("Failed to lookup '{}': {:#x}", stringify!($name), res);
                    } else if value == 0 {
                        panic!("Could not find '{}', please install the smashline plugin!", stringify!($name));
                    }
                    CACHED_ADDR.store(value, Ordering::Release);
                }

                let addr = CACHED_ADDR.load(Ordering::Acquire);
                let callable: extern "C" fn($($T),*) $(-> $Ret)? = unsafe {
                    std::mem::transmute(addr)
                };

                callable($($arg),*)
            }
        )*
    }
}

decl_imports! {
    fn smashline_get_original_acmd(fighter: &mut L2CAgentBase, name: Hash40) -> Option<AcmdFunction>;
    fn smashline_get_original_status(fighter: &mut L2CFighterBase, line: StatusLine, status: i32) -> Option<NonNull<()>>;

    fn smashline_reload_script(fighter: StringFFI, weapon: StringFFI, file_name: StringFFI);

    fn smashline_get_action_registry() -> &'static acmd_engine::action::ActionRegistry;

    fn smashline_install_acmd_script_costume(
        agent: Hash40,
        costume: Costume,
        script: Hash40,
        category: Acmd,
        priority: Priority,
        function: unsafe extern "C" fn(&mut L2CAgentBase)
    );

    fn smashline_install_acmd_script(
        agent: Hash40,
        script: Hash40,
        category: Acmd,
        priority: Priority,
        function: unsafe extern "C" fn(&mut L2CAgentBase)
    );

    fn smashline_install_status_script_costume(
        agent: Option<NonZeroU64>,
        costume: Costume,
        status: i32,
        line: StatusLine,
        function: *const ()
    );

    fn smashline_install_status_script(
        agent: Option<NonZeroU64>,
        status: i32,
        line: StatusLine,
        function: *const ()
    );

    fn smashline_install_line_callback_costume(
        agent: Option<NonZeroU64>,
        costume: Costume,
        line: StatusLine,
        callback: *const ()
    );

    fn smashline_install_line_callback(
        agent: Option<NonZeroU64>,
        line: StatusLine,
        callback: *const ()
    );

    fn smashline_get_target_function(
        name: StringFFI,
        offset: usize
    ) -> Option<NonZeroUsize>;

    fn smashline_install_symbol_hook(
        symbol: StringFFI,
        replacement: *const (),
        original: &'static locks::RwLock<*const ()>
    );

    fn smashline_install_state_callback_costume(
        agent: Option<NonZeroU64>,
        costume: Costume,
        event: ObjectEvent,
        callback: *const ()
    );

    fn smashline_install_state_callback(
        agent: Option<NonZeroU64>,
        event: ObjectEvent,
        callback: *const ()
    );

    fn smashline_clone_weapon(
        owner_name: StringFFI,
        article_name: StringFFI,
        original_article_id: i32,
        use_original_code: bool
    ) -> CloneWeapon;

    fn smashline_update_weapon_count(
        fighter_id: i32,
        generate_article_id: i32,
        new_count: i32
    );

    fn smashline_replace_article_descriptor_func(
        fighter_id: i32,
        generate_article_id: i32,
        on_init_callback: Option<unsafe extern "C" fn(*mut u64, *mut smash::app::BattleObjectModuleAccessor) -> i32>,
        on_fini_callback: Option<unsafe extern "C" fn(*mut u64, *mut smash::app::BattleObjectModuleAccessor) -> i32>
    );

    fn smashline_add_param_object(
        fighter_name: StringFFI,
        object: StringFFI
    );

    fn smashline_whitelist_kirby_copy_article(
        fighter_id: i32,
        generate_article_id: i32
    );

    fn smashline_replace_weapon_vtable(
        weapon_kind: i32,
        vtable_slot: usize,
        vtable_function: usize
    );

    fn smashline_replace_fighter_vtable(
        fighter_kind: i32,
        vtable_slot: usize,
        vtable_function: usize
    );

    fn smashline_get_original_weapon_vtable(
        weapon_kind: i32,
        vtable_slot: usize
    ) -> usize;

    fn smashline_get_original_fighter_vtable(
        fighter_kind: i32,
        vtable_slot: usize
    ) -> usize;

    fn smashline_replace_fighter_vtable_costume(
        fighter_kind: i32,
        costume: Costume,
        vtable_slot: usize,
        vtable_function: usize
    );

    fn smashline_clone_article_descriptor(
        fighter_id: i32,
        original_fighter_id: i32,
        generate_article_id: i32
    ) -> CloneWeapon;

    fn smashline_enable_weapon_module(
        weapon_kind: i32,
        module_slot: usize
    );

    fn smashline_set_weapon_module_factory(
        weapon_kind: i32,
        module_slot: usize,
        factory_offset: usize
    );

    fn smashline_set_weapon_lvd_nodes(
        weapon_kind: i32,
        facing_right: *const LvdNodeBinding,
        facing_right_len: usize,
        facing_left: *const LvdNodeBinding,
        facing_left_len: usize,
        follow_nodes: bool
    );

    fn smashline_load_weapon_lvd(weapon: *mut u8, file_name: StringFFI) -> bool;

    fn smashline_refresh_weapon_lvd(weapon: *mut u8);
}

pub fn original_acmd(agent: &mut L2CAgentBase, name: Hash40) -> AcmdFunction {
    smashline_get_original_acmd(agent, name)
        .unwrap_or_else(|| panic!("Failed to get original ACMD for {name:#?}"))
}

pub fn original_status<L: StatusLineMarker, T>(
    _line: L,
    fighter: &mut T,
    kind: i32,
) -> L::Function<T> {
    unsafe {
        L::cast_ptr(
            smashline_get_original_status(std::mem::transmute(fighter), L::LINE, kind)
                .unwrap_or_else(|| panic!("Failed to get original status for {kind}"))
                .as_ptr(),
        )
    }
}

#[repr(C)]
pub struct CloneWeapon {
    pub generate_id: i32,
    pub weapon_id: i32
}

pub fn clone_weapon(
    owner_name: impl Into<String>,
    article_name: impl Into<String>,
    original_article_id: i32,
    use_original_code: bool,
) -> CloneWeapon {
    smashline_clone_weapon(
        StringFFI::from_str(owner_name),
        StringFFI::from_str(article_name),
        original_article_id,
        use_original_code,
    )
}

pub fn update_weapon_count(
    fighter_id: i32,
    generate_article_id: i32,
    new_count: i32
) {
    smashline_update_weapon_count(
        fighter_id,
        generate_article_id,
        new_count
    );
}

pub fn replace_article_descriptor_func(
    fighter_id: i32,
    generate_article_id: i32,
    on_init_callback: Option<unsafe extern "C" fn(*mut u64, *mut smash::app::BattleObjectModuleAccessor) -> i32>,
    on_fini_callback: Option<unsafe extern "C" fn(*mut u64, *mut smash::app::BattleObjectModuleAccessor) -> i32>
) {
    smashline_replace_article_descriptor_func(
        fighter_id,
        generate_article_id,
        on_init_callback,
        on_fini_callback
    );
}

pub fn _clone_article_descriptor(
    fighter_id: i32,
    original_fighter_id: i32,
    generate_article_id: i32
) -> CloneWeapon {
    smashline_clone_article_descriptor(
        fighter_id,
        original_fighter_id,
        generate_article_id
    )
}

pub fn add_param_object(fighter: impl Into<String>, object: impl Into<String>) {
    smashline_add_param_object(StringFFI::from_str(fighter), StringFFI::from_str(object));
}

pub fn whitelist_kirby_copy_article(fighter_id: i32, generate_article_id: i32) {
    smashline_whitelist_kirby_copy_article(fighter_id, generate_article_id);
}

/// One lvd collision vertex pinned to a model node. The layout matches the game's own node
/// descriptor entries, so slices of these are handed to the game as they are.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LvdNodeBinding {
    /// Index of the vertex inside the lvd's collision shapes (shapes are concatenated in file order).
    pub vertex: u32,
    _pad: u32,
    /// `Hash40` of the model node the vertex follows, e.g. `virtualgrdcol01`.
    pub node: u64,
}

impl LvdNodeBinding {
    pub fn new(vertex: u32, node: Hash40) -> Self {
        // `Hash40` is a plain u64 under both smash crates, but names its field differently.
        const _: () = assert!(std::mem::size_of::<Hash40>() == std::mem::size_of::<u64>());
        Self { vertex, _pad: 0, node: unsafe { std::mem::transmute::<Hash40, u64>(node) } }
    }
}

/// Gives `weapon_kind` the lvd-capable GroundModule that Pac-Man's firehydrant and Steve's forge
/// use, with its collision vertices pinned to model nodes. It is [`set_weapon_module_factory`] for
/// [`WeaponModule::Ground`] with the hydrant's factory (`follow_nodes`) or the forge's, plus the
/// bindings; call it at plugin load, it takes effect on the kind's next weapon init.
///
/// `facing_right` / `facing_left` are the bindings for the two facing directions (`facing_left`
/// empty reuses `facing_right`); both vanilla weapons bind their two vertices to `virtualgrdcol01`
/// and `virtualgrdcol02` with the vertex indices swapped per direction. `follow_nodes` picks the
/// hydrant's GroundModule, which re-pins every frame as a weapon that moves or rotates needs; a
/// static weapon takes the forge's and can [`refresh_weapon_lvd`] itself when it is moved.
///
/// This only prepares the module. Loading the file is the plugin's job: replace the kind's
/// [`WeaponVtableSlot::WeaponGenerate3`] with [`replace_weapon_vtable`] and call
/// [`load_weapon_lvd`] from it, the way the hydrant's own slot 8 does.
///
/// A two-vertex line whose ends follow the hydrant's nodes, swapped when the weapon faces left so
/// vertex 0 stays the left end of the surface:
///
/// ```ignore
/// use smashline::{set_weapon_lvd_nodes, Hash40, LvdNodeBinding};
///
/// let col01 = Hash40::new("virtualgrdcol01");
/// let col02 = Hash40::new("virtualgrdcol02");
///
/// // facing right: vertex 1 is on col01, vertex 0 on col02
/// let facing_right = [LvdNodeBinding::new(1, col01), LvdNodeBinding::new(0, col02)];
/// // facing left: the same nodes, vertex indices swapped
/// let facing_left = [LvdNodeBinding::new(0, col01), LvdNodeBinding::new(1, col02)];
///
/// // the hydrant tips over, so it re-pins every frame
/// set_weapon_lvd_nodes(clone.weapon_id, &facing_right, &facing_left, true);
/// ```
///
/// Vertex indices count through the lvd's collision shapes in file order (shape 0's vertices first,
/// then shape 1's). A static weapon can pass `&[]` for `facing_left` to reuse `facing_right` and
/// `false` for `follow_nodes`.
pub fn set_weapon_lvd_nodes(
    weapon_kind: i32,
    facing_right: &[LvdNodeBinding],
    facing_left: &[LvdNodeBinding],
    follow_nodes: bool,
) {
    smashline_set_weapon_lvd_nodes(
        weapon_kind,
        facing_right.as_ptr(),
        facing_right.len(),
        facing_left.as_ptr(),
        facing_left.len(),
        follow_nodes,
    );
}

/// Loads `<category>/<owner>/model/<article>/c<costume>/<file_name>` (falling back to `c00`) into
/// `weapon`'s GroundModule: parses it, pins the vertices to the nodes registered with
/// [`set_weapon_lvd_nodes`] and enables the collision. `weapon` is the weapon object a
/// `WeaponGenerate3` replacement receives as its second argument.
///
/// The file needs one or two collision shapes and must already be resident (keep it next to the
/// weapon's model files). Calling it again on a live weapon rebuilds the collision. Returns whether a
/// collision now exists; the reason is logged otherwise.
pub fn load_weapon_lvd<T>(weapon: *mut T, file_name: impl Into<String>) -> bool {
    smashline_load_weapon_lvd(weapon as *mut u8, StringFFI::from_str(file_name))
}

/// Re-pins `weapon`'s lvd vertices to their nodes once, for weapons registered without
/// `follow_nodes` after they moved.
pub fn refresh_weapon_lvd<T>(weapon: *mut T) {
    smashline_refresh_weapon_lvd(weapon as *mut u8);
}

pub fn replace_weapon_vtable(weapon_kind: i32, vtable_slot: WeaponVtableSlot, vtable_function: usize) {
    smashline_replace_weapon_vtable(weapon_kind, vtable_slot as usize, vtable_function);
}

/// Makes the game create `module` for every weapon of `weapon_kind` (vanilla or clone), using the
/// game's own factory for it, instead of the shared do-nothing module such weapons get by default.
///
/// Only that kind is affected, unlike patching the factory table, whose rows are shared between
/// kinds. Call it at plugin load; it takes effect on the next weapon init of that kind.
pub fn enable_weapon_module(weapon_kind: i32, module: WeaponModule) {
    smashline_enable_weapon_module(weapon_kind, module as usize);
}

/// Like [`enable_weapon_module`], but with an explicit factory (`factory(module_accessor)`, given
/// as a text offset), for the per-kind module variants the game has (e.g. the firehydrant
/// GroundModule) or your own.
pub fn set_weapon_module_factory(weapon_kind: i32, module: WeaponModule, factory_offset: usize) {
    smashline_set_weapon_module_factory(weapon_kind, module as usize, factory_offset);
}

/// Replaces a fighter vtable entry for every costume of `fighter_kind` that no costume-specific
/// installer (scripts, callbacks or [`replace_fighter_vtable_costume`]) has claimed.
pub fn replace_fighter_vtable(fighter_kind: i32, vtable_slot: FighterVtableSlot, vtable_function: usize) {
    smashline_replace_fighter_vtable(fighter_kind, vtable_slot as usize, vtable_function);
}

/// Replaces a fighter vtable entry only for fighters spawned on one of the given costume slots.
///
/// Claiming a costume here also stops default scripts, callbacks and vtable replacements from
/// applying to it, exactly like the `_costume` script installers. Lookups the game performs without
/// a fighter instance (Kirby copy abilities, FighterManager iteration) only ever see default
/// replacements.
pub fn replace_fighter_vtable_costume(fighter_kind: i32, costume: Costume, vtable_slot: FighterVtableSlot, vtable_function: usize) {
    smashline_replace_fighter_vtable_costume(fighter_kind, costume, vtable_slot as usize, vtable_function);
}

/// Returns the original vtable function for the weapon kind and slot.
/// If you pass in a cloned weapon, it will get the base weapon's vtable funcion.
pub fn get_original_weapon_vtable(weapon_kind: i32, vtable_slot: WeaponVtableSlot) -> usize {
    smashline_get_original_weapon_vtable(weapon_kind, vtable_slot as usize)
}

/// Returns the original vtable function for the fighter kind and slot.
pub fn get_original_fighter_vtable(fighter_kind: i32, vtable_slot: FighterVtableSlot) -> usize {
    smashline_get_original_fighter_vtable(fighter_kind, vtable_slot as usize)
}

pub mod api {
    use super::*;

    #[cfg(all(not(feature = "smash-rs"), feature = "skyline_smash"))]
    fn extract_hash(hash: Hash40) -> u64 {
        hash.hash
    }

    use acmd_engine::action::Action;
    #[cfg(feature = "skyline_smash")]
    pub use smash as skyline_smash;

    #[cfg(feature = "smash-rs")]
    fn extract_hash(hash: Hash40) -> u64 {
        hash.0
    }

    pub fn reload_script(fighter: &str, weapon: Option<&str>, file: &str) {
        smashline_reload_script(
            StringFFI::from_str(fighter),
            StringFFI::from_str(weapon.unwrap_or("")),
            StringFFI::from_str(file),
        );
    }

    pub fn register_action<A: Action>() {
        smashline_get_action_registry().register::<A>();
    }

    pub fn install_status_script_costume(
        agent: Option<Hash40>,
        costume: Costume,
        line: StatusLine,
        kind: i32,
        function: *const (),
    ) {
        let agent = agent.and_then(|x| NonZeroU64::new(extract_hash(x)));
        smashline_install_status_script_costume(agent, costume, kind, line, function);
    }

    pub fn install_status_script(
        agent: Option<Hash40>,
        line: StatusLine,
        kind: i32,
        function: *const (),
    ) {
        let agent = agent.and_then(|x| NonZeroU64::new(extract_hash(x)));
        smashline_install_status_script(agent, kind, line, function);
    }

    pub fn install_line_callback_costume(
        agent: Option<Hash40>,
        costume: Costume,
        line: StatusLine,
        function: *const (),
    ) {
        let agent = agent.and_then(|x| NonZeroU64::new(extract_hash(x)));
        smashline_install_line_callback_costume(agent, costume, line, function);
    }

    pub fn install_line_callback(
        agent: Option<Hash40>,
        line: StatusLine,
        function: *const (),
    ) {
        let agent = agent.and_then(|x| NonZeroU64::new(extract_hash(x)));
        smashline_install_line_callback(agent, line, function);
    }

    pub fn install_acmd_script_costume(
        agent: Hash40,
        costume: Costume,
        script: Hash40,
        category: Acmd,
        priority: Priority,
        function: unsafe extern "C" fn(&mut L2CAgentBase),
    ) {
        smashline_install_acmd_script_costume(agent, costume, script, category, priority, function);
    }

    pub fn install_acmd_script(
        agent: Hash40,
        script: Hash40,
        category: Acmd,
        priority: Priority,
        function: unsafe extern "C" fn(&mut L2CAgentBase),
    ) {
        smashline_install_acmd_script(agent, script, category, priority, function);
    }

    pub fn get_target_function(module_name: impl Into<String>, offset: usize) -> Option<usize> {
        smashline_get_target_function(StringFFI::from_str(module_name), offset).map(|x| x.get())
    }

    pub fn install_symbol_hook(
        module_name: impl Into<String>,
        replacement: *const (),
        original: &'static locks::RwLock<*const ()>,
    ) {
        unsafe {
            smashline_install_symbol_hook(
                StringFFI::from_str(module_name),
                std::mem::transmute(replacement),
                std::mem::transmute(original),
            );
        }
    }

    pub fn install_state_callback_costume(agent: Option<Hash40>, costume: Costume, event: ObjectEvent, function: *const ()) {
        smashline_install_state_callback_costume(
            agent.and_then(|x| NonZeroU64::new(extract_hash(x))),
            costume,
            event,
            function,
        );
    }

    pub fn install_state_callback(agent: Option<Hash40>, event: ObjectEvent, function: *const ()) {
        smashline_install_state_callback(
            agent.and_then(|x| NonZeroU64::new(extract_hash(x))),
            event,
            function,
        );
    }
}
