"use client";

import { useEffect, useRef, useState } from "react";
import { useTranslations } from "next-intl";

export function TokenLoopSection() {
  const t = useTranslations("home.tokenLoop");
  const [activeIndex, setActiveIndex] = useState(0);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const loopSteps = [
    { label: t("step1"), sub: t("step1Sub"), color: "#3B82F6" },
    { label: t("step2"), sub: t("step2Sub"), color: "#10B981" },
    { label: t("step3"), sub: t("step3Sub"), color: "#10B981" },
    { label: t("step4"), sub: t("step4Sub"), color: "#8B5CF6" },
    { label: t("step5"), sub: t("step5Sub"), color: "#8B5CF6" },
    { label: t("step6"), sub: t("step6Sub"), color: "#F59E0B" },
  ];

  useEffect(() => {
    intervalRef.current = setInterval(() => {
      setActiveIndex((prev) => (prev + 1) % loopSteps.length);
    }, 2500);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [loopSteps.length]);

  return (
    <section id="three-cores" className="section-padding relative overflow-hidden">
      {/* BG glow */}
      <div className="pointer-events-none absolute left-1/2 top-1/2 h-[500px] w-[500px] -translate-x-1/2 -translate-y-1/2 rounded-full bg-purple-500/5 blur-[120px]" />

      <div className="container-wide relative z-10">
        <div className="mx-auto mb-16 max-w-2xl text-center">
          <h2 className="text-3xl font-bold tracking-tight sm:text-4xl">
            {t("title")} <span className="gradient-text">{t("titleHighlight")}</span>
          </h2>
          <p className="mt-4 text-lg t-muted">
            {t("subtitle")}
          </p>
        </div>

        {/* Loop visualization */}
        <div className="mx-auto max-w-3xl">
          {/* Circle of steps */}
          <div className="relative mx-auto aspect-square max-w-lg">
            {/* Center label */}
            <div className="absolute left-1/2 top-1/2 z-10 -translate-x-1/2 -translate-y-1/2 text-center">
              <div className="rounded-2xl border border-[var(--glass-border)] bg-[rgb(var(--bg-primary))]/90 px-6 py-4 backdrop-blur-sm">
                <div className="text-xs font-medium uppercase tracking-wider t-faint">
                  {t("centerTop")}
                </div>
                <div className="mt-1 text-lg font-bold t-primary">
                  {t("centerBottom")}
                </div>
              </div>
            </div>

            {/* Circular nodes */}
            {loopSteps.map((step, i) => {
              const angle = (i / loopSteps.length) * 2 * Math.PI - Math.PI / 2;
              const radius = 42;
              const x = 50 + radius * Math.cos(angle);
              const y = 50 + radius * Math.sin(angle);
              const isActive = i === activeIndex;

              return (
                <div
                  key={i}
                  className="absolute -translate-x-1/2 -translate-y-1/2 transition-all duration-500"
                  style={{
                    left: `${x}%`,
                    top: `${y}%`,
                  }}
                  onMouseEnter={() => {
                    if (intervalRef.current) clearInterval(intervalRef.current);
                    setActiveIndex(i);
                  }}
                  onMouseLeave={() => {
                    intervalRef.current = setInterval(() => {
                      setActiveIndex((prev) => (prev + 1) % loopSteps.length);
                    }, 2500);
                  }}
                >
                  <div
                    className={`cursor-pointer rounded-xl border px-4 py-3 text-center transition-all duration-300 ${
                      isActive
                        ? "scale-110 border-[var(--glass-hover-border)] bg-[var(--glass-hover-bg)] shadow-lg"
                        : "border-[var(--border-subtle)] bg-[var(--glass-bg)] hover:border-[var(--glass-border)] hover:bg-[var(--glass-hover-bg)]"
                    }`}
                    style={{
                      boxShadow: isActive
                        ? `0 0 30px ${step.color}20`
                        : undefined,
                    }}
                  >
                    <div
                      className={`text-sm font-semibold ${isActive ? '' : 't-secondary'}`}
                      style={{ color: isActive ? step.color : undefined }}
                    >
                      {step.label}
                    </div>
                    <div className="mt-0.5 text-xs t-faint">
                      {step.sub}
                    </div>
                  </div>
                </div>
              );
            })}

            {/* SVG connecting ring */}
            <svg className="absolute inset-0 h-full w-full" viewBox="0 0 100 100">
              <circle
                cx="50"
                cy="50"
                r="42"
                fill="none"
                stroke="var(--border-subtle)"
                strokeWidth="0.3"
              />
              {/* Animated arc */}
              <circle
                cx="50"
                cy="50"
                r="42"
                fill="none"
                stroke="url(#loopGradient)"
                strokeWidth="0.5"
                strokeDasharray={`${(2 * Math.PI * 42) / loopSteps.length} ${2 * Math.PI * 42}`}
                strokeDashoffset={
                  -(activeIndex / loopSteps.length) * 2 * Math.PI * 42 +
                  (Math.PI / 2) * 42
                }
                strokeLinecap="round"
                className="transition-all duration-500"
              />
              <defs>
                <linearGradient id="loopGradient" x1="0%" y1="0%" x2="100%" y2="100%">
                  <stop offset="0%" stopColor="#3B82F6" />
                  <stop offset="50%" stopColor="#8B5CF6" />
                  <stop offset="100%" stopColor="#10B981" />
                </linearGradient>
              </defs>
            </svg>
          </div>
        </div>

        {/* Bottom description */}
        <div className="mx-auto mt-12 grid max-w-4xl gap-6 md:grid-cols-3">
          {[
            {
              title: t("bottom1Title"),
              desc: t("bottom1Desc"),
              color: "text-blue-400",
            },
            {
              title: t("bottom2Title"),
              desc: t("bottom2Desc"),
              color: "text-emerald-400",
            },
            {
              title: t("bottom3Title"),
              desc: t("bottom3Desc"),
              color: "text-purple-400",
            },
          ].map((item) => (
            <div key={item.title} className="text-center">
              <h3 className={`text-sm font-semibold ${item.color}`}>
                {item.title}
              </h3>
              <p className="mt-1 text-xs t-faint">{item.desc}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
