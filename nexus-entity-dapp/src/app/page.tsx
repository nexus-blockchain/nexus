'use client';

import { useState } from 'react';
import { useRouter } from 'next/navigation';
import { useTranslations } from 'next-intl';
import { LocaleSwitcher } from '@/components/locale-switcher';

export default function HomePage() {
  const router = useRouter();
  const t = useTranslations('home');
  const [entityId, setEntityId] = useState('');

  const handleGo = () => {
    const id = entityId.trim();
    if (id) {
      router.push(`/${id}`);
    }
  };

  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-8 p-4">
      <div className="absolute top-4 right-4">
        <LocaleSwitcher />
      </div>

      <div className="text-center">
        <h1 className="text-4xl font-bold">{t('title')}</h1>
        <p className="mt-2 text-muted-foreground">{t('subtitle')}</p>
      </div>

      <div className="flex w-full max-w-sm gap-2">
        <input
          type="number"
          min="0"
          placeholder={t('placeholder')}
          value={entityId}
          onChange={(e) => setEntityId(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && handleGo()}
          className="flex-1 rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          aria-label="Entity ID"
        />
        <button
          onClick={handleGo}
          disabled={!entityId.trim()}
          className="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50 disabled:pointer-events-none"
        >
          {t('enter')}
        </button>
      </div>

      <div className="text-center text-sm text-muted-foreground">
        <p>{t('hint')}</p>
      </div>
    </main>
  );
}
