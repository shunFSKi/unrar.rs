#![no_std]

#[cfg(feature = "std")]
extern crate std;

use libc::wchar_t;

#[cfg(feature = "std")]
use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_void};

#[cfg(not(feature = "std"))]
use libc::{c_char, c_int, c_uchar, c_uint, c_void};

// ----------------- ENV SPECIFIC ----------------- //

#[cfg(windows)]
mod env {
    pub use {
        winapi::shared::minwindef::{INT, LPARAM, UCHAR, UINT},
        winapi::shared::ntdef::LONG,
    };
}

#[cfg(not(windows))]
mod env {
    use super::*;

    #[cfg(feature = "std")]
    use std::os::raw::c_long;

    #[cfg(not(feature = "std"))]
    use libc::c_long;

    pub type LPARAM = c_long;
    pub type LONG = c_long;
    pub type UINT = c_uint;
    pub type INT = c_int;
    pub type UCHAR = c_uchar;
}

pub use self::env::INT;
pub use self::env::LONG;
pub use self::env::LPARAM;
pub use self::env::UCHAR;
pub use self::env::UINT;

pub type WCHAR = wchar_t;

// ----------------- CONSTANTS ----------------- //

pub const ERAR_SUCCESS: c_int = 0;
pub const ERAR_END_ARCHIVE: c_int = 10;
pub const ERAR_NO_MEMORY: c_int = 11;
pub const ERAR_BAD_DATA: c_int = 12;
pub const ERAR_BAD_ARCHIVE: c_int = 13;
pub const ERAR_UNKNOWN_FORMAT: c_int = 14;
pub const ERAR_EOPEN: c_int = 15;
pub const ERAR_ECREATE: c_int = 16;
pub const ERAR_ECLOSE: c_int = 17;
pub const ERAR_EREAD: c_int = 18;
pub const ERAR_EWRITE: c_int = 19;
pub const ERAR_SMALL_BUF: c_int = 20;
pub const ERAR_UNKNOWN: c_int = 21;
pub const ERAR_MISSING_PASSWORD: c_int = 22;
pub const ERAR_EREFERENCE: c_int = 23;
pub const ERAR_BAD_PASSWORD: c_int = 24;

pub const RAR_OM_LIST: c_uint = 0;
pub const RAR_OM_EXTRACT: c_uint = 1;
pub const RAR_OM_LIST_INCSPLIT: c_uint = 2;

pub const RAR_SKIP: c_int = 0;
pub const RAR_TEST: c_int = 1;
pub const RAR_EXTRACT: c_int = 2;

pub const RAR_VOL_ASK: LPARAM = 0;
pub const RAR_VOL_NOTIFY: LPARAM = 1;

pub const RAR_HASH_NONE: c_uint = 0;
pub const RAR_HASH_CRC32: c_uint = 1;
pub const RAR_HASH_BLAKE2: c_uint = 2;

pub const RHDF_SPLITBEFORE: c_uint = 1 << 0; // 1, 0x1
pub const RHDF_SPLITAFTER: c_uint = 1 << 1; // 2, 0x2
pub const RHDF_ENCRYPTED: c_uint = 1 << 2; // 4, 0x4
                                           // pub const RHDF_RESERVED: c_uint = 1 << 3; // 8, 0x8
pub const RHDF_SOLID: c_uint = 1 << 4; // 16, 0x10
pub const RHDF_DIRECTORY: c_uint = 1 << 5; // 32, 0x20

pub const UCM_CHANGEVOLUME: c_uint = 0;
pub const UCM_PROCESSDATA: c_uint = 1;
pub const UCM_NEEDPASSWORD: c_uint = 2;
pub const UCM_CHANGEVOLUMEW: c_uint = 3;
pub const UCM_NEEDPASSWORDW: c_uint = 4;

// RAROpenArchiveDataEx::Flags
pub const ROADF_VOLUME: c_uint = 0x0001;
pub const ROADF_COMMENT: c_uint = 0x0002;
pub const ROADF_LOCK: c_uint = 0x0004;
pub const ROADF_SOLID: c_uint = 0x0008;
pub const ROADF_NEWNUMBERING: c_uint = 0x0010;
pub const ROADF_SIGNED: c_uint = 0x0020;
pub const ROADF_RECOVERY: c_uint = 0x0040;
pub const ROADF_ENCHEADERS: c_uint = 0x0080;
pub const ROADF_FIRSTVOLUME: c_uint = 0x0100;

