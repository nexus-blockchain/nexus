"use client";

import { Building2, Megaphone, Bot } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export function PainPointsSection() {
  const t = useTranslations("home.painPoints");

  const painPoints = [
    {
      icon: Building2,
      question: t("q1"),
      answer: t("a1"),
      description: t("d1"),
      color: "blue",
      href: "/tokenize",
      gradient: "from-blue-500/20 to-blue-600/5",
      iconBg: "bg-blue-500/10 text-blue-400",
      borderHover: "hover:border-blue-500/30",
    },
    {
      icon: Megaphone,
      question: t("q2"),
      answer: t("a2"),
      description: t("d2"),
      color: "green",
      href: "/growth",
      gradient: "from-emerald-500/20 to-emerald-600/5",
      iconBg: "bg-emerald-500/10 text-emerald-400",
      borderHover: "hover:border-emerald-500/30",
    },
    {
      icon: Bot,
      question: t("q3"),
      answer: t("a3"),
      description: t("d3"),
      color: "purple",
      href: "/ai",
      gradient: "from-purple-500/20 to-purple-600/5",
      iconBg: "bg-purple-500/10 text-purple-400",
      borderHover: "hover:border-purple-500/30",
    },
  ];

  return (
    <section className="section-padding relative">
      <div className="container-wide">
        <div className="mx-auto mb-16 max-w-2xl text-center">
          <h2 className="text-3xl font-bold tracking-tight sm:text-4xl">
            {t("title")} <span className="gradient-text">{t("titleHighlight")}</span>
          </h2>
          <p className="mt-4 text-lg t-muted">
            {t("subtitle")}
          </p>
        </div>

        <div className="grid gap-6 md:grid-cols-3">
          {painPoints.map((item) => (
            <Link
              key={item.answer}
              href={item.href}
              className={`glass-card group relative overflow-hidden p-8 transition-all duration-300 ${item.borderHover} hover:bg-[var(--glass-hover-bg)]`}
            >
              {/* Gradient bg */}
              <div
                className={`absolute inset-0 bg-gradient-to-b ${item.gradient} opacity-0 transition-opacity group-hover:opacity-100`}
              />

              <div className="relative z-10">
                <div className={`mb-6 inline-flex rounded-xl p-3 ${item.iconBg}`}>
                  <item.icon size={24} />
                </div>

                <p className="mb-3 text-lg font-medium leading-snug t-secondary">
                  {item.question}
                </p>

                <h3 className="mb-3 text-2xl font-bold t-primary">
                  → {item.answer}
                </h3>

                <p className="text-sm leading-relaxed t-muted">
                  {item.description}
                </p>

                <div className="mt-6 flex items-center gap-1 text-sm font-medium t-faint transition-colors group-hover:t-secondary">
                  {t("learnMore")}
                  <span className="transition-transform group-hover:translate-x-1">→</span>
                </div>
              </div>
            </Link>
          ))}
        </div>
      </div>
    </section>
  );
}
