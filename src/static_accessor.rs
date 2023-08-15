use std::{
    marker::PhantomData,
    sync::atomic::{AtomicUsize, Ordering},
};

pub trait FromArrayAddress {
    const STRIDE: usize;
    unsafe fn from_address(addr: usize) -> Self;
}

pub struct StaticArrayAccessor<T: FromArrayAddress> {
    text: AtomicUsize,
    offset: usize,
    count: usize,
    _phantom: PhantomData<T>,
}

impl<T: FromArrayAddress> StaticArrayAccessor<T> {
    pub const fn new(offset: usize, count: usize) -> Self {
        Self {
            text: AtomicUsize::new(0),
            offset,
            count,
            _phantom: PhantomData,
        }
    }

    pub fn get(&self, index: usize) -> Option<T> {
        let mut text = self.text.load(Ordering::Acquire);
        if text == 0 {
            let base =
                unsafe { skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as usize };
            self.text.store(base, Ordering::Release);
            text = base;
        }

        (index < self.count)
            .then(|| unsafe { T::from_address(text + self.offset + T::STRIDE * index) })
    }

    pub fn iter(&self) -> StaticArrayIterator<T> {
        StaticArrayIterator {
            accessor: self,
            current: 0
        }
    }
}

pub struct StaticArrayIterator<'a, T: FromArrayAddress> {
    accessor: &'a StaticArrayAccessor<T>,
    current: usize,
}

impl<'a, T: FromArrayAddress> Iterator for StaticArrayIterator<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.accessor.get(self.current)?;
        self.current += 1;
        Some(item)
    }
}

impl FromArrayAddress for &'static str {
    const STRIDE: usize = 8;

    unsafe fn from_address(addr: usize) -> Self {
        let pointer = *(addr as *const *const u8);
        let mut len = 0;
        while *pointer.add(len) != 0 {
            len += 1;
        }

        std::str::from_utf8_unchecked(std::slice::from_raw_parts(pointer, len))
    }
}

macro_rules! primitives {
    ($($T:ty)*) => {
        $(
            impl FromArrayAddress for $T {
                const STRIDE: usize = std::mem::align_of::<$T>();

                unsafe fn from_address(addr: usize) -> Self {
                    *(addr as *const $T)
                }
            }
        )*
    }
}

primitives![i8 u8 i16 u16 i32 u32 i64 u64 f32 f64];