// RAROpenArchiveDataEx::OpFlags
pub const ROADOF_KEEPBROKEN: c_uint = 0x0001;

// ----------------- IO BRIDGE（本 fork 扩展，对应 vendor dll.hpp 同名结构） ----------------- //
//
// 设计说明见 unrar_sys/BRIDGE.md。要点：回调在 unrar 调用线程【同步】触发；
// Size/ReadAt 任一缺槽会在 C 侧按打开失败拒绝，绝不解引用空槽。

/// 桥回调：档案总字节数；返回 <0 视为读错误。
pub type IoSizeFn = extern "C" fn(user_data: *mut c_void) -> i64;

/// 桥回调：从绝对 offset 定位读至多 len 字节进 buf，返回实际读取数
/// （0=EOF，短读合法，<0=读错误）。
pub type IoReadAtFn =
    extern "C" fn(user_data: *mut c_void, offset: u64, buf: *mut c_void, len: c_uint) -> i64;

/// C 侧 `RARIOBridge` 的镜像（vendor/unrar/dll.hpp）。
#[repr(C)]
pub struct IoBridge {
    pub user_data: *mut c_void,
    pub size: Option<IoSizeFn>,
    pub read_at: Option<IoReadAtFn>,
}

/// C 侧 `RAROpenArchiveDataEx2` 的镜像：`base` 必须是第一成员；同处于
/// pack(1) 契约下（Base 实际 172 字节，bridge 紧随其未对齐尾偏移），故同样
/// repr(C, packed)，保证 bridge 落在 C 侧读取的偏移 172。
#[repr(C, packed)]
pub struct OpenArchiveDataEx2 {
    pub base: OpenArchiveDataEx,
    pub bridge: *const IoBridge,
}

pub type ChangeVolProc = extern "C" fn(*mut c_char, c_int) -> c_int;
pub type ProcessDataProc = extern "C" fn(*mut c_uchar, c_int) -> c_int;
pub type Callback = extern "C" fn(UINT, LPARAM, LPARAM, LPARAM) -> c_int;

#[repr(C)]
pub struct Handle {
    _private: [u8; 0],
}

// ----------------- STRUCTS ----------------- //

#[repr(C)]
pub struct HeaderData {
    pub archive_name: [c_char; 260],
    pub filename: [c_char; 260],
    pub flags: c_uint,
    pub pack_size: c_uint,
    pub unp_size: c_uint,
    pub host_os: c_uint,
    pub file_crc: c_uint,
    pub file_time: c_uint,
    pub unp_ver: c_uint,
    pub method: c_uint,
    pub file_attr: c_uint,
    pub comment_buffer: *mut c_char,
    pub comment_buffer_size: c_uint,
    pub comment_size: c_uint,
    pub comment_state: c_uint,
}

#[repr(C)]
pub struct HeaderDataEx {
    pub archive_name: [c_char; 1024],
    pub archive_name_w: [wchar_t; 1024],
    pub filename: [c_char; 1024],
    pub filename_w: [wchar_t; 1024],
    pub flags: c_uint,
    pub pack_size: c_uint,
    pub pack_size_high: c_uint,
    pub unp_size: c_uint,
    pub unp_size_high: c_uint,
    pub host_os: c_uint,
    pub file_crc: c_uint,
    pub file_time: c_uint,
    pub unp_ver: c_uint,
    pub method: c_uint,
    pub file_attr: c_uint,
    pub comment_buffer: *mut c_char,
    pub comment_buffer_size: c_uint,
    pub comment_size: c_uint,
    pub comment_state: c_uint,
    pub dict_size: c_uint,
    pub hash_type: c_uint,
    pub hash: [c_char; 32],
    pub redir_type: c_uint,
    pub redir_name: *mut wchar_t,
    pub redir_name_size: c_uint,
    pub dir_target: c_uint,
    pub mtime_low: c_uint,
    pub mtime_high: c_uint,
    pub ctime_low: c_uint,
    pub ctime_high: c_uint,
    pub atime_low: c_uint,
    pub atime_high: c_uint,
    pub reserved: [c_uint; 988],
}

