"use client";

import Link from "next/link";
import { useTranslations } from "next-intl";

export function Footer() {
  const t = useTranslations("footer");
  const tNav = useTranslations("nav");

  const footerSections = [
    {
      title: t("platform"),
      links: [
        { label: tNav("tokenize"), href: "/tokenize" },
        { label: tNav("growth"), href: "/growth" },
        { label: tNav("ai"), href: "/ai" },
        { label: tNav("stories"), href: "/stories" },
      ],
    },
    {
      title: t("developers"),
      links: [
        { label: tNav("tech"), href: "/tech" },
        { label: t("apiDocs"), href: "#" },
        { label: "GitHub", href: "https://github.com/nexus-chain" },
        { label: t("auditReports"), href: "#" },
      ],
    },
    {
      title: t("community"),
      links: [
        { label: "Telegram", href: "#" },
        { label: "Discord", href: "#" },
        { label: "Twitter", href: "#" },
        { label: "Medium", href: "#" },
      ],
    },
    {
      title: t("about"),
      links: [
        { label: t("whitepaper"), href: "#" },
        { label: t("roadmap"), href: "/join#roadmap" },
        { label: t("team"), href: "#" },
        { label: t("contactUs"), href: "#" },
      ],
    },
  ];

  return (
    <footer className="border-t border-theme bg-footer">
      <div className="container-wide px-4 py-16 sm:px-6 lg:px-8">
        <div className="grid gap-8 md:grid-cols-2 lg:grid-cols-5">
          {/* Brand */}
          <div className="lg:col-span-1">
            <Link href="/" className="flex items-center gap-2">
              <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-gradient-to-br from-blue-500 via-purple-500 to-emerald-500">
                <span className="text-sm font-bold text-white">N</span>
              </div>
              <span className="text-lg font-bold">NEXUS</span>
            </Link>
            <p className="mt-4 text-sm leading-relaxed t-muted">
              {t("brand1")}
              <br />
              {t("brand2")}
              <br />
              {t("brand3")}
            </p>
          </div>

          {/* Link columns */}
          {footerSections.map((section) => (
            <div key={section.title}>
              <h3 className="mb-4 text-sm font-semibold t-secondary">
                {section.title}
              </h3>
              <ul className="space-y-2.5">
                {section.links.map((link) => (
                  <li key={link.label}>
                    <Link
                      href={link.href}
                      className="text-sm t-faint transition-colors hover:t-secondary"
                    >
                      {link.label}
                    </Link>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>

        <div className="mt-12 flex flex-col items-center justify-between gap-4 border-t border-theme pt-8 sm:flex-row">
          <p className="text-xs t-faint">
            © {new Date().getFullYear()} NEXUS. Built on Substrate. MIT-0 License.
          </p>
          <div className="flex gap-6">
            <Link
              href="#"
              className="text-xs t-faint hover:t-muted"
            >
              {t("privacy")}
            </Link>
            <Link
              href="#"
              className="text-xs t-faint hover:t-muted"
            >
              {t("terms")}
            </Link>
          </div>
        </div>
      </div>
    </footer>
  );
}
