//! Drop-in replacements for the standard [`Mutex`](std::sync::Mutex) and [`RwLock`](std::sync::RwLock)
//!
//! When using the standard library's locking primitives, they don't function as one would imagine.
//! Instead, when you go to lock a [`Mutex`](std::sync::Mutex) or a [`RwLock`](std::sync::RwLock)
//! while it is exclusively locked on another thread, the locking operation never finishes
//! and deadlocks **both** threads.
//!
//! Because of this, drop-in replacements are required. These *should* function identically to the
//! standard library versions, including RAII and `Send`/`Sync` impls.

#![feature(negative_impls)]
use std::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
};

extern "C" {
    fn nnosFinalizeMutex(mutex: &mut RawMutex);
    fn nnosLockMutex(mutex: &RawMutex);
    fn nnosUnlockMutex(mutex: &RawMutex);
    fn nnosTryLockMutex(mutex: &RawMutex) -> bool;

    #[link_name = "_ZN2nn2os24FinalizeReaderWriterLockEPNS0_20ReaderWriterLockTypeE"]
    fn nnosFinalizeReaderWriterLock(lock: &mut RawRwLock);

    #[link_name = "_ZN2nn2os15AcquireReadLockEPNS0_20ReaderWriterLockTypeE"]
    fn nnosAcquireReadLock(lock: &RawRwLock);

    #[link_name = "_ZN2nn2os16AcquireWriteLockEPNS0_20ReaderWriterLockTypeE"]
    fn nnosAcquireWriteLock(lock: &RawRwLock);

    #[link_name = "_ZN2nn2os18TryAcquireReadLockEPNS0_20ReaderWriterLockTypeE"]
    fn nnosTryAcquireReadLock(lock: &RawRwLock) -> bool;

    #[link_name = "_ZN2nn2os19TryAcquireWriteLockEPNS0_20ReaderWriterLockTypeE"]
    fn nnosTryAcquireWriteLock(lock: &RawRwLock) -> bool;

    #[link_name = "_ZN2nn2os15ReleaseReadLockEPNS0_20ReaderWriterLockTypeE"]
    fn nnosReleaseReadLock(lock: &RawRwLock);

    #[link_name = "_ZN2nn2os16ReleaseWriteLockEPNS0_20ReaderWriterLockTypeE"]
    fn nnosReleaseWriteLock(lock: &RawRwLock);
}

#[repr(C)]
struct RawMutex {
    is_initialized: bool,
    recursive: bool,
    unused_: i32,
    recursion_count: u32,
    padding_: u32,
    internal: [u8; 0x10],
}

impl RawMutex {
    const fn new() -> Self {
        Self {
            is_initialized: true,
            recursive: false,
            unused_: 0,
            recursion_count: 0,
            padding_: 0,
            internal: [0u8; 0x10],
        }
    }

    fn lock(&self) {
        unsafe {
            nnosLockMutex(self);
        }
    }

    fn unlock(&self) {
        unsafe {
            nnosUnlockMutex(self);
        }
    }

    fn try_lock(&self) -> bool {
        unsafe { nnosTryLockMutex(self) }
    }
}

impl Drop for RawMutex {
    fn drop(&mut self) {
        unsafe {
            nnosFinalizeMutex(self);
        }
    }
}

#[repr(C)]
struct RawRwLock {
    is_initialized: bool,
    internal: [u32; 11],
}

impl RawRwLock {
    const fn new() -> Self {
        Self {
            is_initialized: true,
            internal: [0u32; 11],
        }
    }

    fn read(&self) {
        unsafe {
            nnosAcquireReadLock(self);
        }
    }

    fn write(&self) {
        unsafe {
            nnosAcquireWriteLock(self);
        }
    }

    fn try_read(&self) -> bool {
        unsafe { nnosTryAcquireReadLock(self) }
    }

    fn try_write(&self) -> bool {
        unsafe { nnosTryAcquireWriteLock(self) }
    }

    fn release_read(&self) {
        unsafe {
            nnosReleaseReadLock(self);
        }
    }

    fn release_write(&self) {
        unsafe {
            nnosReleaseWriteLock(self);
        }
    }
}

impl Drop for RawRwLock {
    fn drop(&mut self) {
        unsafe {
            nnosFinalizeReaderWriterLock(self);
        }
    }
}

/// Locking primitive for mutually-exclusive access
///
/// Using this will prevent multiple threads from accessing this data at the same time. You can
/// acquire a mutable reference to the data via the `lock` method, and the lock is released when the
/// reference is dropped.
///
/// If you are looking to give multiple threads read-only access, see [`RwLock`] instead.
#[repr(C)]
pub struct Mutex<T> {
    raw: RawMutex,
    data: UnsafeCell<T>,
}