#[repr(C)]
pub struct OpenArchiveData {
    pub archive_name: *const c_char,
    pub open_mode: c_uint,
    pub open_result: c_uint,
    pub comment_buffer: *mut c_char,
    pub comment_buffer_size: c_uint,
    pub comment_size: c_uint,
    pub comment_state: c_uint,
}

// ABI 对齐注记（fork 修正）：vendor dll.hpp 整体处于 `#pragma pack(1)` 之下，
// C 侧 RAROpenArchiveDataEx 实际 172 字节——指针字段存在非 8 对齐偏移
// （CmtBufW@68）。`#[repr(C)]` 的自然对齐会把本结构膨胀为 184 字节，与其后
// 追加的字段全部错位；上游旧声明因消费面都在偏移 68 之前而侥幸可用，桥模式
// 需要在结构之后扩展字段，必须逐字节对齐。repr(C, packed) 与 pack(1) 同构
// （禁对字段取引用，本 crate 全部按值读写）。
#[repr(C, packed)]
pub struct OpenArchiveDataEx {
    pub archive_name: *const c_char,
    pub archive_name_w: *const wchar_t,
    pub open_mode: c_uint,
    pub open_result: c_uint,
    pub comment_buffer: *mut c_char,
    pub comment_buffer_size: c_uint,
    pub comment_size: c_uint,
    pub comment_state: c_uint,
    pub flags: c_uint,
    pub callback: Option<Callback>,
    pub user_data: LPARAM,
    pub op_flags: c_uint,
    pub comment_buffer_w: *mut wchar_t,
    pub reserved: [c_uint; 25],
}

// ----------------- BINDINGS ----------------- //

#[link(name = "unrar", kind = "static")]
#[cfg_attr(
    all(windows, target_env = "gnu"),
    link(name = "stdc++", kind = "static", modifiers = "-bundle")
)]
#[cfg_attr(target_os = "macos", link(name = "c++"))]
#[cfg_attr(any(target_os = "freebsd", target_os = "openbsd"), link(name = "c++"))]
#[cfg_attr(any(target_os = "linux", target_os = "netbsd"), link(name = "stdc++"))]
extern "C" {
    pub fn RAROpenArchive(data: *const OpenArchiveData) -> *const Handle;

    pub fn RAROpenArchiveEx(data: *const OpenArchiveDataEx) -> *const Handle;

    /// 桥模式打开（fork 扩展）。注意 unrar 语义：`IsArchive` 失败时仍返回
    /// 非 NULL 句柄且 `base.open_result != 0`——调用方必须检查 open_result。
    pub fn RAROpenArchiveEx2(data: *mut OpenArchiveDataEx2) -> *const Handle;

    pub fn RARCloseArchive(handle: *const Handle) -> c_int;

    pub fn RARReadHeader(handle: *const Handle, header_data: *const HeaderData) -> c_int;

    pub fn RARReadHeaderEx(handle: *const Handle, header_data: *const HeaderDataEx) -> c_int;

    pub fn RARProcessFile(
        handle: *const Handle,
        operation: c_int,
        dest_path: *const c_char,
        dest_name: *const c_char,
    ) -> c_int;

    pub fn RARProcessFileW(
        handle: *const Handle,
        operation: c_int,
        dest_path: *const wchar_t,
        dest_name: *const wchar_t,
    ) -> c_int;

    pub fn RARSetCallback(handle: *const Handle, callback: Option<Callback>, user_data: LPARAM);

    pub fn RARSetChangeVolProc(handle: *const Handle, change_vol_proc: Option<ChangeVolProc>);

    pub fn RARSetProcessDataProc(handle: *const Handle, process_data_proc: Option<ProcessDataProc>);

    pub fn RARSetPassword(handle: *const Handle, password: *const c_char);

    pub fn RARGetDllVersion() -> c_int;
}

