'use client';

import { useLocale } from '@/app/providers';
import { locales, type Locale } from '@/i18n/config';

const LOCALE_LABELS: Record<Locale, string> = {
  zh: '中文',
  en: 'EN',
};

export function LocaleSwitcher() {
  const { locale, setLocale } = useLocale();

  return (
    <div className="flex items-center gap-1 rounded-md border border-input bg-background p-0.5 text-sm">
      {locales.map((l) => (
        <button
          key={l}
          onClick={() => setLocale(l)}
          className={`rounded px-2 py-1 transition-colors ${
            locale === l
              ? 'bg-primary text-primary-foreground'
              : 'text-muted-foreground hover:text-foreground'
          }`}
          aria-label={`Switch to ${LOCALE_LABELS[l]}`}
          aria-pressed={locale === l}
        >
          {LOCALE_LABELS[l]}
        </button>
      ))}
    </div>
  );
}