unsafe impl<T> Send for Mutex<T> {}
unsafe impl<T> Sync for Mutex<T> {}

impl<T> Mutex<T> {
    /// Constructs a new mutex with the provided data
    pub const fn new(data: T) -> Self {
        Self {
            raw: RawMutex::new(),
            data: UnsafeCell::new(data),
        }
    }

    /// Gets a mutable reference to the underlying data, which we can guarantee is unique
    /// at compile time due to the mutable reference to the mutex
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }

    /// Locks the mutex and acquires an exclusive reference, blocking the thread until
    /// it can complete.
    pub fn lock(&self) -> MutexGuard<'_, T> {
        self.raw.lock();
        MutexGuard { mutex: self }
    }

    /// Attempts to lock the mutex for an exclusive reference. Will not block the thread if it
    /// cannot be acquired and will instead return [`None`]
    pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        self.raw.try_lock().then(|| MutexGuard { mutex: self })
    }

    /// Consumes this mutex and returns the inner data
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

/// RAII guard for a mutex lock, will unlock the mutex upon dropping
pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
}

impl<'a, T> !Send for MutexGuard<'a, T> {}

impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<'a, T> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.mutex.raw.unlock();
    }
}

/// Locking primitive for reader-writer shared access
///
/// Using this will prevent multiple threads from mutably accessing this data at the same time.
/// You can acquire a read-only reference to the data via the `read` method, a mutable reference
/// to the data via the `wwrite` method, and the lock is released when the reference is dropped.
#[repr(C)]
pub struct RwLock<T> {
    raw: RawRwLock,
    data: UnsafeCell<T>,
}

unsafe impl<T> Send for RwLock<T> {}
unsafe impl<T> Sync for RwLock<T> {}

impl<T> RwLock<T> {
    /// Constructs a new reader-writer lock with the provided data
    pub const fn new(data: T) -> Self {
        Self {
            raw: RawRwLock::new(),
            data: UnsafeCell::new(data),
        }
    }

    /// Gets a mutable reference to the underlying data, which we can guarantee is unique
    /// at compile time due to the mutable reference to the lock
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }

    /// Acquires a read-only reference to the underlying data. This will block on the current thread
    /// until there are no active writers.
    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        self.raw.read();
        RwLockReadGuard { inner: self }
    }

    /// Acquires a mutable reference to the underlying data. This will block on the current thread
    /// until there are no active readers. If reader access is requested while this writer is either
    /// active or waiting, they will wait until write access is relinquished.
    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        self.raw.write();
        RwLockWriteGuard { inner: self }
    }

    /// Attempts to acquire a read-only reference to the data. This will not block on the current
    /// thread, and if it cannot acquire a reference [`None`] will be returned.
    pub fn try_read(&self) -> Option<RwLockReadGuard<'_, T>> {
        self.raw.try_read().then(|| RwLockReadGuard { inner: self })
    }

    /// Attempts to acquire an exclusive reference to the data. This will not block on the current
    /// thread, and if it cannot acquire a reference [`None`] will be returned.
    pub fn try_write(&self) -> Option<RwLockWriteGuard<'_, T>> {
        self.raw
            .try_write()
            .then(|| RwLockWriteGuard { inner: self })
    }

    /// Consumes the lock and returns the underlying data
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

/// RAII guard for a reader lock, will release read access upon drop
pub struct RwLockReadGuard<'a, T> {
    inner: &'a RwLock<T>,
}

impl<'a, T> !Send for RwLockReadGuard<'a, T> {}

impl<'a, T> Deref for RwLockReadGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.inner.data.get() }
    }
}

impl<'a, T> Drop for RwLockReadGuard<'a, T> {
    fn drop(&mut self) {
        self.inner.raw.release_read();
    }
}

/// RAII guard for a writer lock, will release write access upon drop
pub struct RwLockWriteGuard<'a, T> {
    inner: &'a RwLock<T>,
}

impl<'a, T> !Send for RwLockWriteGuard<'a, T> {}

impl<'a, T> Deref for RwLockWriteGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.inner.data.get() }
    }
}

impl<'a, T> DerefMut for RwLockWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.inner.data.get() }
    }
}

impl<'a, T> Drop for RwLockWriteGuard<'a, T> {
    fn drop(&mut self) {
        self.inner.raw.release_write();
    }
}
