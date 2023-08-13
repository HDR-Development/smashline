use std::{ops::Range, str::Utf8Error};

use object::{
    elf::{R_AARCH64_ABS32, R_AARCH64_ABS64, R_AARCH64_GLOB_DAT, R_AARCH64_JUMP_SLOT, SHN_COMMON},
    *,
};

pub type Rel = elf::Rel64<LittleEndian>;
pub type Rela = elf::Rela64<LittleEndian>;
pub type Dyn = elf::Dyn64<LittleEndian>;
pub type Sym = elf::Sym64<LittleEndian>;
pub use object::LittleEndian;

pub mod nx;

use thiserror::Error;

#[repr(C)]
pub struct ModuleObject {
    next: *mut ModuleObject,
    prev: *mut ModuleObject,

    rela_plt: *const Rela,
    rela: *const Rela,
    module_base: *const u8,
    dynamic: *const Dyn,
    is_rela: bool,
    rela_plt_size: usize,
    dt_init: *const u8,
    dt_fini: *const u8,
    hash_bucket: *const u32,
    hash_chain: *const u32,
    dynstr: *const u8,
    dynsym: *const Sym,
    dynstr_size: usize,
    got: *const *const u8,
    rela_dyn_size: usize,
    rel_dyn_size: usize,
    rel_count: usize,
    rela_count: usize,
    hash_nchain_value: usize,
    hash_nbucket_value: usize,
    got_stub_ptr: *const u8,
    soname_idx: usize,
    nro_size: usize,
    cannot_revert_symbols: bool,
}

unsafe impl Send for ModuleObject {}
unsafe impl Sync for ModuleObject {}

