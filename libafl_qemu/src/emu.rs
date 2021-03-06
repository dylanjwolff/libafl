//! Expose QEMU user `LibAFL` C api to Rust

use core::{
    convert::Into,
    ffi::c_void,
    mem::MaybeUninit,
    ptr::{addr_of, addr_of_mut, copy_nonoverlapping, null},
};
use libc::c_int;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use num_traits::Num;
use std::{slice::from_raw_parts, str::from_utf8_unchecked};
use strum_macros::EnumIter;

#[cfg(not(any(cpu_target = "x86_64", cpu_target = "aarch64")))]
/// `GuestAddr` is u32 for 32-bit targets
pub type GuestAddr = u32;

#[cfg(any(cpu_target = "x86_64", cpu_target = "aarch64"))]
/// `GuestAddr` is u64 for 64-bit targets
pub type GuestAddr = u64;

pub type GuestUsize = GuestAddr;

#[cfg(feature = "python")]
use pyo3::{prelude::*, PyIterProtocol};

pub const SKIP_EXEC_HOOK: u64 = u64::MAX;

#[derive(IntoPrimitive, TryFromPrimitive, Debug, Clone, Copy, EnumIter, PartialEq, Eq)]
#[repr(i32)]
pub enum MmapPerms {
    None = 0,
    Read = libc::PROT_READ,
    Write = libc::PROT_WRITE,
    Execute = libc::PROT_EXEC,
    ReadWrite = libc::PROT_READ | libc::PROT_WRITE,
    ReadExecute = libc::PROT_READ | libc::PROT_EXEC,
    WriteExecute = libc::PROT_WRITE | libc::PROT_EXEC,
    ReadWriteExecute = libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
}

impl MmapPerms {
    #[must_use]
    pub fn is_r(&self) -> bool {
        matches!(
            self,
            MmapPerms::Read
                | MmapPerms::ReadWrite
                | MmapPerms::ReadExecute
                | MmapPerms::ReadWriteExecute
        )
    }

    #[must_use]
    pub fn is_w(&self) -> bool {
        matches!(
            self,
            MmapPerms::Write
                | MmapPerms::ReadWrite
                | MmapPerms::WriteExecute
                | MmapPerms::ReadWriteExecute
        )
    }

    #[must_use]
    pub fn is_x(&self) -> bool {
        matches!(
            self,
            MmapPerms::Execute
                | MmapPerms::ReadExecute
                | MmapPerms::WriteExecute
                | MmapPerms::ReadWriteExecute
        )
    }
}

#[cfg(feature = "python")]
impl IntoPy<PyObject> for MmapPerms {
    fn into_py(self, py: Python) -> PyObject {
        let n: i32 = self.into();
        n.into_py(py)
    }
}

#[repr(C)]
#[cfg_attr(feature = "python", pyclass)]
#[cfg_attr(feature = "python", derive(FromPyObject))]
pub struct SyscallHookResult {
    pub retval: u64,
    pub skip_syscall: bool,
}

#[cfg(feature = "python")]
#[pymethods]
impl SyscallHookResult {
    #[new]
    #[must_use]
    pub fn new(value: Option<u64>) -> Self {
        value.map_or(
            Self {
                retval: 0,
                skip_syscall: false,
            },
            |v| Self {
                retval: v,
                skip_syscall: true,
            },
        )
    }
}

#[cfg(not(feature = "python"))]
impl SyscallHookResult {
    #[must_use]
    pub fn new(value: Option<u64>) -> Self {
        value.map_or(
            Self {
                retval: 0,
                skip_syscall: false,
            },
            |v| Self {
                retval: v,
                skip_syscall: true,
            },
        )
    }
}

#[repr(C)]
#[cfg_attr(feature = "python", pyclass(unsendable))]
pub struct MapInfo {
    start: GuestAddr,
    end: GuestAddr,
    offset: GuestAddr,
    path: *const u8,
    flags: i32,
    is_priv: i32,
}

