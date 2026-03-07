export const locales = ['zh', 'en'] as const;
export type Locale = (typeof locales)[number];

export const defaultLocale: Locale = 'zh';

export async function getMessages(locale: Locale = defaultLocale) {
  return (await import(`../../messages/${locale}.json`)).default;
}
