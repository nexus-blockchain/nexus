'use client';

import { useTranslations } from 'next-intl';
import { EntityStatus } from '@/lib/types/enums';

interface RestrictedProps {
  entityId: number;
  status: EntityStatus;
}

export function EntityRestricted({ entityId, status }: RestrictedProps) {
  const t = useTranslations('entity');
  const isBanned = status === EntityStatus.Banned;

  return (
    <div className="flex min-h-screen flex-col items-center justify-center gap-4 p-4">
      <div className="rounded-lg border border-red-200 bg-red-50 p-8 text-center">
        <h1 className="text-2xl font-bold text-red-700">
          {isBanned ? t('banned') : t('suspended')}
        </h1>
        <p className="mt-2 text-red-600">
          {isBanned
            ? t('bannedDescription', { entityId })
            : t('suspendedDescription', { entityId })}
        </p>
        <p className="mt-4 text-sm text-gray-500">
          {t('contactAdmin')}
        </p>
      </div>
    </div>
  );
}