impl ModuleObject {
    fn hash_bucket(&self) -> &'static [u32] {
        unsafe { std::slice::from_raw_parts(self.hash_bucket, self.hash_nbucket_value as usize) }
    }

    fn hash_chain(&self) -> &'static [u32] {
        unsafe { std::slice::from_raw_parts(self.hash_chain, self.hash_nchain_value as usize) }
    }

    pub fn get_dynstr(&self, offset: usize) -> &str {
        unsafe {
            let ptr = self.dynstr.add(offset);
            let mut len = 0;
            while *ptr.add(len) != 0 {
                len += 1;
            }

            std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len))
        }
    }

    pub fn next(&mut self) -> &'static mut ModuleObject {
        unsafe { &mut *self.next }
    }

    pub fn prev(&mut self) -> &'static mut ModuleObject {
        unsafe { &mut *self.prev }
    }

    pub fn get_symbol_by_name(&self, name: &str) -> Option<Sym> {
        let hash = zelf::hash::hash(name.as_bytes());

        let bucket = self.hash_bucket();
        let chain = self.hash_chain();

        let mut hash_index = bucket[(hash as usize) % bucket.len()];
        while hash_index != 0 {
            let sym = unsafe { *self.dynsym.add(hash_index as usize) };

            let is_common =
                sym.st_shndx.get(LittleEndian) == 0 || sym.st_shndx.get(LittleEndian) == SHN_COMMON;

            if !is_common && self.get_dynstr(sym.st_name.get(LittleEndian) as usize) == name {
                return Some(sym);
            }

            hash_index = chain[hash_index as usize];
        }

        None
    }

    pub fn get_symbol_ptr_by_name(&self, name: &str) -> Option<*const ()> {
        let sym = self.get_symbol_by_name(name)?;

        unsafe {
            Some(
                self.module_base
                    .add(sym.st_value.get(LittleEndian) as usize) as *const (),
            )
        }
    }

    fn try_patch_absolute_reloc_impl(
        &mut self,
        replacement: *const (),
        name: &str,
        entry: Rel,
    ) -> Option<*const ()> {
        let ty = entry.r_type(LittleEndian);
        let sym = entry.r_sym(LittleEndian);

        if ty != R_AARCH64_ABS32 && ty != R_AARCH64_ABS64 && ty != R_AARCH64_GLOB_DAT {
            return None;
        }

        let sym = unsafe { *self.dynsym.add(sym as usize) };
        let symbol_name = self.get_dynstr(sym.st_name.get(LittleEndian) as usize);
        if symbol_name != name {
            return None;
        }

        unsafe {
            let prev = *(self
                .module_base
                .add(entry.r_offset.get(LittleEndian) as usize)
                as *const *const ());
            *(self
                .module_base
                .add(entry.r_offset.get(LittleEndian) as usize) as *mut *const ()) = replacement;
            Some(prev)
        }
    }

    fn try_patch_reloc_impl(
        &mut self,
        replacement: *const (),
        name: &str,
        entry: Rel,
    ) -> Option<*const ()> {
        let ty = entry.r_type(LittleEndian);
        let sym = entry.r_sym(LittleEndian);

        if ty != R_AARCH64_JUMP_SLOT {
            return None;
        }

        let sym = unsafe { *self.dynsym.add(sym as usize) };
        let symbol_name = self.get_dynstr(sym.st_name.get(LittleEndian) as usize);
        if symbol_name != name {
            return None;
        }

        unsafe {
            let prev = *(self
                .module_base
                .add(entry.r_offset.get(LittleEndian) as usize)
                as *const *const ());
            *(self
                .module_base
                .add(entry.r_offset.get(LittleEndian) as usize) as *mut *const ()) = replacement;
            Some(prev)
        }
    }

    pub fn try_patch_absolute_reloc(
        &mut self,
        replacement: *const (),
        name: &str,
    ) -> Option<*const ()> {
        for index in self.rela_count..(self.rela_dyn_size / std::mem::size_of::<Rela>()) {
            let entry = unsafe { *self.rela.add(index) };
            if let Some(prev) = self.try_patch_absolute_reloc_impl(
                replacement,
                name,
                Rel {
                    r_info: entry.r_info,
                    r_offset: entry.r_offset,
                },
            ) {
                return Some(prev);
            }
        }

        None
    }

    pub fn try_patch_reloc(&mut self, replacement: *const (), name: &str) -> Option<*const ()> {
        for index in 0..(self.rela_plt_size / std::mem::size_of::<Rela>()) {
            let entry = unsafe { *self.rela_plt.add(index) };
            if let Some(prev) = self.try_patch_reloc_impl(
                replacement,
                name,
                Rel {
                    r_info: entry.r_info,
                    r_offset: entry.r_offset,
                },
            ) {
                return Some(prev);
            }
        }

        None
    }

    pub fn get_module_name(&self) -> Option<&'static str> {
        let info = nx::query_memory(self.module_base as u64);

        let ro_info = nx::query_memory(info.addr + info.size);

        unsafe {
            let rw_data_offset = *(ro_info.addr as *const u32);
            if rw_data_offset as u64 + info.addr == ro_info.addr + ro_info.size {
                return None;
            }

            if rw_data_offset != 0 {
                return None;
            }

            let path_length = *(ro_info.addr as *const i32).add(1);
            if path_length <= 0 {
                return None;
            }

            let name = std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                (ro_info.addr + 8) as *const u8,
                path_length as usize,
            ));

            let split = name.split('\\').last().unwrap();
            let name = split.split('/').last().unwrap();

            Some(name)
        }
    }

    pub fn get_address_range(&self, section: Section) -> Range<u64> {
        match section {
            Section::Text => {
                let info = nx::query_memory(self.module_base as u64);
                info.addr..info.addr + info.size
            }
            Section::RoData => {
                let text = self.get_address_range(Section::Text);
                let info = nx::query_memory(text.end);

                info.addr..info.addr + info.size
            }
            Section::Data => {
                let ro_data = self.get_address_range(Section::RoData);
                let info = nx::query_memory(ro_data.end);
                info.addr..info.addr + info.size
            }
        }
    }

    pub fn get_symbol_range_for_address(&self, address: u64) -> Option<Range<u64>> {
        let symbols =
            unsafe { std::slice::from_raw_parts(self.dynsym, self.hash_nchain_value as usize) };

        for symbol in symbols {
            let shndx = symbol.st_shndx.get(LittleEndian);
            if shndx == 0 || (shndx & 0xFF00) == 0xFF00 {
                continue;
            }

            if symbol.st_info & 0xF != 2 {
                continue;
            }

            let function_start = unsafe {
                self.module_base
                    .add(symbol.st_value.get(LittleEndian) as usize) as u64
            };

            let function_end = function_start + symbol.st_size.get(LittleEndian);

            if function_start <= address && address < function_end {
                return Some(function_start..function_end);
            }
        }

        None
    }
}