impl OpenArchiveDataEx2 {
    /// 桥模式打开参数（fork 扩展）。`bridge` 指针须在 `RAROpenArchiveEx2`
    /// 返回前保持有效；来源对象生命周期见 [`io_bridge`] 模块说明。
    pub fn new(bridge: *const IoBridge, mode: c_uint) -> Self {
        Self::with_callback(bridge, mode, None, 0)
    }

    /// 同 [`OpenArchiveDataEx2::new`]，另随打开注册 unrar 回调（UCM_* 消息
    /// 面）。头加密容器要求回调在 open 期即在位（vendor archive.cpp
    /// IsArchive：`Cmd->Callback==NULL → SilentOpen=true` 会跳过加密头处理）。
    pub fn with_callback(
        bridge: *const IoBridge,
        mode: c_uint,
        callback: Option<Callback>,
        user_data: LPARAM,
    ) -> Self {
        let mut base = OpenArchiveDataEx::new(std::ptr::null(), mode);
        base.callback = callback;
        base.user_data = user_data;
        OpenArchiveDataEx2 { base, bridge }
    }
}

// ----------------- MINIMAL ABSTRACTIONS ----------------- //

impl Default for HeaderData {
    fn default() -> Self {
        HeaderData {
            archive_name: [0; 260],
            filename: [0; 260],
            flags: 0,
            pack_size: 0,
            unp_size: 0,
            host_os: 0,
            file_crc: 0,
            file_time: 0,
            unp_ver: 0,
            method: 0,
            file_attr: 0,
            comment_buffer: std::ptr::null_mut(),
            comment_buffer_size: 0,
            comment_size: 0,
            comment_state: 0,
        }
    }
}

impl Default for HeaderDataEx {
    fn default() -> Self {
        HeaderDataEx {
            archive_name: [0; 1024],
            archive_name_w: [0; 1024],
            filename: [0; 1024],
            filename_w: [0; 1024],
            flags: 0,
            pack_size: 0,
            pack_size_high: 0,
            unp_size: 0,
            unp_size_high: 0,
            host_os: 0,
            file_crc: 0,
            file_time: 0,
            unp_ver: 0,
            method: 0,
            file_attr: 0,
            comment_buffer: std::ptr::null_mut(),
            comment_buffer_size: 0,
            comment_size: 0,
            comment_state: 0,
            dict_size: 0,
            hash_type: 0,
            hash: [0; 32],
            redir_type: 0,
            redir_name: std::ptr::null_mut(),
            redir_name_size: 0,
            dir_target: 0,
            mtime_low: 0,
            mtime_high: 0,
            ctime_low: 0,
            ctime_high: 0,
            atime_low: 0,
            atime_high: 0,
            reserved: [0; 988],
        }
    }
}

impl OpenArchiveData {
    pub fn new(archive: *const c_char, mode: c_uint) -> Self {
        Self::with_comment_buffer(archive, mode, std::ptr::null_mut(), 0)
    }

    pub fn with_comment_buffer(
        archive_name: *const c_char,
        open_mode: c_uint,
        buffer: *mut c_char,
        buffer_size: c_uint,
    ) -> Self {
        OpenArchiveData {
            archive_name: archive_name,
            open_mode: open_mode,
            comment_buffer: buffer,
            comment_buffer_size: buffer_size,
            // set by library:
            open_result: 0,
            comment_size: 0,
            comment_state: 0,
        }
    }
}

impl OpenArchiveDataEx {
    #[cfg(any(target_os = "linux", target_os = "netbsd"))]
    pub fn new(archive: *const c_char, mode: c_uint) -> Self {
        Self::new_internal(archive, std::ptr::null(), mode)
    }

    #[cfg(not(any(target_os = "linux", target_os = "netbsd")))]
    pub fn new(archive: *const wchar_t, mode: c_uint) -> Self {
        Self::new_internal(std::ptr::null(), archive, mode)
    }

    fn new_internal(
        archive_name: *const c_char,
        archive_name_w: *const wchar_t,
        mode: c_uint,
    ) -> Self {
        OpenArchiveDataEx {
            archive_name,
            archive_name_w,
            open_mode: mode,
            open_result: 0,
            comment_buffer: std::ptr::null_mut(),
            comment_buffer_size: 0,
            comment_size: 0,
            comment_state: 0,
            flags: 0,
            callback: None,
            user_data: 0,
            op_flags: 0,
            comment_buffer_w: std::ptr::null_mut(),
            reserved: [0; 25],
        }
    }
}

