"use client";

import { Building2, Users, Cpu, TrendingUp, Code, ArrowRight } from "lucide-react";
import { useTranslations } from "next-intl";

const colorMap: Record<string, { border: string; badge: string; icon: string; bg: string }> = {
  blue: { border: "border-blue-500/20", badge: "text-blue-400", icon: "text-blue-400", bg: "bg-blue-500/10" },
  green: { border: "border-emerald-500/20", badge: "text-emerald-400", icon: "text-emerald-400", bg: "bg-emerald-500/10" },
  purple: { border: "border-purple-500/20", badge: "text-purple-400", icon: "text-purple-400", bg: "bg-purple-500/10" },
  amber: { border: "border-amber-500/20", badge: "text-amber-400", icon: "text-amber-400", bg: "bg-amber-500/10" },
  cyan: { border: "border-cyan-500/20", badge: "text-cyan-400", icon: "text-cyan-400", bg: "bg-cyan-500/10" },
};

export default function JoinPage() {
  const t = useTranslations("join");

  const roles = [
    {
      icon: Building2,
      title: t("r1Title"),
      desc: t("r1Desc"),
      steps: [t("r1S1"), t("r1S2"), t("r1S3"), t("r1S4"), t("r1S5")],
      cta: t("r1Cta"),
      href: "https://app.nexus.io",
      color: "blue",
    },
    {
      icon: Users,
      title: t("r2Title"),
      desc: t("r2Desc"),
      steps: [t("r2S1"), t("r2S2"), t("r2S3"), t("r2S4"), t("r2S5")],
      cta: t("r2Cta"),
      href: "/growth",
      color: "green",
    },
    {
      icon: Cpu,
      title: t("r3Title"),
      desc: t("r3Desc"),
      steps: [t("r3S1"), t("r3S2"), t("r3S3"), t("r3S4"), t("r3S5")],
      cta: t("r3Cta"),
      href: "/ai",
      color: "purple",
    },
    {
      icon: TrendingUp,
      title: t("r4Title"),
      desc: t("r4Desc"),
      steps: [t("r4S1"), t("r4S2"), t("r4S3"), t("r4S4"), t("r4S5")],
      cta: t("r4Cta"),
      href: "/growth",
      color: "amber",
    },
    {
      icon: Code,
      title: t("r5Title"),
      desc: t("r5Desc"),
      steps: [t("r5S1"), t("r5S2"), t("r5S3"), t("r5S4"), t("r5S5")],
      cta: t("r5Cta"),
      href: "/tech",
      color: "cyan",
    },
  ];

  return (
    <div className="pt-16">
      <section className="hero-gradient section-padding text-center">
        <div className="container-wide">
          <h1 className="text-4xl font-bold sm:text-5xl">
            {t("title")} <span className="gradient-text">NEXUS</span>
          </h1>
          <p className="mx-auto mt-6 max-w-2xl text-lg t-muted">
            {t("subtitle")}
          </p>
        </div>
      </section>

      <section className="section-padding">
        <div className="container-wide space-y-6">
          {roles.map((role) => {
            const colors = colorMap[role.color];
            return (
              <div key={role.title} className={`glass-card overflow-hidden ${colors.border}`}>
                <div className="grid gap-6 p-8 lg:grid-cols-[1fr_2fr_auto]">
                  <div>
                    <div className={`mb-4 inline-flex rounded-xl p-3 ${colors.bg}`}>
                      <role.icon size={28} className={colors.icon} />
                    </div>
                    <h2 className="text-xl font-bold">{role.title}</h2>
                    <p className="mt-2 text-sm t-muted">{role.desc}</p>
                  </div>

                  <div>
                    <h3 className="mb-3 text-sm font-semibold t-faint">{t("stepsLabel")}</h3>
                    <div className="space-y-2">
                      {role.steps.map((step) => (
                        <div key={step} className="flex items-center gap-3 rounded-lg border border-[var(--border-subtle)] bg-[var(--overlay-muted)] px-4 py-2.5 text-sm t-secondary">
                          {step}
                        </div>
                      ))}
                    </div>
                  </div>

                  <div className="flex items-center">
                    <a
                      href={role.href}
                      className={`group flex items-center gap-2 whitespace-nowrap rounded-xl px-6 py-3 text-sm font-semibold ${colors.bg} ${colors.badge}`}
                    >
                      {role.cta}
                      <ArrowRight size={14} className="transition-transform group-hover:translate-x-1" />
                    </a>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </section>

      <section className="section-padding text-center">
        <div className="container-wide">
          <div className="rounded-2xl border border-[var(--glass-border)] bg-gradient-to-r from-blue-600/10 via-purple-600/10 to-emerald-600/10 p-12">
            <h2 className="text-2xl font-bold sm:text-3xl">{t("communityTitle")}</h2>
            <p className="mx-auto mt-4 max-w-xl t-muted">
              {t("communityDesc")}
            </p>
            <div className="mt-8 flex flex-wrap justify-center gap-4">
              {[
                { label: "Telegram", href: "#" },
                { label: "Discord", href: "#" },
                { label: "Twitter", href: "#" },
                { label: "GitHub", href: "#" },
              ].map((link) => (
                <a
                  key={link.label}
                  href={link.href}
                  className="rounded-xl border border-[var(--glass-border)] px-6 py-3 text-sm font-semibold t-secondary transition-all hover:border-[var(--glass-hover-border)] hover:bg-[var(--overlay-subtle)]"
                >
                  {link.label}
                </a>
              ))}
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}
