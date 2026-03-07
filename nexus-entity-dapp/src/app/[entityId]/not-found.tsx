'use client';

import Link from 'next/link';
import { useTranslations } from 'next-intl';

/** Named export for direct use with entityId prop */
export function EntityNotFound({ entityId }: { entityId: number }) {
  const t = useTranslations('entity');
  return (
    <div className="flex min-h-screen flex-col items-center justify-center gap-4 p-4">
      <h1 className="text-4xl font-bold">404</h1>
      <p className="text-lg text-gray-600">
        {t('notFoundWithId', { entityId })}
      </p>
      <Link
        href="/"
        className="rounded-md bg-blue-600 px-4 py-2 text-white hover:bg-blue-700"
      >
        {t('backToHome')}
      </Link>
    </div>
  );
}

/** Default export required by Next.js not-found convention */
export default function NotFound() {
  const t = useTranslations('entity');
  return (
    <div className="flex min-h-screen flex-col items-center justify-center gap-4 p-4">
      <h1 className="text-4xl font-bold">404</h1>
      <p className="text-lg text-gray-600">{t('notFound')}</p>
      <Link
        href="/"
        className="rounded-md bg-blue-600 px-4 py-2 text-white hover:bg-blue-700"
      >
        {t('backToHome')}
      </Link>
    </div>
  );
}