// ----------------- IO BRIDGE SAFE WRAPPER（std 特性下） ----------------- //

/// 桥模式最小安全封装：把宿主随机读来源接进 unrar，替代整包落盘。
///
/// # 线程模型（实现方必须遵守的约束）
///
/// unrar 在调用 `RARReadHeaderEx`/`RARProcessFile` 的**同一线程**上同步触发
/// [`IoSource`] 回调——宿主把 unrar 调用放进 `spawn_blocking` 时，回调即在
/// 阻塞池线程执行，来源内部不得依赖调用方线程局部状态。因此 [`IoSource`]
/// 要求 `Send + Sync`：
/// - `Send`：来源对象经 `Arc` 从 open 线程移交到 unrar 调用线程；
/// - `Sync`：`&self` 共享读；多档案句柄可各持 `Arc` 克隆并发读同一来源。
///
/// # 生命周期（使用方无须手工管理）
///
/// 来源 `Arc` 被 Box 装载后以裸指针交给 C 侧 [`IoBridge::user_data`]；
/// [`IoArchive`] 持有该 Box，`Drop` 先 `RARCloseArchive`、后释放 Box——
/// C 侧指针自 `RAROpenArchiveEx2` 成功返回至 `RARCloseArchive` 返回全程有效
/// （顺序由 Rust 字段析构规则保证：`Drop::drop` 先于字段析构）。
///
/// # panic 语义（FFI 铁律）
///
/// 两个 C 回调体均 `catch_unwind`：宿主实现内的 panic 一律转 `-1` 读错误
/// 返回给 unrar（上层转 `ERAR_EREAD`），绝不穿越 FFI 边界。
pub mod io_bridge {
    use super::*;
    use std::boxed::Box;
    use std::io;
    use std::ptr::NonNull;
    use std::sync::Arc;

    /// 桥模式随机读来源。语义与 [`IoReadAtFn`]/[`IoSizeFn`] 一致，
    /// 以 `std::io::Result` 表达错误（Err → C 侧 -1 → unrar 读错误）。
    pub trait IoSource: Send + Sync {
        /// 档案总字节数。
        fn size(&self) -> io::Result<u64>;
        /// 定位读：向 buf 写入至多 `buf.len()` 字节，返回实际读取数
        /// （0=EOF；POSIX pread 语义，短读合法）；`offset` 保证 <
        /// [`IoSource::size`]。短读由桥 trampoline 聚合（unrar 头链按精确
        /// 字节数消费，聚合责任在桥边界，见 [`read_at_trampoline`]）。
        fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<usize>;
    }

    /// C 侧 user_data 指向的宿主上下文（Arc 装载，unrar 只透传指针）。
    struct BridgeCtx {
        src: Arc<dyn IoSource>,
    }

