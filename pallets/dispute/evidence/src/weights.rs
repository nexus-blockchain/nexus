//! 权重实现（临时手写版）。
//! 说明：后续可用 benchmark 自动生成替换此文件。

use core::marker::PhantomData;
use frame_support::{
    traits::Get,
    weights::{constants::RocksDbWeight, Weight},
};

/// 函数级中文注释：定义 evidence Pallet 所需权重函数接口。
pub trait WeightInfo {
    /// 提交明文 CID 列表，权重与三类媒体数量线性相关
    fn commit(n_imgs: u32, n_vids: u32, n_docs: u32) -> Weight;
    /// 提交承诺哈希（常数级）
    fn commit_hash() -> Weight;
    /// 链接既有证据（常数级）
    fn link() -> Weight;
    /// 命名空间链接既有证据（常数级）
    fn link_by_ns() -> Weight;
    /// 取消链接（常数级）
    fn unlink() -> Weight;
    /// 命名空间取消链接（常数级）
    fn unlink_by_ns() -> Weight;
    /// 🆕 M3: 密封证据 (call_index 16)
    fn seal_evidence() -> Weight;
    /// 🆕 M3: 解封证据 (call_index 17)
    fn unseal_evidence() -> Weight;
    /// 🆕 M3: 撤回证据 (call_index 19)
    fn withdraw_evidence() -> Weight;
    /// 🆕 M3: 揭示承诺 (call_index 15)
    fn reveal_commitment() -> Weight;
    /// 🆕 VC2: 注册用户公钥 (call_index 6)
    fn register_public_key() -> Weight;
    /// 🆕 VC2: 存储私密内容 (call_index 7)
    fn store_private_content() -> Weight;
    /// 🆕 VC2: 授予访问权限 (call_index 8)
    fn grant_access() -> Weight;
    /// 🆕 VC2: 撤销访问权限 (call_index 9)
    fn revoke_access() -> Weight;
    /// 🆕 VC2: 轮换加密密钥 (call_index 10)
    fn rotate_content_keys() -> Weight;
    /// 🆕 VC3: 请求访问加密内容 (call_index 13)
    fn request_access() -> Weight;
    /// 🆕 VC3: 更新访问策略 (call_index 14)
    fn update_access_policy() -> Weight;
    /// 🆕 M5-R3: 强制移除证据 (call_index 18)
    fn force_remove_evidence() -> Weight;
    /// 🆕 M5-R3: 强制归档证据 (call_index 21)
    fn force_archive_evidence() -> Weight;
    /// 🆕 M5-R3: 删除私密内容 (call_index 20)
    fn delete_private_content() -> Weight;
}

