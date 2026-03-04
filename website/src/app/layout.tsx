import type { Metadata } from "next";
import { Inter } from "next/font/google";
import "./globals.css";
import { Navbar } from "@/components/layout/Navbar";
import { Footer } from "@/components/layout/Footer";
import { NextIntlClientProvider } from "next-intl";
import { getLocale, getMessages } from "next-intl/server";
import { rtlLocales, type Locale } from "@/i18n/config";
import { StarfieldBackground } from "@/components/ui/StarfieldBackground";
import { ThemeProvider } from "@/components/ui/ThemeProvider";

const inter = Inter({ subsets: ["latin"] });

export const metadata: Metadata = {
  title: "NEXUS — Entity Economy Tokenization · Community Marketing · AI Empowerment",
  description:
    "Next-gen business infrastructure — create on-chain entities, drive growth with community networks, let AI operate for you. Layer-1 blockchain built on Substrate.",
  keywords: [
    "NEXUS",
    "blockchain",
    "entity economy",
    "tokenization",
    "community marketing",
    "AI",
    "Web3",
    "Substrate",
    "DAO",
  ],
};

export default async function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const locale = await getLocale();
  const messages = await getMessages();
  const dir = rtlLocales.includes(locale as Locale) ? "rtl" : "ltr";

  return (
    <html lang={locale} dir={dir} className="dark" suppressHydrationWarning>
      <head>
        <script
          dangerouslySetInnerHTML={{
            __html: `(function(){try{var t=localStorage.getItem('nexus-theme');if(t==='light'){document.documentElement.classList.remove('dark');document.documentElement.classList.add('light')}else if(!t&&window.matchMedia('(prefers-color-scheme:light)').matches){document.documentElement.classList.remove('dark');document.documentElement.classList.add('light')}}catch(e){}})()`,
          }}
        />
      </head>
      <body className={`${inter.className} antialiased`}>
        <NextIntlClientProvider locale={locale} messages={messages}>
          <ThemeProvider>
            <StarfieldBackground />
            <div className="relative z-10">
              <Navbar />
              <main className="min-h-screen">{children}</main>
              <Footer />
            </div>
          </ThemeProvider>
        </NextIntlClientProvider>
      </body>
    </html>
  );
}
