"use client";

import {
  Building2,
  Coins,
  Store,
  Vote,
  Shield,
  FileCheck,
  ArrowRight,
  CheckCircle2,
} from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function TokenizePage() {
  const t = useTranslations("tokenize");

  const steps = [
    {
      num: "01",
      icon: Building2,
      title: t("s1Title"),
      desc: t("s1Desc"),
      pallets: ["entity-registry", "entity-kyc"],
      details: [t("s1D1"), t("s1D2"), t("s1D3")],
    },
    {
      num: "02",
      icon: Coins,
      title: t("s2Title"),
      desc: t("s2Desc"),
      pallets: ["entity-token"],
      details: [t("s2D1"), t("s2D2"), t("s2D3")],
    },
    {
      num: "03",
      icon: Store,
      title: t("s3Title"),
      desc: t("s3Desc"),
      pallets: ["entity-shop", "entity-product", "entity-order", "entity-review"],
      details: [t("s3D1"), t("s3D2"), t("s3D3")],
    },
    {
      num: "04",
      icon: FileCheck,
      title: t("s4Title"),
      desc: t("s4Desc"),
      pallets: ["entity-market", "entity-disclosure"],
      details: [t("s4D1"), t("s4D2"), t("s4D3")],
    },
    {
      num: "05",
      icon: Vote,
      title: t("s5Title"),
      desc: t("s5Desc"),
      pallets: ["entity-governance"],
      details: [t("s5D1"), t("s5D2"), t("s5D3")],
    },
  ];

  const comparisons = [
    { traditional: t("comp1T"), nexus: t("comp1N") },
    { traditional: t("comp2T"), nexus: t("comp2N") },
    { traditional: t("comp3T"), nexus: t("comp3N") },
    { traditional: t("comp4T"), nexus: t("comp4N") },
    { traditional: t("comp5T"), nexus: t("comp5N") },
  ];

  return (
    <div className="pt-16">
      {/* Hero */}
      <section className="hero-gradient section-padding text-center">
        <div className="container-wide">
          <div className="mb-4 inline-flex items-center gap-2 rounded-full border border-blue-500/20 bg-blue-500/5 px-4 py-1.5 text-sm text-blue-400">
            <Building2 size={14} />
            {t("badge")}
          </div>
          <h1 className="mx-auto max-w-3xl text-4xl font-bold leading-tight sm:text-5xl lg:text-6xl">
            <span className="bg-gradient-to-r from-blue-400 to-blue-600 bg-clip-text text-transparent">
              {t("title")}
            </span>
          </h1>
          <p className="mx-auto mt-6 max-w-2xl text-lg t-muted">
            {t("subtitle")}
          </p>
        </div>
      </section>

      {/* Why tokenize */}
      <section className="section-padding">
        <div className="container-wide">
          <h2 className="mb-12 text-center text-3xl font-bold">
            {t("whyTitle")} <span className="text-blue-400">{t("whyHighlight")}</span>？
          </h2>
          <div className="mx-auto max-w-3xl overflow-hidden rounded-xl border border-[var(--glass-border)]">
            <div className="grid grid-cols-2 bg-[var(--overlay-subtle)] px-6 py-3 text-xs font-semibold uppercase tracking-wider t-faint">
              <div>{t("traditionalLabel")}</div>
              <div className="text-blue-400">{t("nexusLabel")}</div>
            </div>
            {comparisons.map((row, i) => (
              <div key={i} className="grid grid-cols-2 border-t border-[var(--border-subtle)] px-6 py-4">
                <div className="pr-4 text-sm t-muted line-through decoration-[var(--border-subtle)]">
                  {row.traditional}
                </div>
                <div className="text-sm font-medium text-blue-400">
                  <CheckCircle2 size={14} className="mr-1.5 inline text-blue-500" />
                  {row.nexus}
                </div>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* 5 Steps */}
      <section className="section-padding bg-[var(--overlay-muted)]">
        <div className="container-wide">
          <h2 className="mb-4 text-center text-3xl font-bold">
            {t("stepsTitle")} <span className="text-blue-400">{t("stepsHighlight")}</span>
          </h2>
          <p className="mb-16 text-center t-muted">
            {t("stepsSubtitle")}
          </p>

          <div className="space-y-8">
            {steps.map((step) => (
              <div
                key={step.num}
                className="glass-card-hover group grid gap-6 p-8 md:grid-cols-[auto_1fr_1fr]"
              >
                {/* Number + Icon */}
                <div className="flex items-start gap-4">
                  <span className="text-3xl font-bold text-blue-500/30">
                    {step.num}
                  </span>
                  <div className="rounded-xl bg-blue-500/10 p-3">
                    <step.icon size={24} className="text-blue-400" />
                  </div>
                </div>

                {/* Main info */}
                <div>
                  <h3 className="mb-2 text-xl font-bold">{step.title}</h3>
                  <p className="mb-4 text-sm t-muted">{step.desc}</p>
                  <div className="flex flex-wrap gap-2">
                    {step.pallets.map((p) => (
                      <span
                        key={p}
                        className="rounded-md bg-blue-500/10 px-2 py-0.5 text-xs text-blue-400"
                      >
                        {p}
                      </span>
                    ))}
                  </div>
                </div>

                {/* Details */}
                <ul className="space-y-2">
                  {step.details.map((d, i) => (
                    <li key={i} className="flex items-start gap-2 text-sm t-secondary">
                      <CheckCircle2 size={14} className="mt-0.5 shrink-0 text-blue-500/50" />
                      {d}
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* Compliance */}
      <section className="section-padding">
        <div className="container-wide">
          <h2 className="mb-12 text-center text-3xl font-bold">
            {t("complianceTitle")} <span className="text-blue-400">{t("complianceHighlight")}</span>
          </h2>
          <div className="grid gap-6 md:grid-cols-3">
            {[
              { icon: Shield, title: t("kyc"), desc: t("kycDesc") },
              { icon: FileCheck, title: t("disclosure"), desc: t("disclosureDesc") },
              { icon: Vote, title: t("arbitration"), desc: t("arbitrationDesc") },
            ].map((item) => (
              <div key={item.title} className="glass-card p-8">
                <div className="mb-4 inline-flex rounded-xl bg-blue-500/10 p-3">
                  <item.icon size={24} className="text-blue-400" />
                </div>
                <h3 className="mb-2 text-lg font-bold">{item.title}</h3>
                <p className="text-sm leading-relaxed t-muted">{item.desc}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* CTA */}
      <section className="section-padding text-center">
        <div className="container-wide">
          <h2 className="text-2xl font-bold">{t("ctaTitle")}</h2>
          <div className="mt-8 flex justify-center gap-4">
            <a
              href="https://app.nexus.io"
              className="group flex items-center gap-2 rounded-xl bg-gradient-to-r from-blue-600 to-blue-500 px-8 py-3.5 font-semibold text-white"
            >
              {t("ctaPrimary")}
              <ArrowRight size={16} className="transition-transform group-hover:translate-x-1" />
            </a>
            <Link
              href="/growth"
              className="rounded-xl border border-[var(--glass-border)] px-8 py-3.5 font-semibold t-secondary hover:bg-[var(--overlay-subtle)]"
            >
              {t("ctaSecondary")}
            </Link>
          </div>
        </div>
      </section>
    </div>
  );
}
