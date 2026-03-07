'use client';

import { useEntityQuery } from './use-entity-query';
import { useEntityMutation } from './use-entity-mutation';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { STALE_TIMES } from '@/lib/chain/constants';
import { MemberStatus } from '@/lib/types/enums';
import type { MemberData, CustomLevel } from '@/lib/types/models';
import type { UpgradeTrigger } from '@/lib/types/enums';

// ─── Parsers ────────────────────────────────────────────────

function parseMemberEntries(rawEntries: [any, any][]): MemberData[] {
  if (!rawEntries || !Array.isArray(rawEntries)) return [];
  return rawEntries.map(([key, value]) => {
    const obj = value?.toJSON?.() ?? value;
    return {
      entityId: Number(key.args?.[0]?.toString() ?? obj.entityId ?? 0),
      account: String(key.args?.[1]?.toString() ?? obj.account ?? ''),
      status: String(obj.status ?? 'Active') as MemberStatus,
      level: Number(obj.level ?? 0),
      referrer: obj.referrer ? String(obj.referrer) : null,
      joinedAt: Number(obj.joinedAt ?? obj.joined_at ?? 0),
      totalSpent: BigInt(String(obj.totalSpent ?? obj.total_spent ?? 0)),
      orderCount: Number(obj.orderCount ?? obj.order_count ?? 0),
    };
  });
}

function parsePendingMembers(rawEntries: [any, any][]): string[] {
  if (!rawEntries || !Array.isArray(rawEntries)) return [];
  return rawEntries.map(([key]) => String(key.args?.[1]?.toString() ?? ''));
}

function parseCustomLevels(rawEntries: [any, any][]): CustomLevel[] {
  if (!rawEntries || !Array.isArray(rawEntries)) return [];
  return rawEntries.map(([, value]) => {
    const obj = value?.toJSON?.() ?? value;
    return {
      name: String(obj.name ?? ''),
      threshold: BigInt(String(obj.threshold ?? 0)),
      discountRate: Number(obj.discountRate ?? obj.discount_rate ?? 0),
      commissionBonus: Number(obj.commissionBonus ?? obj.commission_bonus ?? 0),
    };
  });
}

function parseUpgradeTriggers(rawEntries: [any, any][]): { trigger: UpgradeTrigger; value: bigint }[] {
  if (!rawEntries || !Array.isArray(rawEntries)) return [];
  return rawEntries.map(([key, value]) => {
    const trigger = String(key.args?.[1]?.toString() ?? '') as UpgradeTrigger;
    const val = (value as any)?.toJSON?.() ?? value;
    return { trigger, value: BigInt(String(val ?? 0)) };
  });
}

// ─── Hook ───────────────────────────────────────────────────

