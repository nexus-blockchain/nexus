'use client';

import { useEntityQuery } from './use-entity-query';
import { useEntityMutation } from './use-entity-mutation';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { STALE_TIMES } from '@/lib/chain/constants';
import { KycLevel, KycStatus } from '@/lib/types/enums';
import type { KycRecord } from '@/lib/types/models';

// ─── Parsers ────────────────────────────────────────────────

function parseKycEntries(rawEntries: [any, any][]): KycRecord[] {
  if (!rawEntries || !Array.isArray(rawEntries)) return [];
  return rawEntries.map(([key, value]) => {
    const obj = value?.toJSON?.() ?? value;
    return {
      entityId: Number(key.args?.[0]?.toString() ?? obj.entityId ?? obj.entity_id ?? 0),
      account: String(key.args?.[1]?.toString() ?? obj.account ?? ''),
      level: Number(obj.level ?? 0) as KycLevel,
      status: String(obj.status ?? 'NotSubmitted') as KycStatus,
      dataCid: String(obj.dataCid ?? obj.data_cid ?? ''),
      countryCode: obj.countryCode ?? obj.country_code ?? null,
      riskScore: Number(obj.riskScore ?? obj.risk_score ?? 0),
      submittedAt: Number(obj.submittedAt ?? obj.submitted_at ?? 0),
      expiresAt: obj.expiresAt ?? obj.expires_at ?? null,
    };
  });
}

function parseEntityRequirement(raw: unknown): { minLevel: KycLevel; maxRiskScore: number } {
  if (!raw || (raw as { isNone?: boolean }).isNone) return { minLevel: KycLevel.None, maxRiskScore: 100 };
  const unwrapped = (raw as { unwrapOr?: (d: null) => unknown }).unwrapOr?.(null) ?? raw;
  if (!unwrapped) return { minLevel: KycLevel.None, maxRiskScore: 100 };
  const obj = (unwrapped as any).toJSON?.() ?? unwrapped;
  return {
    minLevel: Number(obj.minLevel ?? obj.min_level ?? 0) as KycLevel,
    maxRiskScore: Number(obj.maxRiskScore ?? obj.max_risk_score ?? 100),
  };
}

// ─── Hook ───────────────────────────────────────────────────

export function useKyc() {
  const { entityId } = useEntityContext();

  const invalidateKeys = [['entity', entityId, 'kyc']];

  // ─── Queries ──────────────────────────────────────────

  const kycRecordsQuery = useEntityQuery<KycRecord[]>(
    ['entity', entityId, 'kyc'],
    async (api) => {
      const raw = await (api.query as any).entityKyc.kycRecords.entries(entityId);
      return parseKycEntries(raw);
    },
    { staleTime: STALE_TIMES.members },
  );

  const requirementQuery = useEntityQuery<{ minLevel: KycLevel; maxRiskScore: number }>(
    ['entity', entityId, 'kyc', 'requirement'],
    async (api) => {
      const raw = await (api.query as any).entityKyc.entityRequirement(entityId);
      return parseEntityRequirement(raw);
    },
    { staleTime: STALE_TIMES.members },
  );

  // ─── Mutations ──────────────────────────────────────────

  const submitKyc = useEntityMutation('entityKyc', 'submitKyc', { invalidateKeys });
  const updateKycData = useEntityMutation('entityKyc', 'updateKycData', { invalidateKeys });
  const purgeKycData = useEntityMutation('entityKyc', 'purgeKycData', { invalidateKeys });
  const approveKyc = useEntityMutation('entityKyc', 'approveKyc', { invalidateKeys });
  const rejectKyc = useEntityMutation('entityKyc', 'rejectKyc', { invalidateKeys });
  const revokeKyc = useEntityMutation('entityKyc', 'revokeKyc', { invalidateKeys });
  const setEntityRequirement = useEntityMutation('entityKyc', 'setEntityRequirement', { invalidateKeys });

  return {
    // Query data
    kycRecords: kycRecordsQuery.data ?? [],
    requirement: requirementQuery.data ?? { minLevel: KycLevel.None, maxRiskScore: 100 },
    isLoading: kycRecordsQuery.isLoading,
    error: kycRecordsQuery.error,
    // Mutations
    submitKyc,
    updateKycData,
    purgeKycData,
    approveKyc,
    rejectKyc,
    revokeKyc,
    setEntityRequirement,
  };
}
