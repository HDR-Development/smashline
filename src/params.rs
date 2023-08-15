use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicI32, Ordering},
        Arc,
    },
};

use prc::ParamKind;
use skyline::hooks::InlineCtx;
use smashline::Hash40;
use vtables::{vtable, VirtualClass};

use std::ops::{Deref, DerefMut};

use crate::cloning::weapons::NEW_AGENTS;

#[vtable]
mod param_data_adapter {
    pub fn is_int(&self) -> bool;
    pub fn is_float(&self) -> bool;
    pub fn is_unk(&self) -> [u64; 2];
    pub fn is_hash(&self) -> bool;
    pub fn is_unk2(&self) -> [u64; 2];
    pub fn is_unk3(&self) -> [u64; 2];
    pub fn get_int(&self) -> i32;
    pub fn get_float(&self) -> f32;
    pub fn get_string(&self) -> *const u8;
    pub fn get_hash(&self) -> u64;
    pub fn get_by_key(&self, key: u64) -> Self;
    pub fn unk4(&self) -> [u64; 2];
    pub fn get_element(&self, element: i32) -> Self;
    pub fn set_int(&mut self, val: i32);
    pub fn set_float(&mut self, val: f32);
}

#[vtable]
mod fighter_param_holder {
    fn destructor(&mut self);
    fn deleter(&mut self);
    fn get_param_object(&self, key: u64) -> SmashlineParamDataAdapter;
    fn clone(&self) -> *mut Self;
    fn parse_param_data(&mut self, stream: *const u8);
    fn return_0(&self) -> i32;
}

#[repr(C)]
struct FighterParamHolder {
    vtable: FighterParamHolderVTableAccessor,
}

#[derive(Clone, Default)]
struct FighterParamAdditionalData {
    original_clone: Option<extern "C" fn(&FighterParamHolder) -> *mut FighterParamHolder>,
    original_lookup_param:
        Option<extern "C" fn(&FighterParamHolder, u64) -> SmashlineParamDataAdapter>,
    params: HashMap<u64, Arc<prc::ParamKind>>,
    map_keys: HashMap<u64, u64>,
}

impl Deref for FighterParamHolder {
    type Target = FighterParamHolderVTable;

    fn deref(&self) -> &Self::Target {
        &self.vtable.0
    }
}

impl DerefMut for FighterParamHolder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.vtable.0
    }
}

impl VirtualClass for FighterParamHolder {
    type Accessor = FighterParamHolderVTableAccessor;
    type CustomData = FighterParamAdditionalData;

    const DYNAMIC_MODULE: Option<&'static str> = None;
    const VTABLE_OFFSET: usize = 0x0;
    const DISABLE_OFFSET_CHECK: bool = true;

    fn vtable_accessor(&self) -> &Self::Accessor {
        &self.vtable
    }

    fn vtable_accessor_mut(&mut self) -> &mut Self::Accessor {
        &mut self.vtable
    }
}

#[repr(C)]
struct SmashlineParamDataAdapter {
    vtable: &'static ParamDataAdapterVTable,
    data: Option<&'static mut prc::ParamKind>,
}

impl SmashlineParamDataAdapter {
    pub extern "C" fn is_int(&self) -> bool {
        use ParamKind::*;
        let Some(data) = self.data.as_ref() else {
            return false;
        };

        match data {
            Bool(_) | I8(_) | U8(_) | I16(_) | U16(_) | I32(_) | U32(_) => true,
            _ => false,
        }
    }

    pub extern "C" fn is_float(&self) -> bool {
        matches!(self.data.as_ref(), Some(ParamKind::Float(_)))
    }

    pub extern "C" fn is_hash(&self) -> bool {
        matches!(self.data.as_ref(), Some(ParamKind::Hash(_)))
    }

    pub extern "C" fn get_int(&self) -> i32 {
        use ParamKind::*;

        let Some(data) = self.data.as_ref() else {
            return 0
        };

        match data {
            Bool(b) => *b as i32,
            I8(i) => *i as i32,
            U8(i) => *i as i32,
            I16(i) => *i as i32,
            U16(i) => *i as i32,
            I32(i) => *i as i32,
            U32(i) => *i as i32,
            _ => 0,
        }
    }

    pub extern "C" fn get_float(&self) -> f32 {
        match self.data.as_ref() {
            Some(ParamKind::Float(f)) => *f,
            _ => 0.0,
        }
    }

    pub extern "C" fn get_hash(&self) -> u64 {
        match self.data.as_ref() {
            Some(ParamKind::Hash(hash)) => hash.0,
            _ => 0,
        }
    }

