#ifndef _UNRAR_DLL_
#define _UNRAR_DLL_

#pragma pack(push, 1)

#define ERAR_SUCCESS             0
#define ERAR_END_ARCHIVE        10
#define ERAR_NO_MEMORY          11
#define ERAR_BAD_DATA           12
#define ERAR_BAD_ARCHIVE        13
#define ERAR_UNKNOWN_FORMAT     14
#define ERAR_EOPEN              15
#define ERAR_ECREATE            16
#define ERAR_ECLOSE             17
#define ERAR_EREAD              18
#define ERAR_EWRITE             19
#define ERAR_SMALL_BUF          20
#define ERAR_UNKNOWN            21
#define ERAR_MISSING_PASSWORD   22
#define ERAR_EREFERENCE         23
#define ERAR_BAD_PASSWORD       24
#define ERAR_LARGE_DICT         25

#define RAR_OM_LIST              0
#define RAR_OM_EXTRACT           1
#define RAR_OM_LIST_INCSPLIT     2

#define RAR_SKIP              0
#define RAR_TEST              1
#define RAR_EXTRACT           2

#define RAR_VOL_ASK           0
#define RAR_VOL_NOTIFY        1

#define RAR_DLL_VERSION       9

#define RAR_HASH_NONE         0
#define RAR_HASH_CRC32        1
#define RAR_HASH_BLAKE2       2


#ifdef _UNIX
#define CALLBACK
#define PASCAL
#define LONG long
#define HANDLE void *
#define LPARAM long
#define UINT unsigned int
#endif

#define RHDF_SPLITBEFORE 0x01
#define RHDF_SPLITAFTER  0x02
#define RHDF_ENCRYPTED   0x04
#define RHDF_SOLID       0x10
#define RHDF_DIRECTORY   0x20


struct RARHeaderData
{
  char         ArcName[260];
  char         FileName[260];
  unsigned int Flags;
  unsigned int PackSize;
  unsigned int UnpSize;
  unsigned int HostOS;
  unsigned int FileCRC;
  unsigned int FileTime;
  unsigned int UnpVer;
  unsigned int Method;
  unsigned int FileAttr;
  char         *CmtBuf;
  unsigned int CmtBufSize;
  unsigned int CmtSize;
  unsigned int CmtState;
};


struct RARHeaderDataEx
{
  char         ArcName[1024];
  wchar_t      ArcNameW[1024];
  char         FileName[1024];
  wchar_t      FileNameW[1024];
  unsigned int Flags;
  unsigned int PackSize;
  unsigned int PackSizeHigh;
  unsigned int UnpSize;
  unsigned int UnpSizeHigh;
  unsigned int HostOS;
  unsigned int FileCRC;
  unsigned int FileTime;
  unsigned int UnpVer;
  unsigned int Method;
  unsigned int FileAttr;
  char         *CmtBuf;
  unsigned int CmtBufSize;
  unsigned int CmtSize;
  unsigned int CmtState;
  unsigned int DictSize;
  unsigned int HashType;
  char         Hash[32];
  unsigned int RedirType;
  wchar_t      *RedirName;
  unsigned int RedirNameSize;
  unsigned int DirTarget;
  unsigned int MtimeLow;
  unsigned int MtimeHigh;
  unsigned int CtimeLow;
  unsigned int CtimeHigh;
  unsigned int AtimeLow;
  unsigned int AtimeHigh;
  wchar_t      *ArcNameEx;
  unsigned int ArcNameExSize;
  wchar_t      *FileNameEx;
  unsigned int FileNameExSize;
  unsigned int Reserved[982];
};


struct RAROpenArchiveData
{
  char         *ArcName;
  unsigned int OpenMode;
  unsigned int OpenResult;
  char         *CmtBuf;
  unsigned int CmtBufSize;
  unsigned int CmtSize;
  unsigned int CmtState;
};

typedef int (CALLBACK *UNRARCALLBACK)(UINT msg,LPARAM UserData,LPARAM P1,LPARAM P2);

#define ROADF_VOLUME       0x0001
#define ROADF_COMMENT      0x0002
#define ROADF_LOCK         0x0004
#define ROADF_SOLID        0x0008
#define ROADF_NEWNUMBERING 0x0010
#define ROADF_SIGNED       0x0020
#define ROADF_RECOVERY     0x0040
#define ROADF_ENCHEADERS   0x0080
#define ROADF_FIRSTVOLUME  0x0100

#define ROADOF_KEEPBROKEN  0x0001

struct RAROpenArchiveDataEx
{
  char         *ArcName;
  wchar_t      *ArcNameW;
  unsigned int  OpenMode;
  unsigned int  OpenResult;
  char         *CmtBuf;
  unsigned int  CmtBufSize;
  unsigned int  CmtSize;
  unsigned int  CmtState;
  unsigned int  Flags;
  UNRARCALLBACK Callback;
  LPARAM        UserData;
  unsigned int  OpFlags;
  wchar_t      *CmtBufW;
  unsigned int  Reserved[25];
};