#[cfg_attr(feature = "python", pymethods)]
impl MapInfo {
    #[must_use]
    pub fn start(&self) -> GuestAddr {
        self.start
    }

    #[must_use]
    pub fn end(&self) -> GuestAddr {
        self.end
    }

    #[must_use]
    pub fn offset(&self) -> GuestAddr {
        self.offset
    }

    #[must_use]
    pub fn path(&self) -> Option<&str> {
        if self.path.is_null() {
            None
        } else {
            unsafe {
                Some(from_utf8_unchecked(from_raw_parts(
                    self.path,
                    strlen(self.path),
                )))
            }
        }
    }

    #[must_use]
    pub fn flags(&self) -> MmapPerms {
        MmapPerms::try_from(self.flags).unwrap()
    }

    #[must_use]
    pub fn is_priv(&self) -> bool {
        self.is_priv != 0
    }
}

extern "C" {
    fn qemu_user_init(argc: i32, argv: *const *const u8, envp: *const *const u8) -> i32;

    fn libafl_qemu_write_reg(reg: i32, val: *const u8) -> i32;
    fn libafl_qemu_read_reg(reg: i32, val: *mut u8) -> i32;
    fn libafl_qemu_num_regs() -> i32;
    fn libafl_qemu_set_breakpoint(addr: u64) -> i32;
    fn libafl_qemu_remove_breakpoint(addr: u64) -> i32;
    fn libafl_flush_jit();
    fn libafl_qemu_set_hook(
        addr: GuestAddr,
        callback: extern "C" fn(GuestAddr, u64),
        data: u64,
        invalidate_block: i32,
    ) -> usize;
    // fn libafl_qemu_remove_hook(num: usize, invalidate_block: i32) -> i32;
    fn libafl_qemu_remove_hooks_at(addr: GuestAddr, invalidate_block: i32) -> usize;
    fn libafl_qemu_run() -> i32;
    fn libafl_load_addr() -> u64;
    fn libafl_get_brk() -> u64;
    fn libafl_set_brk(brk: u64) -> u64;

    fn strlen(s: *const u8) -> usize;

    /// abi_long target_mmap(abi_ulong start, abi_ulong len, int target_prot, int flags, int fd, abi_ulong offset)
    fn target_mmap(start: u64, len: u64, target_prot: i32, flags: i32, fd: i32, offset: u64)
        -> u64;

    /// int target_mprotect(abi_ulong start, abi_ulong len, int prot)
    fn target_mprotect(start: u64, len: u64, target_prot: i32) -> i32;

    /// int target_munmap(abi_ulong start, abi_ulong len)
    fn target_munmap(start: u64, len: u64) -> i32;

    fn read_self_maps() -> *const c_void;
    fn free_self_maps(map_info: *const c_void);

    fn libafl_maps_next(map_info: *const c_void, ret: *mut MapInfo) -> *const c_void;

    static exec_path: *const u8;
    static guest_base: usize;
    static mut mmap_next_start: GuestAddr;

    // void libafl_add_edge_hook(uint64_t (*gen)(target_ulong src, target_ulong dst), void (*exec)(uint64_t id));
    fn libafl_add_edge_hook(
        gen: Option<extern "C" fn(GuestAddr, GuestAddr, u64) -> u64>,
        exec: Option<extern "C" fn(u64, u64)>,
        data: u64,
    );

    // void libafl_add_block_hook(uint64_t (*gen)(target_ulong pc), void (*exec)(uint64_t id));
    fn libafl_add_block_hook(
        gen: Option<extern "C" fn(GuestAddr, u64) -> u64>,
        exec: Option<extern "C" fn(u64, u64)>,
        data: u64,
    );

    // void libafl_add_read_hook(uint64_t (*gen)(target_ulong pc, size_t size, uint64_t data),
    //                      void (*exec1)(uint64_t id, target_ulong addr, uint64_t data),
    //                      void (*exec2)(uint64_t id, target_ulong addr, uint64_t data),
    //                      void (*exec4)(uint64_t id, target_ulong addr, uint64_t data),
    //                      void (*exec8)(uint64_t id, target_ulong addr, uint64_t data),
    //                      void (*exec_n)(uint64_t id, target_ulong addr, size_t size, uint64_t data),
    //                      uint64_t data);
    fn libafl_add_read_hook(
        gen: Option<extern "C" fn(GuestAddr, usize, u64) -> u64>,
        exec1: Option<extern "C" fn(u64, GuestAddr, u64)>,
        exec2: Option<extern "C" fn(u64, GuestAddr, u64)>,
        exec4: Option<extern "C" fn(u64, GuestAddr, u64)>,
        exec8: Option<extern "C" fn(u64, GuestAddr, u64)>,
        exec_n: Option<extern "C" fn(u64, GuestAddr, usize, u64)>,
        data: u64,
    );

    // void libafl_add_write_hook(uint64_t (*gen)(target_ulong pc, size_t size, uint64_t data),
    //                      void (*exec1)(uint64_t id, target_ulong addr, uint64_t data),
    //                      void (*exec2)(uint64_t id, target_ulong addr, uint64_t data),
    //                      void (*exec4)(uint64_t id, target_ulong addr, uint64_t data),
    //                      void (*exec8)(uint64_t id, target_ulong addr, uint64_t data),
    //                      void (*exec_n)(uint64_t id, target_ulong addr, size_t size, uint64_t data),
    //                      uint64_t data);
    fn libafl_add_write_hook(
        gen: Option<extern "C" fn(GuestAddr, usize, u64) -> u64>,
        exec1: Option<extern "C" fn(u64, GuestAddr, u64)>,
        exec2: Option<extern "C" fn(u64, GuestAddr, u64)>,
        exec4: Option<extern "C" fn(u64, GuestAddr, u64)>,
        exec8: Option<extern "C" fn(u64, GuestAddr, u64)>,
        exec_n: Option<extern "C" fn(u64, GuestAddr, usize, u64)>,
        data: u64,
    );

    // void libafl_add_cmp_hook(uint64_t (*gen)(target_ulong pc, size_t size, uint64_t data),
    //                      void (*exec1)(uint64_t id, uint8_t v0, uint8_t v1, uint64_t data),
    //                      void (*exec2)(uint64_t id, uint16_t v0, uint16_t v1, uint64_t data),
    //                      void (*exec4)(uint64_t id, uint32_t v0, uint32_t v1, uint64_t data),
    //                      void (*exec8)(uint64_t id, uint64_t v0, uint64_t v1, uint64_t data),
    //                      uint64_t data);
    fn libafl_add_cmp_hook(
        gen: Option<extern "C" fn(GuestAddr, usize, u64) -> u64>,
        exec1: Option<extern "C" fn(u64, u8, u8, u64)>,
        exec2: Option<extern "C" fn(u64, u16, u16, u64)>,
        exec4: Option<extern "C" fn(u64, u32, u32, u64)>,
        exec8: Option<extern "C" fn(u64, u64, u64, u64)>,
        data: u64,
    );

    static mut libafl_on_thread_hook: unsafe extern "C" fn(u32);

    static mut libafl_pre_syscall_hook:
        unsafe extern "C" fn(i32, u64, u64, u64, u64, u64, u64, u64, u64) -> SyscallHookResult;
    static mut libafl_post_syscall_hook:
        unsafe extern "C" fn(u64, i32, u64, u64, u64, u64, u64, u64, u64, u64) -> u64;

    fn libafl_qemu_add_gdb_cmd(
        callback: extern "C" fn(*const u8, usize, *const ()) -> i32,
        data: *const (),
    );
    fn libafl_qemu_gdb_reply(buf: *const u8, len: usize);
}

