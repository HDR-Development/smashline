use std::{
    ffi::{c_char, CStr, CString},
    ops::{Deref, DerefMut},
    sync::OnceLock,
};

pub trait PushArray<T> {
    fn push(&mut self, item: T);
}

pub struct DynamicArrayAccessor<T> {
    data: OnceLock<Vec<T>>,
    offset: usize,
    count: usize,
}

impl<T: Clone> DynamicArrayAccessor<T> {
    pub const fn new(offset: usize, count: usize) -> Self {
        Self {
            data: OnceLock::new(),
            offset,
            count,
        }
    }

    fn init_data(&self) -> Vec<T> {
        unsafe {
            let text = skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as *const u8;
            let array = text.add(self.offset) as *const T;
            let s: &[T]= std::slice::from_raw_parts(array, self.count);
            s.to_vec()
        }
    }
}

impl<T: Clone> Deref for DynamicArrayAccessor<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        self.data.get_or_init(|| self.init_data())
    }
}

impl<T: Clone> DerefMut for DynamicArrayAccessor<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data.get_or_init(|| self.init_data());
        self.data.get_mut().unwrap()
    }
}

// 2+ weapons with the same name use the same pointer to that string.
// So, if someone is adding a new weapon with the same name,
// it'll also use the same pointer that the previous weapons use.
impl PushArray<*const c_char> for DynamicArrayAccessor<*const c_char> {
    fn push(&mut self, item: *const c_char) {
        let data: &mut Vec<*const c_char> = self;
        
        let new_item = unsafe {
            data.iter()
                .find(|&&x| CStr::from_ptr(x) == CStr::from_ptr(item))
                .inspect(|x| {
                    let _ = CString::from_raw(item as *mut i8);
                })
                .unwrap_or(&item)
        };

        data.push(*new_item);
    }
}

macro_rules! primitives {
    ($($T:ty)*) => {
        $(
            impl PushArray<$T> for DynamicArrayAccessor<$T> {
                fn push(&mut self, item: $T) {
                    let data: &mut Vec<$T> = self;
                    data.push(item);
                }
            }
        )*
    }
}

primitives![i8 u8 i16 u16 i32 u32 i64 u64 f32 f64];
