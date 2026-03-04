"use client";

import Link from "next/link";
import {
  Building2,
  Users,
  Cpu,
  TrendingUp,
  Code,
  ArrowRight,
} from "lucide-react";
import { useTranslations } from "next-intl";

export function CTASection() {
  const t = useTranslations("home.cta");

  const roles = [
    {
      icon: Building2,
      title: t("role1"),
      actions: [t("role1A1"), t("role1A2"), t("role1A3")],
      href: "https://app.nexus.io",
      cta: t("role1Cta"),
      color: "text-blue-400",
      iconBg: "bg-blue-500/10",
    },
    {
      icon: Users,
      title: t("role2"),
      actions: [t("role2A1"), t("role2A2"), t("role2A3")],
      href: "/growth",
      cta: t("role2Cta"),
      color: "text-emerald-400",
      iconBg: "bg-emerald-500/10",
    },
    {
      icon: Cpu,
      title: t("role3"),
      actions: [t("role3A1"), t("role3A2"), t("role3A3")],
      href: "/ai",
      cta: t("role3Cta"),
      color: "text-purple-400",
      iconBg: "bg-purple-500/10",
    },
    {
      icon: TrendingUp,
      title: t("role4"),
      actions: [t("role4A1"), t("role4A2"), t("role4A3")],
      href: "/growth",
      cta: t("role4Cta"),
      color: "text-amber-400",
      iconBg: "bg-amber-500/10",
    },
    {
      icon: Code,
      title: t("role5"),
      actions: [t("role5A1"), t("role5A2"), t("role5A3")],
      href: "/tech",
      cta: t("role5Cta"),
      color: "text-cyan-400",
      iconBg: "bg-cyan-500/10",
    },
  ];

  return (
    <section className="section-padding relative overflow-hidden">
      {/* BG */}
      <div className="pointer-events-none absolute inset-0 bg-gradient-to-b from-transparent via-purple-500/5 to-transparent" />

      <div className="container-wide relative z-10">
        <div className="mx-auto mb-16 max-w-2xl text-center">
          <h2 className="text-3xl font-bold tracking-tight sm:text-4xl">
            {t("title")} <span className="gradient-text">{t("titleHighlight")}</span>
          </h2>
          <p className="mt-4 text-lg t-muted">
            {t("subtitle")}
          </p>
        </div>

        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-5">
          {roles.map((role) => (
            <Link
              key={role.title}
              href={role.href}
              className="glass-card-hover group flex flex-col p-6"
            >
              <div className={`mb-4 inline-flex self-start rounded-xl p-3 ${role.iconBg}`}>
                <role.icon size={22} className={role.color} />
              </div>

              <h3 className="mb-3 text-lg font-bold t-primary">{role.title}</h3>

              <ul className="mb-6 flex-1 space-y-1.5">
                {role.actions.map((action) => (
                  <li key={action} className="flex items-center gap-2 text-sm t-muted">
                    <div className={`h-1 w-1 rounded-full ${role.iconBg}`} />
                    {action}
                  </li>
                ))}
              </ul>

              <div className={`flex items-center gap-1 text-sm font-medium ${role.color} transition-all group-hover:gap-2`}>
                {role.cta}
                <ArrowRight size={14} />
              </div>
            </Link>
          ))}
        </div>

        {/* Big CTA */}
        <div className="mt-20 rounded-2xl border border-[var(--glass-border)] bg-gradient-to-r from-blue-600/10 via-purple-600/10 to-emerald-600/10 p-12 text-center">
          <h3 className="text-2xl font-bold sm:text-3xl">
            {t("bigCtaTitle")}
          </h3>
          <p className="mx-auto mt-4 max-w-xl t-muted">
            {t("bigCtaDesc")}
          </p>
          <div className="mt-8 flex flex-col items-center justify-center gap-4 sm:flex-row">
            <a
              href="https://app.nexus.io"
              className="group flex items-center gap-2 rounded-xl bg-gradient-to-r from-blue-600 to-purple-600 px-8 py-3.5 font-semibold text-white shadow-lg shadow-purple-500/20 transition-all hover:from-blue-500 hover:to-purple-500 hover:shadow-xl"
            >
              {t("bigCtaPrimary")}
              <ArrowRight size={16} className="transition-transform group-hover:translate-x-1" />
            </a>
            <Link
              href="/join"
              className="rounded-xl border border-[var(--glass-border)] px-8 py-3.5 font-semibold t-secondary transition-all hover:border-[var(--glass-hover-border)] hover:bg-[var(--overlay-subtle)]"
            >
              {t("bigCtaSecondary")}
            </Link>
          </div>
        </div>
      </div>
    </section>
  );
}