    /// C 回调 trampoline：size。铁律 catch_unwind（模块级 doc 注明）。
    extern "C" fn size_trampoline(user_data: *mut c_void) -> i64 {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let ctx = unsafe { &*(user_data as *const BridgeCtx) };
            ctx.src.size().map(|n| n.min(i64::MAX as u64) as i64)
        }));
        match result {
            Ok(Ok(n)) => n,
            _ => -1, // panic 或宿主 Err → unrar 读错误链
        }
    }

    /// C 回调 trampoline：read_at。铁律 catch_unwind（模块级 doc 注明）。
    ///
    /// 填充循环：unrar 上层按精确字节数消费（`File::Read` 对常规文件不做
    /// 短读聚合，头链解析直接比对请求长度），宿主 POSIX pread 短读必须在
    /// 桥边界聚合为满读；宿主返回 0（EOF）即止，聚合后的短读如实上报
    /// （截断来源 → 头链校验失败 → 归档错误，语义正确）。
    extern "C" fn read_at_trampoline(
        user_data: *mut c_void,
        offset: u64,
        buf: *mut c_void,
        len: c_uint,
    ) -> i64 {
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> io::Result<usize> {
                let ctx = unsafe { &*(user_data as *const BridgeCtx) };
                // 防御式钳制：unrar 单次请求上限为其内部缓冲（≤4MB），此处仅
                // 保证 slice 构造合法，不改变语义。
                let n = (len as usize).min(isize::MAX as usize);
                let slice = unsafe { std::slice::from_raw_parts_mut(buf.cast::<u8>(), n) };
                let mut filled = 0usize;
                while filled < slice.len() {
                    let k = ctx
                        .src
                        .read_at(offset + filled as u64, &mut slice[filled..])?;
                    if k == 0 {
                        break; // EOF
                    }
                    filled += k;
                }
                Ok(filled)
            }));
        match result {
            Ok(Ok(k)) => k.min(i64::MAX as usize) as i64,
            _ => -1, // panic 或宿主 Err → unrar 读错误链
        }
    }

    /// 桥模式打开的档案句柄守卫：独占持有 unrar 句柄与宿主上下文。
    ///
    /// 后续 `RARReadHeaderEx`/`RARProcessFile`/`RARSetPassword` 等调用经
    /// [`IoArchive::handle`] 取裸句柄进行（与本文件其余 extern 声明配合）。
    pub struct IoArchive {
        handle: NonNull<Handle>,
        // 字段序即析构序（handle 语义上先于 ctx 失效）：Drop::drop 先跑
        // RARCloseArchive，返回后 ctx 的 Box 才在字段析构阶段释放。
        _ctx: Box<BridgeCtx>,
    }

    impl IoArchive {
        /// 底层 unrar 句柄；配套 RARReadHeaderEx/RARProcessFile 等使用。
        pub fn handle(&self) -> *mut Handle {
            self.handle.as_ptr()
        }
    }

    impl std::fmt::Debug for IoArchive {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            // 不展开来源内容（宿主对象无 Debug 约束），仅结构性输出。
            f.debug_struct("IoArchive").finish_non_exhaustive()
        }
    }

    impl Drop for IoArchive {
        fn drop(&mut self) {
            unsafe { RARCloseArchive(self.handle.as_ptr() as *const _) };
            // self._ctx 在本函数返回后（字段析构阶段）释放——
            // C 侧 user_data 有效性覆盖至 RARCloseArchive 返回。
        }
    }

    /// 以内存/网络等随机读来源打开 RAR 档案（不触碰磁盘路径）。
    ///
    /// `open_mode` 取 `RAR_OM_LIST`/`RAR_OM_LIST_INCSPLIT`/`RAR_OM_EXTRACT`。
    /// 返回 `Err(open_result)`（`ERAR_*` 码）；注意 unrar 语义下坏档案以
    /// 「非 NULL 句柄 + open_result!=0」表达，此处已归一为 Err。
    pub fn open_with_io(src: Arc<dyn IoSource>, open_mode: c_uint) -> Result<IoArchive, c_int> {
        open_with_io_and_callback(src, open_mode, None, 0)
    }

    /// 同 [`open_with_io`]，另随打开注册 unrar 回调（UCM_* 消息面：凭据
    /// 询问/进度/换卷，宿主自管 `user_data` 生命周期）。
    ///
    /// 头加密容器要求回调在 open 期即在位：vendor archive.cpp IsArchive 的
    /// `Cmd->Callback==NULL → SilentOpen=true` 会跳过加密头处理（凭据询问
    /// 不再发生，语义退化为首个头读 ERAR_BAD_PASSWORD）。
    pub fn open_with_io_and_callback(
        src: Arc<dyn IoSource>,
        open_mode: c_uint,
        callback: Option<Callback>,
        user_data: LPARAM,
    ) -> Result<IoArchive, c_int> {
        let bridge_ctx = Box::into_raw(Box::new(BridgeCtx { src }));
        let bridge = IoBridge {
            user_data: bridge_ctx.cast(),
            size: Some(size_trampoline),
            read_at: Some(read_at_trampoline),
        };
        let mut data = OpenArchiveDataEx2::with_callback(
            &bridge as *const IoBridge,
            open_mode,
            callback,
            user_data,
        );
        let handle = unsafe { RAROpenArchiveEx2(&mut data as *mut _) };
        match NonNull::new(handle as *mut Handle) {
            // unrar 语义：IsArchive 失败仍返回非 NULL + open_result!=0，归一为 Err。
            Some(h) if data.base.open_result == ERAR_SUCCESS as c_uint => {
                // 收回 Box 交给守卫持有（不重新构造 Arc——来源所有权闭合于守卫）。
                let ctx = unsafe { Box::from_raw(bridge_ctx) };
                Ok(IoArchive {
                    handle: h,
                    _ctx: ctx,
                })
            }
            _ => unsafe {
                if !handle.is_null() {
                    RARCloseArchive(handle as *const _);
                }
                drop(Box::from_raw(bridge_ctx));
                Err(if data.base.open_result == 0 {
                    ERAR_UNKNOWN
                } else {
                    data.base.open_result as c_int
                })
            },
        }
    }

    #[cfg(test)]
    mod trampoline_tests {
        use super::*;

        /// panic 面（FFI 铁律）：宿主 panic 不得穿越 trampoline，一律转 -1。
        struct Panicking;
        impl IoSource for Panicking {
            fn size(&self) -> io::Result<u64> {
                panic!("boom size")
            }
            fn read_at(&self, _: u64, _: &mut [u8]) -> io::Result<usize> {
                panic!("boom read")
            }
        }

        #[test]
        fn panic_in_callbacks_maps_to_minus_one() {
            let ctx = Box::into_raw(Box::new(BridgeCtx {
                src: Arc::new(Panicking),
            }));
            let s = size_trampoline(ctx.cast());
            let mut buf = [0u8; 4];
            let r = read_at_trampoline(ctx.cast(), 0, buf.as_mut_ptr().cast(), 4);
            drop(unsafe { Box::from_raw(ctx) }); // 测试内手工收回，不走 IoArchive 守卫
            assert_eq!(s, -1);
            assert_eq!(r, -1);
        }

        /// 正常面：trampoline 透传宿主结果（size 与定位读字节数）。
        struct Fixed(u64);
        impl IoSource for Fixed {
            fn size(&self) -> io::Result<u64> {
                Ok(self.0)
            }
            fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
                let offset = (offset as usize).min(self.0 as usize);
                let n = buf.len().min(self.0 as usize - offset);
                for b in buf.iter_mut().take(n) {
                    *b = 0xAB;
                }
                Ok(n)
            }
        }

        #[test]
        fn callbacks_pass_through_host_results() {
            let ctx = Box::into_raw(Box::new(BridgeCtx {
                src: Arc::new(Fixed(10)),
            }));
            let s = size_trampoline(ctx.cast());
            let mut buf = [0u8; 4];
            let r = read_at_trampoline(ctx.cast(), 6, buf.as_mut_ptr().cast(), 4);
            drop(unsafe { Box::from_raw(ctx) });
            assert_eq!(s, 10);
            assert_eq!(r, 4); // offset=6、总长=10 → 短读收敛为 4 字节
        }
    }
}

