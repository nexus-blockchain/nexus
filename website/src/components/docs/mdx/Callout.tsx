import { Info, AlertTriangle, AlertCircle, Lightbulb } from "lucide-react";
import { cn } from "@/lib/utils";

const styles = {
  info: {
    border: "border-blue-500/30",
    bg: "bg-blue-500/5",
    icon: Info,
    iconColor: "text-blue-400",
  },
  warning: {
    border: "border-yellow-500/30",
    bg: "bg-yellow-500/5",
    icon: AlertTriangle,
    iconColor: "text-yellow-400",
  },
  danger: {
    border: "border-red-500/30",
    bg: "bg-red-500/5",
    icon: AlertCircle,
    iconColor: "text-red-400",
  },
  tip: {
    border: "border-emerald-500/30",
    bg: "bg-emerald-500/5",
    icon: Lightbulb,
    iconColor: "text-emerald-400",
  },
};

interface CalloutProps {
  type?: "info" | "warning" | "danger" | "tip";
  title?: string;
  children: React.ReactNode;
}

export function Callout({ type = "info", title, children }: CalloutProps) {
  const s = styles[type];
  const Icon = s.icon;

  return (
    <div className={cn("my-6 rounded-xl border-l-4 p-4", s.border, s.bg)}>
      <div className="flex gap-3">
        <Icon size={20} className={cn("mt-0.5 shrink-0", s.iconColor)} />
        <div className="min-w-0">
          {title && <p className="mb-1 font-semibold">{title}</p>}
          <div className="text-sm leading-relaxed text-[rgb(var(--text-secondary))]">{children}</div>
        </div>
      </div>
    </div>
  );
}
