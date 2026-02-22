// 内存安全加固 — 防止 Token 通过 core dump / swap / 内存残留泄漏
//
// 加固措施:
// 1. 禁用 core dump (PR_SET_DUMPABLE=0, RLIMIT_CORE=0)
// 2. mlock 关键内存页 (防止 swap 到磁盘)
// 3. MADV_DONTDUMP 标记敏感内存区域
//
// 配合 jemalloc zero-on-free (在 main.rs 中配置 #[global_allocator])
// 实现堆内存释放后自动清零

use tracing::{info, warn};

/// 内存加固结果
#[derive(Debug)]
pub struct HardeningReport {
    pub core_dump_disabled: bool,
    pub dumpable_cleared: bool,
    pub mlock_limit_raised: bool,
}

/// 执行所有内存加固措施
///
/// 应在 main() 最开始调用, 在任何 Token 操作之前
pub fn harden_process_memory() -> HardeningReport {
    let mut report = HardeningReport {
        core_dump_disabled: false,
        dumpable_cleared: false,
        mlock_limit_raised: false,
    };

    // ── 1. 禁用 core dump (RLIMIT_CORE = 0) ──
    // 防止进程崩溃时将含 Token 的内存 dump 到磁盘
    #[cfg(target_os = "linux")]
    {
        let zero_limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        let ret = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &zero_limit) };
        if ret == 0 {
            report.core_dump_disabled = true;
            info!("🔒 core dump 已禁用 (RLIMIT_CORE=0)");
        } else {
            warn!("setrlimit(RLIMIT_CORE) 失败: {}", std::io::Error::last_os_error());
        }
    }

    // ── 2. PR_SET_DUMPABLE = 0 ──
    // 防止其他进程通过 /proc/pid/mem 读取本进程内存
    // 同时阻止 ptrace attach
    #[cfg(target_os = "linux")]
    {
        let ret = unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) };
        if ret == 0 {
            report.dumpable_cleared = true;
            info!("🔒 PR_SET_DUMPABLE=0 (禁止 /proc/pid/mem 读取)");
        } else {
            warn!("prctl(PR_SET_DUMPABLE) 失败: {}", std::io::Error::last_os_error());
        }
    }

    // ── 3. 尝试提高 mlock 限制 ──
    // 允许后续 mlock() 调用锁定更多内存页
    #[cfg(target_os = "linux")]
    {
        let mut current = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        let ret = unsafe { libc::getrlimit(libc::RLIMIT_MEMLOCK, &mut current) };
        if ret == 0 {
            // 尝试提高 soft limit 到 hard limit
            if current.rlim_cur < current.rlim_max {
                let raised = libc::rlimit {
                    rlim_cur: current.rlim_max,
                    rlim_max: current.rlim_max,
                };
                if unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &raised) } == 0 {
                    report.mlock_limit_raised = true;
                    info!(
                        limit_kb = current.rlim_max / 1024,
                        "🔒 RLIMIT_MEMLOCK 已提升至 hard limit"
                    );
                }
            } else {
                report.mlock_limit_raised = true; // 已经是最大值
            }
        }
    }

    // 非 Linux 平台: 跳过
    #[cfg(not(target_os = "linux"))]
    {
        warn!("内存加固仅支持 Linux, 当前平台跳过");
    }

    report
}

/// 锁定内存页 (防止 swap)
///
/// 对给定的字节切片调用 mlock(), 确保其不被换出到磁盘
/// 适用于 Token 等小量关键数据
#[cfg(target_os = "linux")]
pub fn mlock_bytes(data: &[u8]) -> bool {
    if data.is_empty() {
        return true;
    }
    let ret = unsafe {
        libc::mlock(data.as_ptr() as *const libc::c_void, data.len())
    };
    if ret != 0 {
        warn!(
            size = data.len(),
            error = %std::io::Error::last_os_error(),
            "mlock 失败 (可能需要提高 RLIMIT_MEMLOCK)"
        );
        false
    } else {
        true
    }
}

#[cfg(not(target_os = "linux"))]
pub fn mlock_bytes(_data: &[u8]) -> bool {
    false
}

/// 解锁内存页
#[cfg(target_os = "linux")]
pub fn munlock_bytes(data: &[u8]) -> bool {
    if data.is_empty() {
        return true;
    }
    unsafe {
        libc::munlock(data.as_ptr() as *const libc::c_void, data.len()) == 0
    }
}

#[cfg(not(target_os = "linux"))]
pub fn munlock_bytes(_data: &[u8]) -> bool {
    false
}

/// 标记内存区域为 MADV_DONTDUMP (不包含在 core dump 中)
#[cfg(target_os = "linux")]
#[allow(dead_code)]
pub fn mark_dontdump(data: &[u8]) -> bool {
    if data.is_empty() {
        return true;
    }
    // 需要页对齐
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;
    let addr = data.as_ptr() as usize;
    let aligned_addr = addr & !(page_size - 1);
    let len = (addr - aligned_addr) + data.len();
    let aligned_len = (len + page_size - 1) & !(page_size - 1);

    let ret = unsafe {
        libc::madvise(
            aligned_addr as *mut libc::c_void,
            aligned_len,
            libc::MADV_DONTDUMP,
        )
    };
    ret == 0
}

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
pub fn mark_dontdump(_data: &[u8]) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harden_process_memory_does_not_panic() {
        let report = harden_process_memory();
        // Linux 上应该都成功
        #[cfg(target_os = "linux")]
        {
            assert!(report.core_dump_disabled);
            assert!(report.dumpable_cleared);
        }
    }

    #[test]
    fn mlock_small_buffer() {
        let data = [0x42u8; 64];
        // mlock 可能因 RLIMIT_MEMLOCK 太小而失败, 不 assert
        let _ = mlock_bytes(&data);
        let _ = munlock_bytes(&data);
    }

    #[test]
    fn mlock_empty_buffer() {
        assert!(mlock_bytes(&[]));
        assert!(munlock_bytes(&[]));
    }

    #[test]
    fn mark_dontdump_small_buffer() {
        let data = vec![0u8; 4096]; // 页对齐大小
        let _ = mark_dontdump(&data);
    }
}