#[repr(C)]
pub struct ModuleObjectList {
    front: *mut ModuleObject,
    back: *mut ModuleObject,
}

impl ModuleObjectList {
    pub fn iter(&self) -> ModuleObjectListIterator {
        ModuleObjectListIterator {
            end: self as *const ModuleObjectList as *mut ModuleObject,
            current: self.front,
        }
    }
}

pub struct ModuleObjectListIterator {
    end: *mut ModuleObject,
    current: *mut ModuleObject,
}

impl Iterator for ModuleObjectListIterator {
    type Item = &'static mut ModuleObject;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current == self.end {
            None
        } else {
            let next = unsafe { (*self.current).next };
            let current = unsafe { &mut *self.current };
            self.current = next;
            Some(current)
        }
    }
}

extern "C" {
    #[link_name = "_ZN2nn2ro6detail15g_pAutoLoadListE"]
    pub(crate) static AUTO_LOAD_LIST: &'static mut ModuleObjectList;

    #[link_name = "_ZN2nn2ro6detail17g_pManualLoadListE"]
    pub(crate) static MANUAL_LOAD_LIST: &'static mut ModuleObjectList;
}

pub fn auto_load_list() -> ModuleObjectListIterator {
    unsafe { AUTO_LOAD_LIST.iter() }
}

pub fn manual_load_list() -> ModuleObjectListIterator {
    unsafe { MANUAL_LOAD_LIST.iter() }
}

#[derive(Error, Debug)]
pub enum RtldError {
    #[error(".rodata section is not read only")]
    RoNotReadOnly,

    #[error("Module is in a deprecated format")]
    DeprecatedFormat,

    #[error("Module name has invalid length ({0})")]
    InvalidNameLength(i32),

    #[error("{0}")]
    InvalidUtf8(#[from] Utf8Error),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Section {
    Text,
    RoData,
    Data,
}

pub fn find_module_for_address(
    address: u64,
    section: Section,
) -> Option<&'static mut ModuleObject> {
    unsafe {
        AUTO_LOAD_LIST
            .iter()
            .chain(MANUAL_LOAD_LIST.iter())
            .find(|object| {
                let range = object.get_address_range(section);
                range.contains(&address)
            })
    }
}

pub fn find_module_for_address_no_section(address: u64) -> Option<&'static mut ModuleObject> {
    unsafe {
        AUTO_LOAD_LIST
            .iter()
            .chain(MANUAL_LOAD_LIST.iter())
            .find(|object| {
                object.get_address_range(Section::Text).contains(&address)
                    || object.get_address_range(Section::Data).contains(&address)
                    || object.get_address_range(Section::RoData).contains(&address)
            })
    }
}

pub fn find_module_by_name(name: &str) -> Option<&'static mut ModuleObject> {
    let mut objects = unsafe { AUTO_LOAD_LIST.iter().chain(MANUAL_LOAD_LIST.iter()) };

    objects.find(|object| object.get_module_name().unwrap_or("__invalid_name") == name)
}

pub fn is_valid_pointer_for_section(address: u64, section: Section) -> bool {
    find_module_for_address(address, section).is_some()
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MemoryState {
    NonDereferencable,
    DereferencableOutsideModule,
    DereferencableInModule,
}

pub fn get_memory_state(address: u64) -> MemoryState {
    let state = nx::query_memory(address);
    if state.perm & 0x1 == 0 {
        return MemoryState::NonDereferencable;
    }

    if find_module_for_address_no_section(address).is_some() {
        MemoryState::DereferencableInModule
    } else {
        MemoryState::DereferencableOutsideModule
    }
}
