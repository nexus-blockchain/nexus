'use client';

import { useTranslations } from 'next-intl';
import { EntityProvider, useEntityContext } from './entity-provider';
import { useApi } from '@/lib/chain/api-provider';
import { computeRenderMode } from '@/lib/utils/entity-status';
import { EntityNotFound } from './not-found';
import { EntityRestricted } from './restricted';
import { EntitySidebar } from '@/components/sidebar/entity-sidebar';

function EntityLayoutInner({ children }: { children: React.ReactNode }) {
  const { entityId, entity, isLoading, error, isReadOnly } = useEntityContext();
  const { connectionStatus } = useApi();
  const t = useTranslations('layout');

  if (connectionStatus !== 'connected' && !entity) {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <div className="flex flex-col items-center gap-3">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-gray-200 border-t-blue-600" />
          <p className="text-sm text-gray-500">
            {connectionStatus === 'connecting'
              ? t('connecting')
              : connectionStatus === 'error'
                ? t('connectFailed')
                : t('waiting')}
          </p>
          <p className="text-xs text-gray-400">
            {t('endpoint')}: {process.env.NEXT_PUBLIC_WS_ENDPOINT || 'ws://127.0.0.1:9944'}
          </p>
        </div>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <div className="flex flex-col items-center gap-3">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-gray-200 border-t-blue-600" />
          <p className="text-sm text-gray-500">{t('loadingEntity')}</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex min-h-screen items-center justify-center p-4">
        <div className="rounded-lg border border-red-200 bg-red-50 p-6 text-center">
          <p className="text-red-600">{t('loadFailed')}: {error}</p>
        </div>
      </div>
    );
  }

  const renderMode = computeRenderMode(entity?.status ?? null);

  if (renderMode === 'not_found') {
    return <EntityNotFound entityId={entityId} />;
  }

  if (renderMode === 'restricted') {
    return <EntityRestricted entityId={entityId} status={entity!.status} />;
  }

  return (
    <div className="min-h-screen">
      {isReadOnly && (
        <div className="bg-yellow-50 border-b border-yellow-200 px-4 py-2 text-center text-sm text-yellow-700">
          {t('readOnlyNotice')}
        </div>
      )}
      <div className="flex min-h-screen">
        <EntitySidebar />
        <main className="flex-1 pb-16 md:pb-0">{children}</main>
      </div>
    </div>
  );
}

export default function EntityLayout({
  params,
  children,
}: {
  params: { entityId: string };
  children: React.ReactNode;
}) {
  const entityId = Number(params.entityId);

  if (Number.isNaN(entityId) || entityId <= 0 || !Number.isInteger(entityId)) {
    return <EntityNotFound entityId={entityId} />;
  }

  return (
    <EntityProvider entityId={entityId}>
      <EntityLayoutInner>{children}</EntityLayoutInner>
    </EntityProvider>
  );
}
