# unrar_sys IO 输入桥（fork 扩展）设计说明

> KC-07 方案 A 落地件：为 unrar 增加输入回调桥，宿主以随机读来源（内存/网络/
> 流式缓存）直接供给档案字节，**不再需要整包落盘的临时文件桥**。
>
> 上游：[muja/unrar.rs](https://github.com/muja/unrar.rs) v0.5.8；
> fork：[shunFSKi/unrar.rs](https://github.com/shunFSKi/unrar.rs)。
> 本文件描述 fork 相对上游的全部补丁面与设计取舍。

## 一、背景与约束

- 上游 unrar_sys 的 DLL API 仅收**路径**（`RAROpenArchiveEx` → `Archive::Open` →
  `File::Open` → `open(2)`），宿主若持有的是字节流只能整包写临时文件——违反
  喔喔漫画重建「不整包副本」原则（主仓库 KC-07 登记）。
- unrar 全部档案磁盘读收敛在 `File` 类（勘察实证）：
  - `dll.cpp:33` `RAROpenArchiveEx` → `Data->Arc.Open(ArcName, FMF_OPENSHARED)`；
  - `file.cpp:47` `File::Open` → 平台 `open`/`CreateFile`（filefn.cpp 同族）；
  - 读路径 `File::Read` → `File::DirectRead` → `read(2)`；寻位 `File::RawSeek`；
    定位 `File::Tell`；长度 `File::FileLength`。
  - `Archive` 类继承 `File` 并覆盖 `Open/Read/Seek/Tell` 四个虚函数
    （archive.hpp:89-92），但覆盖层只做 QOpen 分流，最终仍回落 `File::xxx`——
    **在 File 层分流即覆盖 unrar 上层（archive/arcread/extract）全部读路径**，
    上层零改动。
- QOpen 快开机制（qopen.cpp）只在内存缓存头链，回源仍走 `Arc->Read/Seek`，
  无独立磁盘句柄——桥模式天然兼容。

## 二、挂点定稿：新 API `RAROpenArchiveEx2`（非保留字段劫持）

二选一裁决：**新增 API**，不动 `RAROpenArchiveDataEx`。

理由：

1. 上游有征用保留区的前科——`RARHeaderDataEx` 的 `RedirType/RedirName` 即
   「taken from Reserved area」（dll.cpp 内上游注释自证）。劫持
   `RAROpenArchiveDataEx.Reserved` 在上游合并时极易碰撞。
2. 新结构 `RAROpenArchiveDataEx2 { RAROpenArchiveDataEx Base; RARIOBridge* Bridge; }`
   使官方全部既有符号与结构**零改动**，fork diff 面最小、可审、可回退。
3. 与上游演进惯例一致（`RARGetDllVersion` 亦为后加导出）。

```
============================ C 侧新增（vendor/unrar/dll.hpp） ============================
struct RARIOBridge {
  void       *UserData;      // 宿主不透明指针，unrar 只透传
  long long (*Size)(void*);  // 档案总长；<0 = 读错误
  long long (*ReadAt)(void*, unsigned long long Offset, void *Buf, unsigned int BufSize);
                             // 定位读；0=EOF；<0=读错误；短读合法
};
struct RAROpenArchiveDataEx2 { RAROpenArchiveDataEx Base; RARIOBridge *Bridge; };
HANDLE PASCAL RAROpenArchiveEx2(struct RAROpenArchiveDataEx2 *r);
==========================================================================================
```

类型用 `long long`/`unsigned long long` 而非 unrar 内部 `int64`，保证 dll.hpp
可脱离 unrar 内部头被 FFI 侧独立镜像。

## 三、File 层分流（vendor/unrar/file.{hpp,cpp}）

`File` 类新增两字段与一入口：

- `RARIOBridge *Bridge`（默认 `nullptr`，不持有——生命周期归挂接方）；
- `int64 BridgePos`（File 自持逻辑位置，回调一律按绝对偏移读）；
- `SetIOBridge(Bridge)`（须在 `Open` 之前挂接）。

五个分流点（全部早退分支，磁盘路径零触碰）：

```
======================== 分流矩阵 ========================
方法            桥模式行为                                磁盘语义对照
--------------- ---------------------------------------- ----------------
Open            置位即成：FileName/BridgePos 归零，true   open(2)
DirectRead      ReadAt(BridgePos) 并推进；宿主错误        read(2)
                （<0，含 panic 转译）跳过 FREM_ASK 交互
                链直接抛 RARX_READ → ERAR_EREAD
RawSeek         纯位置算术（SET/CUR/END + 负偏移归一化，  lseek(2)
                Target<0 → false → SeekError 链）
Tell            返回 BridgePos                            lseek(SEEK_CUR)
FileLength      Size() 回调；<0 抛 RARX_READ（不返回负    定长探测
                长度污染上层边界检查）
IsOpened        Bridge!=nullptr || hFile 有效             -
Close/析构      零改动自然兼容（hFile 恒为 BAD_HANDLE，
                fd 关闭路径跳过；桥生命周期归 Rust 侧）
==========================================================
```

要点：

- 分流挂在 `DirectRead`（最低层）而非 `Read`——`File::Read` 的短读循环、
  错误重试、`CurFilePos` 统计逻辑原样复用；
- **宿主错误不进交互链**：DLL 构建未定义 SILENT，上游 `File::Read` 错误路径
  的 `uiAskRepeatRead`（uiconsole.cpp）会走 stdin 询问——GUI 宿主无控制台
  语义，桥模式在该路径前直接抛 `RARX_READ`（语义即无头消费方的
  「错误→ERAR_EREAD」，非静默截断、非交互挂起）；

要点：

- 分流挂在 `DirectRead`（最低层）而非 `Read`——`File::Read` 的短读循环、
  错误重试、`CurFilePos` 统计逻辑原样复用；
- `Archive` 四个虚函数覆盖层与 QOpen 无需任何改动（均回落 File 层）；
- 解压目标文件等其余 `File` 使用者保持默认 `nullptr`，只有档案本体挂桥——
  上游行为零变化（`Bridge==nullptr` 时每个分支即原有代码）。

## 四、dll.cpp 接线

- 原 `RAROpenArchiveEx` 主体抽为 `static OpenArchiveInternal(r, Bridge)`；
  `RAROpenArchiveEx` = `OpenArchiveInternal(r, nullptr)`（官方路径逐行不变）；
  `RAROpenArchiveEx2` 校验回调槽非空后走共用主体。
- 桥回调表**拷贝进 `DataSet`**（进程内副本），`Arc.SetIOBridge(&Data->Bridge)`——
  File 持有的指针与 DataSet 同生共死，`RARCloseArchive` 内 `delete Data` 时
  桥指针随之湮灭，无悬垂窗口。
- 宿主回调表不完整（Size/ReadAt 任一空槽）→ `ERAR_UNKNOWN` 打开失败，
  绝不解引用空槽。
- `dll.def` / `dll_nocrypt.def` 增补 `RAROpenArchiveEx2` 导出。

## 五、Rust 绑定（src/lib.rs）

FFI 面（no_std 兼容）：

- `IoBridge` / `OpenArchiveDataEx2`（`#[repr(C)]` 镜像）+ `RAROpenArchiveEx2`
  extern 声明。

std 门控安全封装（`io_bridge` 模块）：

- `trait IoSource: Send + Sync { fn size(); fn read_at(offset, buf); }`；
- `open_with_io(Arc<dyn IoSource>, open_mode) -> Result<IoArchive, ERAR 码>`；
- `IoArchive` 守卫：`handle()` 取裸句柄配 `RARReadHeaderEx`/`RARProcessFile`/
  `RARSetPassword` 等既有 extern 使用。

**生命周期**：来源 `Arc` 被 Box 装载后以裸指针交给 C；`IoArchive` 持有该 Box，
`Drop` 先 `RARCloseArchive`、后（字段析构阶段）释放 Box——C 侧 `user_data`
自打开成功至关闭返回全程有效，顺序由 Rust 字段析构规则保证，使用方零手工管理。

**线程模型**（`IoSource: Send + Sync` 的依据，写死在模块 doc）：
unrar 在调用 `RARReadHeaderEx`/`RARProcessFile` 的**同一线程**同步触发回调。
宿主把 unrar 调用放进 `spawn_blocking`（阻塞 C 调用铁律）时，回调即在阻塞池
线程执行：`Send` 支撑 Arc 跨线程移交，`Sync` 支撑多句柄并发共享读。回调内
驱动异步来源用 `tokio Handle::block_on`（主仓库 pdf.rs / zip SyncBridge 先例
同构）。

**短读聚合责任在桥边界（实施期定稿）**：`IoSource::read_at` 保持 POSIX pread
语义（短读合法、0=EOF），但 unrar 头链按**精确字节数**消费（`File::Read` 对
常规文件不做短读聚合，`IsArchive` 直接比对签名读取长度）——`read_at_trampoline`
内置填充循环把宿主短读聚合为满读，宿主 EOF 即止、聚合后短读如实上报
（截断来源 → 头链校验失败 → 归档错误，语义正确）。tests/io_bridge.rs 的
「每次 1 字节」用例锁此契约。

**ABI 对齐（实施期发现并修正）**：vendor `dll.hpp` 整体处于 `#pragma pack(1)`
之下，C 侧 `RAROpenArchiveDataEx` 实际 172 字节且指针字段存在非对齐偏移
（`CmtBufW`@68）；上游 Rust 镜像 `#[repr(C)]` 自然对齐膨胀为 184 字节——
上游因消费字段全在偏移 68 之前而侥幸可用，桥契约在结构之后扩展字段则必然
错位（首验实证：C 在偏移 172 读 `Bridge` 落进 reserved 区读到 NULL）。修正：
`OpenArchiveDataEx`/`OpenArchiveDataEx2` 改 `#[repr(C, packed)]`，并以
`layout_tests::open_archive_data_mirrors_packed_c_layout` 常驻锁定。
**同源漂移登记未修**：上游 Rust `HeaderDataEx`（reserved 988 vs C 982 +
ArcNameEx/FileNameEx 族）同样在 `comment_buffer`@5164 处开始错位——上游
消费面（文件名/标志/尺寸）恰在漂移点之前故未暴露；K8.2 若需消费
`hash/dict_size/mtime` 等后段字段须先同法修正。

**panic 语义（FFI 铁律）**：两个 trampoline 均 `catch_unwind`，宿主 panic 一律
转 `-1` 读错误返回 unrar，绝不穿越 FFI 边界。C→Rust 方向的入口
（`RAROpenArchiveEx2` 等）维持上游 try/catch（RAR_EXIT/bad_alloc）不变。

## 六、边界与如实声明

- **多卷档案**：桥语义为单宿主来源；多卷触发 `MergeArchive` 时经
  UCM_CHANGEVOLUME 回调询问下一卷，宿主返回停止即得 `ERAR_EOPEN`——安全
  失败，无 UB。
- **头加密**：与路径模式同链（`UCM_NEEDPASSWORD(W)` 回调或 `RARSetPassword`），
  桥不改变凭据面。
- **全局 ErrHandler**：unrar 上游使用进程级全局错误处理器，多线程并发打开
  多个档案存在上游固有的竞争面——与路径模式同源，非本桥引入；宿主侧按
  每档案串行使用即可（主仓库 RarReader 既有形态）。
- **读错误交互路径已消除**：DLL 构建未定义 SILENT，上游 `File::Read` 错误链
  的 `uiAskRepeatRead`（uiconsole.cpp）走 stdin 询问；桥模式在分流点直接抛
  `RARX_READ`，宿主错误确定性地转 `ERAR_EREAD`，不触交互、不挂起（见 §3）。

## 七、补丁面清单（相对上游 0.5.8）

```
============================== 补丁面 ==============================
vendor/unrar/dll.hpp        RARIOBridge/RAROpenArchiveDataEx2 结构
                            + RAROpenArchiveEx2 声明（含设计注释）
vendor/unrar/file.hpp       File 增 Bridge/BridgePos 字段与 SetIOBridge
                            + IsOpened 桥分支 + RARIOBridge 前置声明
vendor/unrar/file.cpp       构造/operator= 透传 + Open/DirectRead/
                            RawSeek/Tell/FileLength 五分流点
vendor/unrar/dll.cpp        DataSet 增 Bridge 副本；主体抽
                            OpenArchiveInternal；新增 RAROpenArchiveEx2
vendor/unrar/dll.def        增补导出
vendor/unrar/dll_nocrypt.def 增补导出
src/lib.rs                  FFI 镜像 + io_bridge 安全封装模块
                            + OpenArchiveDataEx(2) repr(C,packed) 修正
                            + trampoline 单测 + 布局锁定测试
tests/io_bridge.rs          最小验证六例：内存列目录（交叉路径模式）、
                            解压全链、1 字节短读、宿主 Err/panic、
                            非档案字节
====================================================================
```

## 八、开发注意（fork 维护者必读）

- **cc 不跟踪 vendor 源变化**：cc 1.6.0 的 build 输出零 `rerun-if-changed`
  声明（实证 build output），改 `vendor/unrar/**` 后 cargo 视 unrar_sys 为
  Fresh、C++ 不会重编——每次改 C++ 须 `touch unrar_sys/build.rs` 强制重建，
  否则链接旧对象、症状诡异（首验曾因此白查一轮）。
- **测试须串行共享 unrar 全局态**：ErrHandler 为进程级全局，同进程并发
  unrar 调用互相污染错误码；tests/io_bridge.rs 以进程内 Mutex 串行。
- **列目录语义**：每次 `RARReadHeaderEx` 后必须 `RARProcessFile(RAR_SKIP)`
  推进游标（unrar crate List 迭代器同款），否则下一头从文件数据区解析必炸
  头 CRC——消费方（K8.2 RarReader）沿用主仓库既有 walk 形态即无此坑。
