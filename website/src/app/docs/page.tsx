"use client";

import Link from "next/link";
import { TrendingUp, Wrench, ArrowRight, BookOpen } from "lucide-react";
import { useTranslations } from "next-intl";

export default function DocsHomePage() {
  const t = useTranslations("docs");

  const categories = [
    {
      key: "business",
      icon: TrendingUp,
      title: t("businessCenter"),
      desc: t("businessDesc"),
      cta: t("enterBusiness"),
      href: "/docs/business/overview",
      gradient: "from-amber-500/10 to-orange-500/10",
      iconBg: "bg-amber-500/10",
      iconColor: "text-amber-400",
      borderHover: "hover:border-amber-500/30",
    },
    {
      key: "technical",
      icon: Wrench,
      title: t("technicalDocs"),
      desc: t("technicalDesc"),
      cta: t("startReading"),
      href: "/docs/technical/getting-started/setup",
      gradient: "from-cyan-500/10 to-blue-500/10",
      iconBg: "bg-cyan-500/10",
      iconColor: "text-cyan-400",
      borderHover: "hover:border-cyan-500/30",
    },
  ];

  return (
    <div className="px-4 py-12 sm:px-8 lg:px-12">
      {/* Hero */}
      <div className="mb-12 text-center">
        <div className="mb-4 inline-flex items-center gap-2 rounded-full bg-[var(--overlay-subtle)] px-4 py-1.5 text-sm text-[rgb(var(--text-muted))]">
          <BookOpen size={14} />
          NEXUS Knowledge Base
        </div>
        <h1 className="text-3xl font-bold sm:text-4xl">
          {t("title")}
        </h1>
        <p className="mx-auto mt-4 max-w-xl text-[rgb(var(--text-muted))]">
          {t("subtitle")}
        </p>
      </div>

      {/* Two category cards */}
      <div className="mx-auto grid max-w-4xl gap-6 md:grid-cols-2">
        {categories.map((cat) => (
          <Link
            key={cat.key}
            href={cat.href}
            className={`glass-card-hover group relative overflow-hidden rounded-2xl p-8 transition-all ${cat.borderHover}`}
          >
            <div className={`absolute inset-0 bg-gradient-to-br ${cat.gradient} opacity-0 transition-opacity group-hover:opacity-100`} />
            <div className="relative">
              <div className={`mb-5 inline-flex rounded-xl p-3 ${cat.iconBg}`}>
                <cat.icon size={24} className={cat.iconColor} />
              </div>
              <h2 className="mb-2 text-xl font-bold">{cat.title}</h2>
              <p className="mb-6 text-sm leading-relaxed text-[rgb(var(--text-muted))]">
                {cat.desc}
              </p>
              <span className="inline-flex items-center gap-1.5 text-sm font-medium text-[rgb(var(--text-secondary))] transition-colors group-hover:text-[rgb(var(--text-primary))]">
                {cat.cta}
                <ArrowRight size={14} className="transition-transform group-hover:translate-x-1" />
              </span>
            </div>
          </Link>
        ))}
      </div>
    </div>
  );
}