    pub extern "C" fn get_string(&self) -> *const u8 {
        b"\0".as_ptr()
    }

    fn get_by_key_optional(&self, key: u64) -> Option<Self> {
        let data = self.data.as_ref()?;

        match data {
            ParamKind::Struct(data) => {
                let (_, val) = data.0.iter().find(|(k, _)| k.0 == key)?;
                Some(SmashlineParamDataAdapter {
                    vtable: self.vtable,
                    data: Some(unsafe { &mut *(val as *const ParamKind).cast_mut() }),
                })
            }
            ParamKind::List(list) => {
                let first = list.0.first()?;
                let ParamKind::Struct(data) = first else {
                    return None;
                };

                let (_, val) = data.0.iter().find(|(k, _)| k.0 == key)?;
                Some(SmashlineParamDataAdapter {
                    vtable: self.vtable,
                    data: Some(unsafe { &mut *(val as *const ParamKind).cast_mut() }),
                })
            }
            _ => None,
        }
    }

    pub extern "C" fn get_by_key(&self, key: u64) -> Self {
        self.get_by_key_optional(key).unwrap_or(Self {
            vtable: self.vtable,
            data: None,
        })
    }

    fn get_element_optional(&self, element: i32) -> Option<Self> {
        let data = self.data.as_ref()?;

        let ParamKind::List(data) = data else {
            return None;
        };

        let param = data.0.get(element as usize)?;
        Some(Self {
            vtable: self.vtable,
            data: Some(unsafe { &mut *(param as *const ParamKind).cast_mut() }),
        })
    }

    pub extern "C" fn get_element(&self, element: i32) -> Self {
        self.get_element_optional(element).unwrap_or(Self {
            vtable: self.vtable,
            data: None,
        })
    }

    pub extern "C" fn set_int(&mut self, value: i32) {
        use ParamKind::*;

        let Some(data) = self.data.as_mut() else {
            return;
        };

        match data {
            Bool(b) => *b = value != 0,
            I8(i) => *i = value as i8,
            U8(i) => *i = value as u8,
            I16(i) => *i = value as i16,
            U16(i) => *i = value as u16,
            I32(i) => *i = value,
            U32(i) => *i = value as u32,
            _ => {}
        }
    }

    pub extern "C" fn set_float(&mut self, value: f32) {
        if let Some(ParamKind::Float(f)) = self.data.as_mut() {
            *f = value;
        }
    }

    #[allow(improper_ctypes_definitions)]
    pub extern "C" fn stub(&self) -> [u64; 2] {
        [0, 0]
    }
}

static ADAPTER_VTABLE: &'static ParamDataAdapterVTable = &ParamDataAdapterVTable {
    is_int: unsafe { std::mem::transmute(SmashlineParamDataAdapter::is_int as *const ()) },
    is_float: unsafe { std::mem::transmute(SmashlineParamDataAdapter::is_float as *const ()) },
    is_unk: unsafe { std::mem::transmute(SmashlineParamDataAdapter::stub as *const ()) },
    is_hash: unsafe { std::mem::transmute(SmashlineParamDataAdapter::is_hash as *const ()) },
    is_unk2: unsafe { std::mem::transmute(SmashlineParamDataAdapter::stub as *const ()) },
    is_unk3: unsafe { std::mem::transmute(SmashlineParamDataAdapter::stub as *const ()) },
    get_int: unsafe { std::mem::transmute(SmashlineParamDataAdapter::get_int as *const ()) },
    get_float: unsafe { std::mem::transmute(SmashlineParamDataAdapter::get_float as *const ()) },
    get_string: unsafe { std::mem::transmute(SmashlineParamDataAdapter::get_string as *const ()) },
    get_hash: unsafe { std::mem::transmute(SmashlineParamDataAdapter::get_hash as *const ()) },
    get_by_key: unsafe { std::mem::transmute(SmashlineParamDataAdapter::get_by_key as *const ()) },
    unk4: unsafe { std::mem::transmute(SmashlineParamDataAdapter::stub as *const ()) },
    get_element: unsafe {
        std::mem::transmute(SmashlineParamDataAdapter::get_element as *const ())
    },
    set_int: unsafe { std::mem::transmute(SmashlineParamDataAdapter::set_int as *const ()) },
    set_float: unsafe { std::mem::transmute(SmashlineParamDataAdapter::set_float as *const ()) },
};