/// 函数级中文注释：参照 Substrate 推荐的 RocksDb 权重，提供通用实现。
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn commit(n_imgs: u32, n_vids: u32, n_docs: u32) -> Weight {
        // 🆕 M3修复: 补充缺失的 reads 和 proof_size
        // 读：AccountWindows(r), EvidenceCountByTarget(r), CidHashIndex(r)*N = 2+N reads
        // 写：NextEvidenceId(w), Evidences(w), EvidenceByTarget(w), AccountWindows(w),
        //     EvidenceCountByTarget(w), CidHashIndex(w)*N = 5+N writes
        let per_cid_cost = 2_000_000u64;
        let n_total = n_imgs.saturating_add(n_vids).saturating_add(n_docs);
        Weight::from_parts(8_000_000, 3_500)
            .saturating_add(
                Weight::from_parts(per_cid_cost, 0).saturating_mul(n_total as u64),
            )
            .saturating_add(T::DbWeight::get().reads(2_u64.saturating_add(n_total as u64)))
            .saturating_add(T::DbWeight::get().writes(5_u64.saturating_add(n_total as u64)))
    }
    fn commit_hash() -> Weight {
        // 🆕 M3修复: 补充 proof_size 和缺失的 reads
        // 读：CommitIndex(r), AccountWindows(r), EvidenceCountByNs(r) = 3 reads
        // 写：NextEvidenceId(w), Evidences(w), EvidenceByNs(w), CommitIndex(w),
        //     AccountWindows(w), EvidenceCountByNs(w) = 6 writes
        Weight::from_parts(6_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(6_u64))
    }
    fn link() -> Weight {
        // 🆕 M1-R4修复: 读：Evidences(r), SealedEvidences(r), EvidenceStatuses(r) = 3 reads
        // 写：EvidenceByTarget(w) = 1 write
        Weight::from_parts(4_000_000, 2_500)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    fn link_by_ns() -> Weight {
        // 🆕 M1-R4修复: 读：Evidences(r), SealedEvidences(r), EvidenceStatuses(r) = 3 reads
        // 写：EvidenceByNs(w) = 1 write
        Weight::from_parts(4_000_000, 2_500)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    fn unlink() -> Weight {
        // 读：Evidences(r), SealedEvidences(r)；写：EvidenceByTarget(w)
        Weight::from_parts(4_000_000, 2_000)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    fn unlink_by_ns() -> Weight {
        // 读：Evidences(r), SealedEvidences(r)；写：EvidenceByNs(w)
        Weight::from_parts(4_000_000, 2_000)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    fn seal_evidence() -> Weight {
        // 读：Evidences(r), EvidenceStatuses(r), SealedEvidences(r) = 3 reads
        // 写：SealedEvidences(w), EvidenceStatuses(w) = 2 writes
        Weight::from_parts(30_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(2_u64))
    }
    fn unseal_evidence() -> Weight {
        // 读：Evidences(r), SealedEvidences(r) = 2 reads
        // 写：SealedEvidences(w), EvidenceStatuses(w) = 2 writes
        Weight::from_parts(30_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(2_u64))
    }
    fn withdraw_evidence() -> Weight {
        // 读：Evidences(r), EvidenceStatuses(r), SealedEvidences(r) = 3 reads
        // 写：EvidenceStatuses(w) = 1 write
        Weight::from_parts(30_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    fn reveal_commitment() -> Weight {
        // 读：Evidences(r), EvidenceStatuses(r), SealedEvidences(r), CidHashIndex(r) = 4 reads
        // 写：Evidences(w), CommitIndex(w), CidHashIndex(w) = 3 writes
        Weight::from_parts(50_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(4_u64))
            .saturating_add(T::DbWeight::get().writes(3_u64))
    }
    fn register_public_key() -> Weight {
        // 读：0；写：UserPublicKeys(w) = 1 write
        Weight::from_parts(40_000_000, 4_000)
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    fn store_private_content() -> Weight {
        // 读：PrivateContentByCid(r), UserPublicKeys(r) * N, NextPrivateContentId(r) = ~5 reads
        // 🆕 M1-R5修复: 写：PrivateContents(w), PrivateContentByCid(w), PrivateContentBySubject(w), NextPrivateContentId(w) = 4 writes
        Weight::from_parts(80_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(5_u64))
            .saturating_add(T::DbWeight::get().writes(4_u64))
    }
    fn grant_access() -> Weight {
        // 读：PrivateContents(r), UserPublicKeys(r) = 2 reads
        // 🆕 M2-R5修复: 写：PrivateContents(w), AccessRequests::remove(w) = 2 writes
        Weight::from_parts(50_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(2_u64))
    }
    fn revoke_access() -> Weight {
        // 读：PrivateContents(r) = 1 read；写：PrivateContents(w) = 1 write
        Weight::from_parts(45_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(1_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    fn rotate_content_keys() -> Weight {
        // 🆕 M4-R4修复: 読：PrivateContents(r), UserPublicKeys(r) * N, KeyRotationCounter(r) = ~8 reads
        // 写：PrivateContents(w), KeyRotationHistory(w), KeyRotationCounter(w) = 3 writes
        Weight::from_parts(70_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(8_u64))
            .saturating_add(T::DbWeight::get().writes(3_u64))
    }
    fn request_access() -> Weight {
        // 读：UserPublicKeys(r), PrivateContents(r), AccessRequests(r) = 3 reads
        // 写：AccessRequests(w) = 1 write
        Weight::from_parts(40_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    fn update_access_policy() -> Weight {
        // 读：PrivateContents(r) = 1 read；写：PrivateContents(w) = 1 write
        Weight::from_parts(45_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(1_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    fn force_remove_evidence() -> Weight {
        // 读：Evidences(r), EvidenceParent(r), EvidenceChildren(r) = 3 reads
        // 写：EvidenceByTarget(w), EvidenceByNs(w), EvidenceCountByNs(w),
        //     EvidenceCountByTarget(w), CidHashIndex(w), CommitIndex(w),
        //     SealedEvidences(w), EvidenceParent(w), EvidenceChildren(w)*2,
        //     PendingManifests(w), Evidences(w), EvidenceStatuses(w) ≈ 13 writes
        Weight::from_parts(80_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(13_u64))
    }
    fn force_archive_evidence() -> Weight {
        // 🆕 M3-R5修复: 读：Evidences(r), EvidenceParent(r), EvidenceChildren(r) = 3 reads
        // 写：ArchivedEvidences(w), EvidenceByTarget(w), EvidenceByNs(w),
        //     EvidenceCountByNs(w), EvidenceCountByTarget(w), CidHashIndex(w),
        //     CommitIndex(w), EvidenceParent(w), EvidenceChildren(w),
        //     SealedEvidences(w), EvidenceStatuses(w), PendingManifests(w),
        //     Evidences(w), ArchiveStats(w) ≈ 14 writes
        Weight::from_parts(80_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(14_u64))
    }
    fn delete_private_content() -> Weight {
        // 读：PrivateContents(r), KeyRotationCounter(r) = 2 reads
        // 写：PrivateContentByCid(w), PrivateContentBySubject(w),
        //     AccessRequests prefix clear(w), KeyRotationHistory(w)*N,
        //     KeyRotationCounter(w), PrivateContents(w) ≈ 5+N writes
        // 假设平均 N=5 次轮换
        Weight::from_parts(70_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(10_u64))
    }
}

/// 函数级中文注释：为测试与未接线场景提供默认实现（基于 RocksDbWeight）。
impl WeightInfo for () {
    fn commit(n_imgs: u32, n_vids: u32, n_docs: u32) -> Weight {
        let per_cid_cost = 2_000_000u64;
        let n_total = n_imgs as u64 + n_vids as u64 + n_docs as u64;
        Weight::from_parts(8_000_000, 3_500)
            .saturating_add(Weight::from_parts(per_cid_cost, 0).saturating_mul(n_total))
            .saturating_add(RocksDbWeight::get().reads(2_u64.saturating_add(n_total)))
            .saturating_add(RocksDbWeight::get().writes(5_u64.saturating_add(n_total)))
    }
    fn commit_hash() -> Weight {
        Weight::from_parts(6_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(6_u64))
    }
    fn link() -> Weight {
        Weight::from_parts(4_000_000, 2_500)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn link_by_ns() -> Weight {
        Weight::from_parts(4_000_000, 2_500)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn unlink() -> Weight {
        Weight::from_parts(4_000_000, 2_000)
            .saturating_add(RocksDbWeight::get().reads(2_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn unlink_by_ns() -> Weight {
        Weight::from_parts(4_000_000, 2_000)
            .saturating_add(RocksDbWeight::get().reads(2_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn seal_evidence() -> Weight {
        Weight::from_parts(30_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(2_u64))
    }
    fn unseal_evidence() -> Weight {
        Weight::from_parts(30_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(2_u64))
            .saturating_add(RocksDbWeight::get().writes(2_u64))
    }
    fn withdraw_evidence() -> Weight {
        Weight::from_parts(30_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn reveal_commitment() -> Weight {
        Weight::from_parts(50_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(4_u64))
            .saturating_add(RocksDbWeight::get().writes(3_u64))
    }
    fn register_public_key() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn store_private_content() -> Weight {
        // 🆕 M1-R5修复: writes 3→4 (PrivateContentBySubject)
        Weight::from_parts(80_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(5_u64))
            .saturating_add(RocksDbWeight::get().writes(4_u64))
    }
    fn grant_access() -> Weight {
        // 🆕 M2-R5修复: writes 1→2 (AccessRequests::remove)
        Weight::from_parts(50_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(2_u64))
            .saturating_add(RocksDbWeight::get().writes(2_u64))
    }
    fn revoke_access() -> Weight {
        Weight::from_parts(45_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(1_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn rotate_content_keys() -> Weight {
        Weight::from_parts(70_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(8_u64))
            .saturating_add(RocksDbWeight::get().writes(3_u64))
    }
    fn request_access() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn update_access_policy() -> Weight {
        Weight::from_parts(45_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(1_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn force_remove_evidence() -> Weight {
        Weight::from_parts(80_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(13_u64))
    }
    fn force_archive_evidence() -> Weight {
        // 🆕 M3-R5修复: reads 2→3 (EvidenceChildren)
        Weight::from_parts(80_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(14_u64))
    }
    fn delete_private_content() -> Weight {
        Weight::from_parts(70_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(2_u64))
            .saturating_add(RocksDbWeight::get().writes(10_u64))
    }
}
