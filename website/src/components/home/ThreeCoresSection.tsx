"use client";

import {
  Building2,
  Coins,
  Store,
  Vote,
  Shield,
  Megaphone,
  Users,
  ArrowUpRight,
  TrendingUp,
  Target,
  Bot,
  Brain,
  Cpu,
  LineChart,
  Zap,
} from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const colorMap: Record<string, { border: string; bg: string; text: string; iconBg: string }> = {
  blue: {
    border: "border-blue-500/20",
    bg: "bg-blue-500/5",
    text: "text-blue-400",
    iconBg: "bg-blue-500/10",
  },
  green: {
    border: "border-emerald-500/20",
    bg: "bg-emerald-500/5",
    text: "text-emerald-400",
    iconBg: "bg-emerald-500/10",
  },
  purple: {
    border: "border-purple-500/20",
    bg: "bg-purple-500/5",
    text: "text-purple-400",
    iconBg: "bg-purple-500/10",
  },
};

export function ThreeCoresSection() {
  const t = useTranslations("home.threeCores");

  const cores = [
    {
      id: "tokenize",
      title: t("core1Title"),
      subtitle: t("core1Subtitle"),
      href: "/tokenize",
      color: "blue",
      features: [
        { icon: Building2, label: t("core1F1"), desc: t("core1F1Desc") },
        { icon: Coins, label: t("core1F2"), desc: t("core1F2Desc") },
        { icon: Store, label: t("core1F3"), desc: t("core1F3Desc") },
        { icon: Vote, label: t("core1F4"), desc: t("core1F4Desc") },
        { icon: Shield, label: t("core1F5"), desc: t("core1F5Desc") },
      ],
      comparison: [
        { traditional: t("core1C1T"), nexus: t("core1C1N") },
        { traditional: t("core1C2T"), nexus: t("core1C2N") },
        { traditional: t("core1C3T"), nexus: t("core1C3N") },
      ],
    },
    {
      id: "growth",
      title: t("core2Title"),
      subtitle: t("core2Subtitle"),
      href: "/growth",
      color: "green",
      features: [
        { icon: Megaphone, label: t("core2F1"), desc: t("core2F1Desc") },
        { icon: Users, label: t("core2F2"), desc: t("core2F2Desc") },
        { icon: ArrowUpRight, label: t("core2F3"), desc: t("core2F3Desc") },
        { icon: Target, label: t("core2F4"), desc: t("core2F4Desc") },
        { icon: TrendingUp, label: t("core2F5"), desc: t("core2F5Desc") },
      ],
      comparison: [
        { traditional: t("core2C1T"), nexus: t("core2C1N") },
        { traditional: t("core2C2T"), nexus: t("core2C2N") },
        { traditional: t("core2C3T"), nexus: t("core2C3N") },
      ],
    },
    {
      id: "ai",
      title: t("core3Title"),
      subtitle: t("core3Subtitle"),
      href: "/ai",
      color: "purple",
      features: [
        { icon: Bot, label: t("core3F1"), desc: t("core3F1Desc") },
        { icon: Brain, label: t("core3F2"), desc: t("core3F2Desc") },
        { icon: LineChart, label: t("core3F3"), desc: t("core3F3Desc") },
        { icon: Cpu, label: t("core3F4"), desc: t("core3F4Desc") },
        { icon: Zap, label: t("core3F5"), desc: t("core3F5Desc") },
      ],
      comparison: [
        { traditional: t("core3C1T"), nexus: t("core3C1N") },
        { traditional: t("core3C2T"), nexus: t("core3C2N") },
        { traditional: t("core3C3T"), nexus: t("core3C3N") },
      ],
    },
  ];

  return (
    <section className="section-padding">
      <div className="container-wide">
        <div className="mx-auto mb-20 max-w-2xl text-center">
          <h2 className="text-3xl font-bold tracking-tight sm:text-4xl">
            <span className="gradient-text">{t("title")}</span> {t("titleSuffix")}
          </h2>
          <p className="mt-4 text-lg t-muted">
            {t("subtitle")}
          </p>
        </div>

        <div className="space-y-24">
          {cores.map((core, coreIdx) => {
            const colors = colorMap[core.color];
            const isReversed = coreIdx % 2 === 1;

            return (
              <div key={core.id} className="relative">
                {/* Section header */}
                <div className={`mb-10 flex items-center gap-4 ${isReversed ? "md:flex-row-reverse" : ""}`}>
                  <div className={`h-px flex-1 ${colors.border} border-t`} />
                  <div className="flex items-center gap-3">
                    <span className={`text-sm font-bold ${colors.text}`}>
                      0{coreIdx + 1}
                    </span>
                    <h3 className="text-2xl font-bold sm:text-3xl">
                      {core.title}
                    </h3>
                  </div>
                  <div className={`h-px flex-1 ${colors.border} border-t`} />
                </div>

                <p className="mb-10 text-center t-muted">{core.subtitle}</p>

                {/* Features grid */}
                <div className="mb-10 grid gap-4 sm:grid-cols-2 lg:grid-cols-5">
                  {core.features.map((feat) => (
                    <div
                      key={feat.label}
                      className={`glass-card-hover p-5 ${colors.border}`}
                    >
                      <div className={`mb-3 inline-flex rounded-lg p-2 ${colors.iconBg}`}>
                        <feat.icon size={18} className={colors.text} />
                      </div>
                      <h4 className="mb-1 text-sm font-semibold t-primary">
                        {feat.label}
                      </h4>
                      <p className="text-xs leading-relaxed t-faint">
                        {feat.desc}
                      </p>
                    </div>
                  ))}
                </div>

                {/* Comparison table */}
                <div className={`mx-auto max-w-2xl rounded-xl border ${colors.border} ${colors.bg} p-6`}>
                  <div className="mb-4 grid grid-cols-2 gap-4 text-xs font-semibold uppercase tracking-wider t-faint">
                    <div>{t("traditionalLabel")}</div>
                    <div className={colors.text}>{t("nexusLabel")}</div>
                  </div>
                  {core.comparison.map((row, i) => (
                    <div
                      key={i}
                      className="grid grid-cols-2 gap-4 border-t border-[var(--border-subtle)] py-3"
                    >
                      <div className="text-sm t-muted line-through decoration-[var(--border-subtle)]">
                        {row.traditional}
                      </div>
                      <div className={`text-sm font-medium ${colors.text}`}>
                        {row.nexus}
                      </div>
                    </div>
                  ))}
                </div>

                {/* Link */}
                <div className="mt-6 text-center">
                  <Link
                    href={core.href}
                    className={`inline-flex items-center gap-1 text-sm font-medium ${colors.text} transition-all hover:gap-2`}
                  >
                    {t("learnMore")}{core.title}
                    <ArrowUpRight size={14} />
                  </Link>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}