#[cfg_attr(feature = "python", pyclass(unsendable))]
pub struct GuestMaps {
    orig_c_iter: *const c_void,
    c_iter: *const c_void,
}

// Consider a private new only for Emulator
impl GuestMaps {
    #[must_use]
    pub(crate) fn new() -> Self {
        unsafe {
            let maps = read_self_maps();
            Self {
                orig_c_iter: maps,
                c_iter: maps,
            }
        }
    }
}

impl Iterator for GuestMaps {
    type Item = MapInfo;

    #[allow(clippy::uninit_assumed_init)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.c_iter.is_null() {
            return None;
        }
        unsafe {
            let mut ret: MapInfo = MaybeUninit::uninit().assume_init();
            self.c_iter = libafl_maps_next(self.c_iter, addr_of_mut!(ret));
            if self.c_iter.is_null() {
                None
            } else {
                Some(ret)
            }
        }
    }
}

#[cfg(feature = "python")]
#[pyproto]
impl PyIterProtocol for GuestMaps {
    fn __iter__(slf: PyRef<Self>) -> PyRef<Self> {
        slf
    }
    fn __next__(mut slf: PyRefMut<Self>) -> Option<PyObject> {
        Python::with_gil(|py| slf.next().map(|x| x.into_py(py)))
    }
}