extern "C" fn get_param_reimpl(object: &FighterParamHolder, key: u64) -> SmashlineParamDataAdapter {
    let data = vtables::vtable_custom_data::<_, FighterParamHolder>(object.vtable.0);

    let key = data.map_keys.get(&key).copied().unwrap_or(key);

    if let Some(data) = data.params.get(&key) {
        SmashlineParamDataAdapter {
            vtable: ADAPTER_VTABLE,
            data: unsafe { Some(&mut *(Arc::as_ptr(data).cast_mut())) },
        }
    } else {
        (data.original_lookup_param.unwrap())(object, key)
    }
}

extern "C" fn clone_reimpl(object: &FighterParamHolder) -> *mut FighterParamHolder {
    let data = vtables::vtable_custom_data::<_, FighterParamHolder>(object.vtable.0);

    let original_clone = data.original_clone.unwrap();

    let new_data = original_clone(object);

    unsafe {
        (*new_data)
            .vtable_accessor_mut()
            .set_clone::<FighterParamHolder>(std::mem::transmute(clone_reimpl as *const ()));
        (*new_data)
            .vtable_accessor_mut()
            .set_get_param_object(get_param_reimpl);
        let new_custom_data =
            vtables::vtable_custom_data_mut::<_, FighterParamHolder>((*new_data).vtable.0);
        *new_custom_data = data.clone();
    }

    new_data
}

static CREATED_FIGHTER: AtomicI32 = AtomicI32::new(-1);

#[skyline::hook(offset = 0x70c560)]
unsafe fn fighter_create_param_object(arg: u64, kind: i32, fpi: &i32) -> bool {
    CREATED_FIGHTER.store(kind, Ordering::Relaxed);
    let val = call_original!(arg, kind, fpi);
    CREATED_FIGHTER.store(-1, Ordering::Relaxed);
    val
}

#[skyline::hook(offset = 0x371f934, inline)]
unsafe fn init_fighter_p_object(ctx: &InlineCtx) {
    let func: extern "C" fn(u64, u64) = std::mem::transmute(*ctx.registers[8].x.as_ref());

    func(*ctx.registers[0].x.as_ref(), *ctx.registers[1].x.as_ref());

    let fighter_id = CREATED_FIGHTER.load(Ordering::Relaxed);

    if fighter_id < 0 {
        return;
    }

    let Some(_) = crate::create_agent::LOWERCASE_FIGHTER_NAMES.get(fighter_id as usize) else {
        return;
    };

    let param_holder = *ctx.registers[0].x.as_ref() as *mut FighterParamHolder;

    let original_clone = (*param_holder)
        .vtable_accessor()
        .get_clone::<FighterParamHolder>();
    let original_get_param = (*param_holder)
        .vtable_accessor()
        .get_get_param_object::<FighterParamHolder>();

    (*param_holder)
        .vtable_accessor_mut()
        .set_clone::<FighterParamHolder>(std::mem::transmute(clone_reimpl as *const ()));
    (*param_holder)
        .vtable_accessor_mut()
        .set_get_param_object(get_param_reimpl);

    let prc_stream = *((*ctx.registers[1].x.as_ref() + 0x18) as *const *const u8);
    let prc_data = prc::read_stream(&mut std::io::Cursor::new(std::slice::from_raw_parts(
        prc_stream,
        u32::MAX as usize,
    )))
    .unwrap();

    let mut allowed_names = vec![];
    let mut remap_names = HashMap::new();

    if let Some(agents) = NEW_AGENTS.read().get(&fighter_id) {
        for agent in agents.iter() {
            allowed_names.push(Hash40::new(&format!("param_{}", agent.new_name)).0);
            remap_names.insert(
                Hash40::new(&format!("param_{}", agent.old_name)).0,
                Hash40::new(&format!("param_{}", agent.new_name)).0,
            );
        }
    }

    let check_key = |key: u64| allowed_names.contains(&key);

    let map = HashMap::from_iter(
        prc_data
            .0
            .into_iter()
            .filter_map(|(key, val)| check_key(key.0).then_some((key.0, Arc::new(val)))),
    );

    let data = vtables::vtable_custom_data_mut::<_, FighterParamHolder>((*param_holder).vtable.0);
    data.original_clone = Some(std::mem::transmute(original_clone));
    data.original_lookup_param = Some(original_get_param);
    data.params = map;
    data.map_keys = remap_names;
}

pub fn install_param_hooks() {
    skyline::patching::Patch::in_text(0x371f934).nop().unwrap();
    skyline::install_hooks!(fighter_create_param_object, init_fighter_p_object);
}