export function useMembers() {
  const { entityId } = useEntityContext();

  const invalidateKeys = [['entity', entityId, 'members']];

  // ─── Queries ──────────────────────────────────────────

  const membersQuery = useEntityQuery<MemberData[]>(
    ['entity', entityId, 'members'],
    async (api) => {
      const raw = await (api.query as any).entityMember.members.entries(entityId);
      return parseMemberEntries(raw);
    },
    { staleTime: STALE_TIMES.members },
  );

  const memberCountQuery = useEntityQuery<number>(
    ['entity', entityId, 'members', 'count'],
    async (api) => {
      const raw = await (api.query as any).entityMember.memberCount(entityId);
      return Number(raw?.toString() ?? 0);
    },
    { staleTime: STALE_TIMES.members },
  );

  const policyQuery = useEntityQuery<number>(
    ['entity', entityId, 'members', 'policy'],
    async (api) => {
      const raw = await (api.query as any).entityMember.registrationPolicy(entityId);
      return Number(raw?.toString() ?? 0);
    },
    { staleTime: STALE_TIMES.members },
  );

  const pendingQuery = useEntityQuery<string[]>(
    ['entity', entityId, 'members', 'pending'],
    async (api) => {
      const raw = await (api.query as any).entityMember.pendingMembers.entries(entityId);
      return parsePendingMembers(raw);
    },
    { staleTime: STALE_TIMES.members },
  );

  // Referral tree query factory
  const useReferralTree = (account: string | null) =>
    useEntityQuery<{ directReferrals: string[]; teamSize: number }>(
      ['entity', entityId, 'members', 'referral', account],
      async (api) => {
        if (!account) return { directReferrals: [], teamSize: 0 };
        const raw = await (api.query as any).entityMember.referralTree(entityId, account);
        const obj = raw?.toJSON?.() ?? raw;
        return {
          directReferrals: Array.isArray(obj?.directReferrals ?? obj?.direct_referrals)
            ? (obj.directReferrals ?? obj.direct_referrals).map(String)
            : [],
          teamSize: Number(obj?.teamSize ?? obj?.team_size ?? 0),
        };
      },
      { staleTime: STALE_TIMES.members, enabled: !!account },
    );

  // Custom levels query
  const customLevelsQuery = useEntityQuery<CustomLevel[]>(
    ['entity', entityId, 'members', 'levels'],
    async (api) => {
      const raw = await (api.query as any).entityMember.customLevels.entries(entityId);
      return parseCustomLevels(raw);
    },
    { staleTime: STALE_TIMES.members },
  );

  // Upgrade triggers query
  const upgradeTriggersQuery = useEntityQuery<{ trigger: UpgradeTrigger; value: bigint }[]>(
    ['entity', entityId, 'members', 'triggers'],
    async (api) => {
      const raw = await (api.query as any).entityMember.upgradeTriggers.entries(entityId);
      return parseUpgradeTriggers(raw);
    },
    { staleTime: STALE_TIMES.members },
  );

  // ─── Mutations ──────────────────────────────────────────

  const registerMember = useEntityMutation('entityMember', 'registerMember', { invalidateKeys });
  const freezeMember = useEntityMutation('entityMember', 'freezeMember', { invalidateKeys });
  const unfreezeMember = useEntityMutation('entityMember', 'unfreezeMember', { invalidateKeys });
  const banMember = useEntityMutation('entityMember', 'banMember', {
    invalidateKeys,
    confirmDialog: { title: '确认封禁', description: '封禁后该会员将无法参与任何实体活动，确定继续？', severity: 'danger' },
  });
  const unbanMember = useEntityMutation('entityMember', 'unbanMember', { invalidateKeys });
  const removeMember = useEntityMutation('entityMember', 'removeMember', {
    invalidateKeys,
    confirmDialog: { title: '确认移除', description: '移除会员不可撤销，确定继续？', severity: 'danger' },
  });
  const approveMember = useEntityMutation('entityMember', 'approveMember', { invalidateKeys });
  const rejectMember = useEntityMutation('entityMember', 'rejectMember', { invalidateKeys });
  const setRegistrationPolicy = useEntityMutation('entityMember', 'setRegistrationPolicy', { invalidateKeys });

  // Level mutations
  const initializeLevels = useEntityMutation('entityMember', 'initializeLevels', { invalidateKeys });
  const addCustomLevel = useEntityMutation('entityMember', 'addCustomLevel', { invalidateKeys });
  const updateCustomLevel = useEntityMutation('entityMember', 'updateCustomLevel', { invalidateKeys });
  const deleteCustomLevel = useEntityMutation('entityMember', 'deleteCustomLevel', { invalidateKeys });
  const setUpgradeTrigger = useEntityMutation('entityMember', 'setUpgradeTrigger', { invalidateKeys });

  return {
    // Query data
    members: membersQuery.data ?? [],
    memberCount: memberCountQuery.data ?? 0,
    policy: policyQuery.data ?? 0,
    pendingMembers: pendingQuery.data ?? [],
    customLevels: customLevelsQuery.data ?? [],
    upgradeTriggers: upgradeTriggersQuery.data ?? [],
    isLoading: membersQuery.isLoading || memberCountQuery.isLoading,
    error: membersQuery.error ?? memberCountQuery.error,
    // Referral
    useReferralTree,
    // Member mutations
    registerMember,
    freezeMember,
    unfreezeMember,
    banMember,
    unbanMember,
    removeMember,
    approveMember,
    rejectMember,
    setRegistrationPolicy,
    // Level mutations
    initializeLevels,
    addCustomLevel,
    updateCustomLevel,
    deleteCustomLevel,
    setUpgradeTrigger,
  };
}
