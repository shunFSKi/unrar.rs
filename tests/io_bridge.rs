//! IO 输入桥最小验证（KC-07 方案 A 第一步，设计说明见 unrar_sys/BRIDGE.md）
//! ============================================================
//! ① 内存字节当档案打开能列目录，与路径模式逐一交叉一致（头链解析全走桥）；
//! ② 解压路径（RAR_TEST + UCM_PROCESSDATA）经桥全链取回内容正确
//!    （Read/Seek/解码/回调数据通道全走桥）；
//! ③ 短读容错：宿主每次只给 1 字节，unrar 上层读循环收敛；
//! ④ 错误语义：宿主 Err → ERAR_EREAD；宿主 panic → catch_unwind →
//!    ERAR_EREAD（进程不崩）；非档案字节 → ERAR_BAD_ARCHIVE（open_result
//!    非 NULL 句柄语义归一为 Err）。
//! ============================================================

use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use unrar_sys::io_bridge::{open_with_io, IoSource};

// unrar 进程级全局 ErrHandler（BRIDGE.md §6 上游固有约束）：同进程并发
// 打开/读头会互相污染错误码——本文件全部用例经此锁串行执行。
static BRIDGE_LOCK: Mutex<()> = Mutex::new(());

fn bridge_guard() -> MutexGuard<'static, ()> {
    // 不让单个失败用例的锁中毒级联成误导性失败面：中毒也照常接管。
    BRIDGE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// ============================================================
// 宿主来源实现
// ============================================================

/// 整段内存当随机读来源（桥模式基线）。
struct MemSource {
    data: Vec<u8>,
}

impl MemSource {
    fn new(data: Vec<u8>) -> Self {
        Self { data }
    }
}

impl IoSource for MemSource {
    fn size(&self) -> io::Result<u64> {
        Ok(self.data.len() as u64)
    }
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        let offset = offset as usize;
        if offset >= self.data.len() {
            return Ok(0); // EOF
        }
        let n = buf.len().min(self.data.len() - offset);
        buf[..n].copy_from_slice(&self.data[offset..offset + n]);
        Ok(n)
    }
}

/// 每次只给 1 字节的来源（短读容错面）。
struct OneByteSource {
    data: Vec<u8>,
}

impl IoSource for OneByteSource {
    fn size(&self) -> io::Result<u64> {
        Ok(self.data.len() as u64)
    }
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        let offset = offset as usize;
        if offset >= self.data.len() || buf.is_empty() {
            return Ok(0);
        }
        buf[0] = self.data[offset]; // 短读：一次 1 字节
        Ok(1)
    }
}

/// size 正常、read_at 必 panic 的来源（FFI 铁律面）。
struct PanicRead {
    len: u64,
}

impl IoSource for PanicRead {
    fn size(&self) -> io::Result<u64> {
        Ok(self.len)
    }
    fn read_at(&self, _: u64, _: &mut [u8]) -> io::Result<usize> {
        panic!("host panic in read_at")
    }
}

/// 全 Err 来源（宿主 IO 错误面）。
struct ErrSource;

impl IoSource for ErrSource {
    fn size(&self) -> io::Result<u64> {
        Err(io::Error::other("no size"))
    }
    fn read_at(&self, _: u64, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("no read"))
    }
}

// ============================================================
// 工具
// ============================================================

/// 桥模式列目录：返回 (文件名, 解包尺寸) 列表。
fn list_via_bridge(data: Vec<u8>) -> Vec<(PathBuf, u64)> {
    let arch = open_with_io(Arc::new(MemSource::new(data)), unrar_sys::RAR_OM_LIST)
        .expect("bridge open failed");
    let mut out = Vec::new();
    loop {
        let mut hd = unrar_sys::HeaderDataEx::default();
        let code = unsafe { unrar_sys::RARReadHeaderEx(arch.handle(), &mut hd as *mut _) };
        if code == unrar_sys::ERAR_END_ARCHIVE {
            break;
        }
        assert_eq!(
            code,
            unrar_sys::ERAR_SUCCESS,
            "unexpected header error: {code}"
        );
        let fh: unrar::FileHeader = hd.into();
        out.push((fh.filename.clone(), fh.unpacked_size));
        // 与 unrar crate List 迭代器同语义：读头后必须 SKIP 推进游标，
        // 否则下一次 RARReadHeaderEx 从文件数据区解析 → 头 CRC 必炸。
        let skip = unsafe {
            unrar_sys::RARProcessFile(
                arch.handle(),
                unrar_sys::RAR_SKIP,
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        assert_eq!(skip, unrar_sys::ERAR_SUCCESS, "skip failed: {skip}");
    }
    out
}

/// 路径模式列目录（交叉验证基准）。
fn list_via_path() -> Vec<(PathBuf, u64)> {
    unrar::Archive::new("data/version.rar")
        .open_for_listing()
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            (e.filename.clone(), e.unpacked_size)
        })
        .collect()
}