// ----------------- TESTS ----------------- //

#[cfg(test)]
mod layout_tests {
    use super::*;

    // pack(1) 契约锁定（LP64 主机）：Rust 镜像与 vendor dll.hpp 逐字节对齐。
    // 防回归点：repr(C) 自然对齐会膨胀 OpenArchiveDataEx 至 184 字节并使其后
    // 扩展字段全部错位（桥模式曾因此读到 NULL Bridge）——本测试常驻拦截。
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn open_archive_data_mirrors_packed_c_layout() {
        assert_eq!(std::mem::size_of::<IoBridge>(), 24);
        assert_eq!(std::mem::size_of::<OpenArchiveDataEx>(), 172);
        assert_eq!(std::mem::offset_of!(OpenArchiveDataEx, op_flags), 64);
        assert_eq!(
            std::mem::offset_of!(OpenArchiveDataEx, comment_buffer_w),
            68
        );
        assert_eq!(std::mem::offset_of!(OpenArchiveDataEx, reserved), 72);
        assert_eq!(std::mem::size_of::<OpenArchiveDataEx2>(), 180);
        assert_eq!(std::mem::offset_of!(OpenArchiveDataEx2, bridge), 172);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_version() {
        assert_eq!(unsafe { super::RARGetDllVersion() }, 9);
    }
}
