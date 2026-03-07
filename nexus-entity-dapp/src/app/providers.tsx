'use client';

import { useState, useEffect } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { NextIntlClientProvider } from 'next-intl';
import { ApiProvider } from '@/lib/chain/api-provider';
import { defaultLocale, type Locale, locales } from '@/i18n/config';

function getStoredLocale(): Locale {
  if (typeof window === 'undefined') return defaultLocale;
  const stored = localStorage.getItem('locale');
  if (stored && locales.includes(stored as Locale)) return stored as Locale;
  return defaultLocale;
}

export function Providers({ children }: { children: React.ReactNode }) {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            refetchOnWindowFocus: false,
            retry: 3,
            retryDelay: (attempt) => Math.min(1000 * 2 ** attempt, 10000),
          },
        },
      }),
  );

  const [locale, setLocale] = useState<Locale>(defaultLocale);
  const [messages, setMessages] = useState<Record<string, unknown> | null>(null);

  useEffect(() => {
    const initial = getStoredLocale();
    setLocale(initial);
    import(`../../messages/${initial}.json`).then((m) => setMessages(m.default));
  }, []);

  useEffect(() => {
    import(`../../messages/${locale}.json`).then((m) => setMessages(m.default));
    localStorage.setItem('locale', locale);
    document.documentElement.lang = locale;
  }, [locale]);

  if (!messages) return null;

  return (
    <QueryClientProvider client={queryClient}>
      <ApiProvider>
        <NextIntlClientProvider locale={locale} messages={messages}>
          <LocaleContext.Provider value={{ locale, setLocale }}>
            {children}
          </LocaleContext.Provider>
        </NextIntlClientProvider>
      </ApiProvider>
    </QueryClientProvider>
  );
}

// ─── Locale switching context ───────────────────────────────

import { createContext, useContext } from 'react';

interface LocaleContextValue {
  locale: Locale;
  setLocale: (locale: Locale) => void;
}

const LocaleContext = createContext<LocaleContextValue>({
  locale: defaultLocale,
  setLocale: () => {},
});

export function useLocale() {
  return useContext(LocaleContext);
}