/// UCM_PROCESSDATA 收集回调（catch_unwind 铁律同款：panic → -1 停止处理）。
extern "C" fn collect_cb(
    msg: unrar_sys::UINT,
    user: unrar_sys::LPARAM,
    p1: unrar_sys::LPARAM,
    p2: unrar_sys::LPARAM,
) -> std::os::raw::c_int {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if msg == unrar_sys::UCM_PROCESSDATA && user != 0 {
            let out = unsafe { &mut *(user as *mut Vec<u8>) };
            let chunk = unsafe { std::slice::from_raw_parts(p1 as *const u8, p2 as usize) };
            out.extend_from_slice(chunk);
        }
        0
    }));
    result.unwrap_or(-1)
}

// ============================================================
// 用例
// ============================================================

/// ① 内存字节当档案打开能列目录，与路径模式逐一交叉一致。
#[test]
fn bridge_mem_listing_matches_path_mode() {
    let _guard = bridge_guard();
    let data = std::fs::read("data/version.rar").unwrap();
    let via_bridge = list_via_bridge(data);
    let via_path = list_via_path();
    assert!(!via_bridge.is_empty(), "bridge listing must not be empty");
    assert_eq!(via_bridge, via_path, "bridge vs path listing mismatch");
}

/// ② 解压路径经桥全链：RAR_TEST + UCM_PROCESSDATA 取回内容与已知值一致。
#[test]
fn bridge_extract_content_matches_known_payload() {
    let _guard = bridge_guard();
    let data = std::fs::read("data/version.rar").unwrap();
    let arch = open_with_io(Arc::new(MemSource::new(data)), unrar_sys::RAR_OM_EXTRACT)
        .expect("bridge open failed");
    let mut got = Vec::<u8>::new();
    unsafe {
        unrar_sys::RARSetCallback(
            arch.handle(),
            Some(collect_cb),
            &mut got as *mut _ as unrar_sys::LPARAM,
        );
    }
    let mut hd = unrar_sys::HeaderDataEx::default();
    assert_eq!(
        unsafe { unrar_sys::RARReadHeaderEx(arch.handle(), &mut hd as *mut _) },
        unrar_sys::ERAR_SUCCESS
    );
    assert_eq!(
        unsafe {
            unrar_sys::RARProcessFile(
                arch.handle(),
                unrar_sys::RAR_TEST,
                std::ptr::null(),
                std::ptr::null(),
            )
        },
        unrar_sys::ERAR_SUCCESS
    );
    // 期望值与 tests/simple.rs version_cat 同源（data/version.rar 内 payload）。
    assert_eq!(got, b"unrar-0.4.0");
}

/// ③ 短读容错：宿主每次 1 字节，头链解析与列目录仍收敛正确。
#[test]
fn bridge_tolerates_one_byte_short_reads() {
    let _guard = bridge_guard();
    let data = std::fs::read("data/version.rar").unwrap();
    let arch = open_with_io(Arc::new(OneByteSource { data }), unrar_sys::RAR_OM_LIST)
        .expect("bridge open failed (short reads)");
    let mut count = 0;
    loop {
        let mut hd = unrar_sys::HeaderDataEx::default();
        let code = unsafe { unrar_sys::RARReadHeaderEx(arch.handle(), &mut hd as *mut _) };
        if code == unrar_sys::ERAR_END_ARCHIVE {
            break;
        }
        assert_eq!(code, unrar_sys::ERAR_SUCCESS);
        let skip = unsafe {
            unrar_sys::RARProcessFile(
                arch.handle(),
                unrar_sys::RAR_SKIP,
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        assert_eq!(skip, unrar_sys::ERAR_SUCCESS, "skip failed: {skip}");
        count += 1;
    }
    assert_eq!(
        count,
        list_via_path().len(),
        "short-read listing count mismatch"
    );
}

/// ④a 宿主 Err → ERAR_EREAD（不崩、不进交互链）。
#[test]
fn host_error_maps_to_erar_eread() {
    let _guard = bridge_guard();
    let r = open_with_io(Arc::new(ErrSource), unrar_sys::RAR_OM_LIST);
    assert_eq!(r.unwrap_err(), unrar_sys::ERAR_EREAD);
}

/// ④b 宿主 panic → catch_unwind → ERAR_EREAD（进程不崩，FFI 铁律）。
#[test]
fn host_panic_becomes_open_error_not_crash() {
    let _guard = bridge_guard();
    let r = open_with_io(Arc::new(PanicRead { len: 4096 }), unrar_sys::RAR_OM_LIST);
    assert_eq!(r.unwrap_err(), unrar_sys::ERAR_EREAD);
}

/// ④c 非档案字节 → ERAR_BAD_ARCHIVE（unrar「非 NULL 句柄 + open_result!=0」
/// 语义在 open_with_io 归一为 Err）。
#[test]
fn non_archive_bytes_maps_to_bad_archive() {
    let _guard = bridge_guard();
    let garbage = vec![0u8; 1024]; // 无 RAR 签名
    let r = open_with_io(Arc::new(MemSource::new(garbage)), unrar_sys::RAR_OM_LIST);
    assert_eq!(r.unwrap_err(), unrar_sys::ERAR_BAD_ARCHIVE);
}