impl Drop for GuestMaps {
    fn drop(&mut self) {
        unsafe {
            free_self_maps(self.orig_c_iter);
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct FatPtr(*const c_void, *const c_void);

static mut GDB_COMMANDS: Vec<FatPtr> = vec![];

extern "C" fn gdb_cmd(buf: *const u8, len: usize, data: *const ()) -> i32 {
    unsafe {
        let closure = &mut *(data as *mut Box<dyn for<'r> FnMut(&Emulator, &'r str) -> bool>);
        let cmd = std::str::from_utf8_unchecked(std::slice::from_raw_parts(buf, len));
        let emu = Emulator::new_empty();
        if closure(&emu, cmd) {
            1
        } else {
            0
        }
    }
}

static mut EMULATOR_IS_INITIALIZED: bool = false;

#[derive(Clone, Debug)]
pub struct Emulator {
    _private: (),
}

#[allow(clippy::unused_self)]
impl Emulator {
    #[allow(clippy::must_use_candidate, clippy::similar_names)]
    pub fn new(args: &[String], env: &[(String, String)]) -> Emulator {
        unsafe {
            assert!(
                !EMULATOR_IS_INITIALIZED,
                "Only an instance of Emulator is permitted"
            );
        }
        assert!(!args.is_empty());
        let args: Vec<String> = args.iter().map(|x| x.clone() + "\0").collect();
        let argv: Vec<*const u8> = args.iter().map(|x| x.as_bytes().as_ptr()).collect();
        assert!(argv.len() < i32::MAX as usize);
        let env_strs: Vec<String> = env
            .iter()
            .map(|(k, v)| format!("{}={}\0", &k, &v))
            .collect();
        let mut envp: Vec<*const u8> = env_strs.iter().map(|x| x.as_bytes().as_ptr()).collect();
        envp.push(null());
        #[allow(clippy::cast_possible_wrap)]
        let argc = argv.len() as i32;
        unsafe {
            qemu_user_init(
                argc,
                argv.as_ptr() as *const *const u8,
                envp.as_ptr() as *const *const u8,
            );
            EMULATOR_IS_INITIALIZED = true;
        }
        Emulator { _private: () }
    }

    #[must_use]
    pub(crate) fn new_empty() -> Emulator {
        Emulator { _private: () }
    }

    /// This function gets the memory mappings from the emulator.
    #[must_use]
    pub fn mappings(&self) -> GuestMaps {
        GuestMaps::new()
    }

    /// Write a value to a guest address.
    ///
    /// # Safety
    /// This will write to a translated guest address (using `g2h`).
    /// It just adds `guest_base` and writes to that location, without checking the bounds.
    /// This may only be safely used for valid guest addresses!
    pub unsafe fn write_mem(&self, addr: GuestAddr, buf: &[u8]) {
        let host_addr = self.g2h(addr);
        copy_nonoverlapping(buf.as_ptr(), host_addr, buf.len());
    }

    /// Read a value from a guest address.
    ///
    /// # Safety
    /// This will read from a translated guest address (using `g2h`).
    /// It just adds `guest_base` and writes to that location, without checking the bounds.
    /// This may only be safely used for valid guest addresses!
    pub unsafe fn read_mem(&self, addr: GuestAddr, buf: &mut [u8]) {
        let host_addr = self.g2h(addr);
        copy_nonoverlapping(host_addr, buf.as_mut_ptr(), buf.len());
    }

    #[must_use]
    pub fn num_regs(&self) -> i32 {
        unsafe { libafl_qemu_num_regs() }
    }

    pub fn write_reg<R, T>(&self, reg: R, val: T) -> Result<(), String>
    where
        T: Num + PartialOrd + Copy,
        R: Into<i32>,
    {
        let reg = reg.into();
        let success = unsafe { libafl_qemu_write_reg(reg, addr_of!(val) as *const u8) };
        if success == 0 {
            Err(format!("Failed to write to register {}", reg))
        } else {
            Ok(())
        }
    }

    pub fn read_reg<R, T>(&self, reg: R) -> Result<T, String>
    where
        T: Num + PartialOrd + Copy,
        R: Into<i32>,
    {
        let reg = reg.into();
        let mut val = T::zero();
        let success = unsafe { libafl_qemu_read_reg(reg, addr_of_mut!(val) as *mut u8) };
        if success == 0 {
            Err(format!("Failed to read register {}", reg))
        } else {
            Ok(val)
        }
    }

    pub fn set_breakpoint(&self, addr: GuestAddr) {
        unsafe {
            libafl_qemu_set_breakpoint(addr.into());
        }
    }

    pub fn remove_breakpoint(&self, addr: GuestAddr) {
        unsafe {
            libafl_qemu_remove_breakpoint(addr.into());
        }
    }

    pub fn set_hook(
        &self,
        addr: GuestAddr,
        callback: extern "C" fn(GuestAddr, u64),
        data: u64,
        invalidate_block: bool,
    ) -> usize {
        unsafe { libafl_qemu_set_hook(addr.into(), callback, data, i32::from(invalidate_block)) }
    }

    #[must_use]
    pub fn remove_hook(&self, addr: GuestAddr, invalidate_block: bool) -> usize {
        unsafe { libafl_qemu_remove_hooks_at(addr.into(), i32::from(invalidate_block)) }
    }

    /// This function will run the emulator until the next breakpoint, or until finish.
    /// # Safety
    ///
    /// Should, in general, be safe to call.
    /// Of course, the emulated target is not contained securely and can corrupt state or interact with the operating system.
    pub unsafe fn run(&self) {
        libafl_qemu_run();
    }

    #[must_use]
    pub fn g2h<T>(&self, addr: GuestAddr) -> *mut T {
        unsafe { (addr as usize + guest_base) as *mut T }
    }

    #[must_use]
    pub fn h2g<T>(&self, addr: *const T) -> GuestAddr {
        unsafe { (addr as usize - guest_base) as GuestAddr }
    }

    #[must_use]
    pub fn binary_path<'a>(&self) -> &'a str {
        unsafe { from_utf8_unchecked(from_raw_parts(exec_path, strlen(exec_path))) }
    }

    #[must_use]
    pub fn load_addr(&self) -> GuestAddr {
        unsafe { libafl_load_addr() as GuestAddr }
    }

    #[must_use]
    pub fn get_brk(&self) -> GuestAddr {
        unsafe { libafl_get_brk() as GuestAddr }
    }

    pub fn set_brk(&self, brk: GuestAddr) {
        unsafe { libafl_set_brk(brk.into()) };
    }

    #[must_use]
    pub fn get_mmap_start(&self) -> GuestAddr {
        unsafe { mmap_next_start }
    }

    pub fn set_mmap_start(&self, start: GuestAddr) {
        unsafe { mmap_next_start = start };
    }

    fn mmap(
        &self,
        addr: GuestAddr,
        size: usize,
        perms: MmapPerms,
        flags: c_int,
    ) -> Result<u64, ()> {
        let res = unsafe { target_mmap(addr.into(), size as u64, perms.into(), flags, -1, 0) };
        if res == 0 {
            Err(())
        } else {
            Ok(res)
        }
    }

    pub fn map_private(
        &self,
        addr: GuestAddr,
        size: usize,
        perms: MmapPerms,
    ) -> Result<GuestAddr, String> {
        self.mmap(addr, size, perms, libc::MAP_PRIVATE | libc::MAP_ANONYMOUS)
            .map_err(|_| format!("Failed to map {}", addr))
            .map(|addr| addr as GuestAddr)
    }

    pub fn map_fixed(
        &self,
        addr: GuestAddr,
        size: usize,
        perms: MmapPerms,
    ) -> Result<GuestAddr, String> {
        self.mmap(
            addr,
            size,
            perms,
            libc::MAP_FIXED | libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
        )
        .map_err(|_| format!("Failed to map {}", addr))
        .map(|addr| addr as GuestAddr)
    }

    pub fn mprotect(&self, addr: GuestAddr, size: usize, perms: MmapPerms) -> Result<(), String> {
        let res = unsafe { target_mprotect(addr.into(), size as u64, perms.into()) };
        if res == 0 {
            Ok(())
        } else {
            Err(format!("Failed to mprotect {}", addr))
        }
    }

    pub fn unmap(&self, addr: GuestAddr, size: usize) -> Result<(), String> {
        if unsafe { target_munmap(addr.into(), size as u64) } == 0 {
            Ok(())
        } else {
            Err(format!("Failed to unmap {}", addr))
        }
    }

    pub fn flush_jit(&self) {
        unsafe {
            libafl_flush_jit();
        }
    }

    pub fn add_edge_hooks(
        &self,
        gen: Option<extern "C" fn(GuestAddr, GuestAddr, u64) -> u64>,
        exec: Option<extern "C" fn(u64, u64)>,
        data: u64,
    ) {
        unsafe { libafl_add_edge_hook(gen, exec, data) }
    }

    pub fn add_block_hooks(
        &self,
        gen: Option<extern "C" fn(GuestAddr, u64) -> u64>,
        exec: Option<extern "C" fn(u64, u64)>,
        data: u64,
    ) {
        unsafe { libafl_add_block_hook(gen, exec, data) }
    }

    pub fn add_read_hooks(
        &self,
        gen: Option<extern "C" fn(GuestAddr, usize, u64) -> u64>,
        exec1: Option<extern "C" fn(u64, GuestAddr, u64)>,
        exec2: Option<extern "C" fn(u64, GuestAddr, u64)>,
        exec4: Option<extern "C" fn(u64, GuestAddr, u64)>,
        exec8: Option<extern "C" fn(u64, GuestAddr, u64)>,
        exec_n: Option<extern "C" fn(u64, GuestAddr, usize, u64)>,
        data: u64,
    ) {
        unsafe { libafl_add_read_hook(gen, exec1, exec2, exec4, exec8, exec_n, data) }
    }

    pub fn add_write_hooks(
        &self,
        gen: Option<extern "C" fn(GuestAddr, usize, u64) -> u64>,
        exec1: Option<extern "C" fn(u64, GuestAddr, u64)>,
        exec2: Option<extern "C" fn(u64, GuestAddr, u64)>,
        exec4: Option<extern "C" fn(u64, GuestAddr, u64)>,
        exec8: Option<extern "C" fn(u64, GuestAddr, u64)>,
        exec_n: Option<extern "C" fn(u64, GuestAddr, usize, u64)>,
        data: u64,
    ) {
        unsafe { libafl_add_write_hook(gen, exec1, exec2, exec4, exec8, exec_n, data) }
    }

    pub fn add_cmp_hooks(
        &self,
        gen: Option<extern "C" fn(GuestAddr, usize, u64) -> u64>,
        exec1: Option<extern "C" fn(u64, u8, u8, u64)>,
        exec2: Option<extern "C" fn(u64, u16, u16, u64)>,
        exec4: Option<extern "C" fn(u64, u32, u32, u64)>,
        exec8: Option<extern "C" fn(u64, u64, u64, u64)>,
        data: u64,
    ) {
        unsafe { libafl_add_cmp_hook(gen, exec1, exec2, exec4, exec8, data) }
    }

    pub fn set_on_thread_hook(&self, hook: extern "C" fn(tid: u32)) {
        unsafe {
            libafl_on_thread_hook = hook;
        }
    }

    pub fn set_pre_syscall_hook(
        &self,
        hook: extern "C" fn(i32, u64, u64, u64, u64, u64, u64, u64, u64) -> SyscallHookResult,
    ) {
        unsafe {
            libafl_pre_syscall_hook = hook;
        }
    }

    pub fn set_post_syscall_hook(
        &self,
        hook: extern "C" fn(u64, i32, u64, u64, u64, u64, u64, u64, u64, u64) -> u64,
    ) {
        unsafe {
            libafl_post_syscall_hook = hook;
        }
    }

    pub fn add_gdb_cmd(&self, callback: Box<dyn FnMut(&Self, &str) -> bool>) {
        unsafe {
            GDB_COMMANDS.push(core::mem::transmute(callback));
            libafl_qemu_add_gdb_cmd(
                gdb_cmd,
                GDB_COMMANDS.last().unwrap() as *const _ as *const (),
            );
        }
    }

    pub fn gdb_reply(&self, output: &str) {
        unsafe { libafl_qemu_gdb_reply(output.as_bytes().as_ptr(), output.len()) };
    }
}

#[cfg(feature = "python")]
pub mod pybind {
    use super::{GuestAddr, GuestUsize, MmapPerms, SyscallHookResult};
    use pyo3::exceptions::PyValueError;
    use pyo3::{prelude::*, types::PyInt};
    use std::convert::TryFrom;

    static mut PY_SYSCALL_HOOK: Option<PyObject> = None;
    static mut PY_GENERIC_HOOKS: Vec<(GuestAddr, PyObject)> = vec![];

    extern "C" fn py_syscall_hook_wrapper(
        sys_num: i32,
        a0: u64,
        a1: u64,
        a2: u64,
        a3: u64,
        a4: u64,
        a5: u64,
        a6: u64,
        a7: u64,
    ) -> SyscallHookResult {
        unsafe { PY_SYSCALL_HOOK.as_ref() }.map_or_else(
            || SyscallHookResult::new(None),
            |obj| {
                let args = (sys_num, a0, a1, a2, a3, a4, a5, a6, a7);
                Python::with_gil(|py| {
                    let ret = obj.call1(py, args).expect("Error in the syscall hook");
                    let any = ret.as_ref(py);
                    if any.is_none() {
                        SyscallHookResult::new(None)
                    } else {
                        let a: Result<&PyInt, _> = any.cast_as();
                        if let Ok(i) = a {
                            SyscallHookResult::new(Some(
                                i.extract().expect("Invalid syscall hook return value"),
                            ))
                        } else {
                            SyscallHookResult::extract(any)
                                .expect("The syscall hook must return a SyscallHookResult")
                        }
                    }
                })
            },
        )
    }

    extern "C" fn py_generic_hook_wrapper(_pc: GuestAddr, idx: u64) {
        let obj = unsafe { &PY_GENERIC_HOOKS[idx as usize].1 };
        Python::with_gil(|py| {
            obj.call0(py).expect("Error in the hook");
        });
    }

    #[pyclass(unsendable)]
    pub struct Emulator {
        pub emu: super::Emulator,
    }

    #[pymethods]
    impl Emulator {
        #[allow(clippy::needless_pass_by_value)]
        #[new]
        fn new(args: Vec<String>, env: Vec<(String, String)>) -> Emulator {
            Emulator {
                emu: super::Emulator::new(&args, &env),
            }
        }

        fn write_mem(&self, addr: GuestAddr, buf: &[u8]) {
            unsafe {
                self.emu.write_mem(addr, buf);
            }
        }

        fn read_mem(&self, addr: GuestAddr, size: usize) -> Vec<u8> {
            let mut buf = vec![0; size];
            unsafe {
                self.emu.read_mem(addr, &mut buf);
            }
            buf
        }

        fn num_regs(&self) -> i32 {
            self.emu.num_regs()
        }

        fn write_reg(&self, reg: i32, val: GuestUsize) -> PyResult<()> {
            self.emu.write_reg(reg, val).map_err(PyValueError::new_err)
        }

        fn read_reg(&self, reg: i32) -> PyResult<GuestUsize> {
            self.emu.read_reg(reg).map_err(PyValueError::new_err)
        }

        fn set_breakpoint(&self, addr: GuestAddr) {
            self.emu.set_breakpoint(addr);
        }

        fn remove_breakpoint(&self, addr: GuestAddr) {
            self.emu.remove_breakpoint(addr);
        }

        fn run(&self) {
            unsafe {
                self.emu.run();
            }
        }

        fn g2h(&self, addr: GuestAddr) -> u64 {
            self.emu.g2h::<*const u8>(addr) as u64
        }

        fn h2g(&self, addr: u64) -> GuestAddr {
            self.emu.h2g(addr as *const u8)
        }

        fn binary_path(&self) -> String {
            self.emu.binary_path().to_owned()
        }

        fn load_addr(&self) -> GuestAddr {
            self.emu.load_addr()
        }

        fn flush_jit(&self) {
            self.emu.flush_jit();
        }

        fn map_private(&self, addr: GuestAddr, size: usize, perms: i32) -> PyResult<GuestAddr> {
            if let Ok(p) = MmapPerms::try_from(perms) {
                self.emu
                    .map_private(addr, size, p)
                    .map_err(PyValueError::new_err)
            } else {
                Err(PyValueError::new_err("Invalid perms"))
            }
        }

        fn map_fixed(&self, addr: GuestAddr, size: usize, perms: i32) -> PyResult<GuestAddr> {
            if let Ok(p) = MmapPerms::try_from(perms) {
                self.emu
                    .map_fixed(addr, size, p)
                    .map_err(PyValueError::new_err)
            } else {
                Err(PyValueError::new_err("Invalid perms"))
            }
        }

        fn mprotect(&self, addr: GuestAddr, size: usize, perms: i32) -> PyResult<()> {
            if let Ok(p) = MmapPerms::try_from(perms) {
                self.emu
                    .mprotect(addr, size, p)
                    .map_err(PyValueError::new_err)
            } else {
                Err(PyValueError::new_err("Invalid perms"))
            }
        }

        fn unmap(&self, addr: GuestAddr, size: usize) -> PyResult<()> {
            self.emu.unmap(addr, size).map_err(PyValueError::new_err)
        }

        fn set_syscall_hook(&self, hook: PyObject) {
            unsafe {
                PY_SYSCALL_HOOK = Some(hook);
            }
            self.emu.set_pre_syscall_hook(py_syscall_hook_wrapper);
        }

        fn set_hook(&self, addr: GuestAddr, hook: PyObject) {
            unsafe {
                let idx = PY_GENERIC_HOOKS.len();
                PY_GENERIC_HOOKS.push((addr, hook));
                self.emu
                    .set_hook(addr, py_generic_hook_wrapper, idx as u64, true);
            }
        }

        fn remove_hook(&self, addr: GuestAddr) -> usize {
            unsafe {
                PY_GENERIC_HOOKS.retain(|(a, _)| *a != addr);
            }
            self.emu.remove_hook(addr, true)
        }
    }
}