// ========================================================================
// IO 输入桥（本 fork 扩展，非 UnRAR 官方 ABI；设计说明见 unrar_sys/BRIDGE.md）
//
// 挂点选择：新增 RAROpenArchiveEx2 API + RAROpenArchiveDataEx2 结构，而不劫持
// RAROpenArchiveDataEx 的 Reserved 保留区。理由：上游有征用保留区的前科
// （RARHeaderDataEx 的 Redir* 字段即取自 Reserved，见 dll.cpp 对应注释），
// 劫持保留区在上游合并时极易碰撞；新 API 使官方全部既有符号与结构零改动。
//
// 回调线程模型：unrar 在调用 RARReadHeaderEx/RARProcessFile 的同一线程内
// 【同步】调用 Size/ReadAt（无内部线程）。宿主需保证两个回调可重入且线程安全。
// ========================================================================

// 输入回调桥：unrar 对档案的全部磁盘读（头链/数据区）收敛到 File 层后改经
// 此二回调落到宿主提供的随机读来源，宿主无需整包副本。
struct RARIOBridge
{
  // 宿主不透明指针，unrar 只透传不解引用（Rust 侧为 Box 装载的来源对象）。
  // 生命周期：必须覆盖 RAROpenArchiveEx2 成功返回至 RARCloseArchive 返回全程。
  void *UserData;

  // 档案总字节数；返回 <0 视为读错误（unrar 上层转 ERAR_EREAD）。
  // C++ 侧用 long long 而非 int64，保证 dll.hpp 可脱离 unrar 内部头独立包含。
  long long (*Size)(void *UserData);

  // 定位读：从绝对 Offset 起最多读 BufSize 字节进 Buf，返回实际读取数。
  // 返回 0 = EOF；<0 = 读错误；短读合法（unrar 上层可处理短读）。
  // Offset 保证 < Size()（unrar 不读越界位置；EOF 判定以返回 0 表达）。
  long long (*ReadAt)(void *UserData,unsigned long long Offset,void *Buf,unsigned int BufSize);
};

// RAROpenArchiveDataEx 的桥模式扩展：Base 必须是第一成员。
// Bridge 非空 = 桥模式（此时 ArcName 可为 NULL，宿主来源即档案本体）。
struct RAROpenArchiveDataEx2
{
  struct RAROpenArchiveDataEx Base;
  struct RARIOBridge *Bridge;
};

enum UNRARCALLBACK_MESSAGES {
  UCM_CHANGEVOLUME,UCM_PROCESSDATA,UCM_NEEDPASSWORD,UCM_CHANGEVOLUMEW,
  UCM_NEEDPASSWORDW,UCM_LARGEDICT
};

typedef int (PASCAL *CHANGEVOLPROC)(char *ArcName,int Mode);
typedef int (PASCAL *PROCESSDATAPROC)(unsigned char *Addr,int Size);

#ifdef __cplusplus
extern "C" {
#endif

HANDLE PASCAL RAROpenArchive(struct RAROpenArchiveData *ArchiveData);
HANDLE PASCAL RAROpenArchiveEx(struct RAROpenArchiveDataEx *ArchiveData);

// 桥模式打开（本 fork 扩展）：Bridge 非空时档案数据经回调供给，不触碰磁盘。
// 返回与错误语义与 RAROpenArchiveEx 完全一致（注意 IsArchive 失败时返回
// 非 NULL 句柄且 OpenResult!=0，调用方必须检查 OpenResult）。
HANDLE PASCAL RAROpenArchiveEx2(struct RAROpenArchiveDataEx2 *ArchiveData);

int    PASCAL RARCloseArchive(HANDLE hArcData);
int    PASCAL RARReadHeader(HANDLE hArcData,struct RARHeaderData *HeaderData);
int    PASCAL RARReadHeaderEx(HANDLE hArcData,struct RARHeaderDataEx *HeaderData);
int    PASCAL RARProcessFile(HANDLE hArcData,int Operation,char *DestPath,char *DestName);
int    PASCAL RARProcessFileW(HANDLE hArcData,int Operation,wchar_t *DestPath,wchar_t *DestName);
void   PASCAL RARSetCallback(HANDLE hArcData,UNRARCALLBACK Callback,LPARAM UserData);
void   PASCAL RARSetChangeVolProc(HANDLE hArcData,CHANGEVOLPROC ChangeVolProc);
void   PASCAL RARSetProcessDataProc(HANDLE hArcData,PROCESSDATAPROC ProcessDataProc);
void   PASCAL RARSetPassword(HANDLE hArcData,char *Password);
int    PASCAL RARGetDllVersion();

#ifdef __cplusplus
}
#endif

#pragma pack(pop)

#endif
