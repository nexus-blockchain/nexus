import type { Metadata } from "next";
import "@/styles/globals.css";
import { Sidebar } from "@/components/layout/Sidebar";
import { Header } from "@/components/layout/Header";
import { NextIntlClientProvider } from "next-intl";
import { getLocale, getMessages } from "next-intl/server";
import { rtlLocales, type Locale } from "@/i18n/config";

export const metadata: Metadata = {
  title: "NEXUS Entity Manager",
  description: "Manage your NEXUS entities, shops, tokens, and more",
};

export default async function RootLayout({ children }: { children: React.ReactNode }) {
  const locale = await getLocale();
  const messages = await getMessages();
  const dir = rtlLocales.includes(locale as Locale) ? "rtl" : "ltr";

  return (
    <html lang={locale} dir={dir}>
      <body className="min-h-screen antialiased">
        <NextIntlClientProvider locale={locale} messages={messages}>
          <div className="flex h-screen overflow-hidden">
            <Sidebar />
            <div className="flex flex-1 flex-col overflow-hidden">
              <Header />
              <main className="flex-1 overflow-y-auto p-6">{children}</main>
              <footer className="flex h-10 items-center justify-between border-t px-4 text-xs text-muted-foreground">
                <span>NEXUS Entity Manager v0.1.0</span>
                <span>Powered by Substrate</span>
              </footer>
            </div>
          </div>
        </NextIntlClientProvider>
      </body>
    </html>
  );
}
