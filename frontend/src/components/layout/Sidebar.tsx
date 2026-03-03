"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/stores/ui";
import {
  LayoutDashboard,
  Building2,
  Store,
  Coins,
  TrendingUp,
  Users,
  Wallet,
  Vote,
  FileText,
  ShieldCheck,
  Rocket,
  ChevronLeft,
  ChevronRight,
} from "lucide-react";

const navItems = [
  { href: "/", label: "Dashboard", icon: LayoutDashboard },
  {
    label: "Entity",
    icon: Building2,
    children: [
      { href: "/entity/settings", label: "Settings" },
      { href: "/entity/admins", label: "Admins" },
      { href: "/entity/fund", label: "Fund" },
    ],
  },
  {
    label: "Shops",
    icon: Store,
    children: [
      { href: "/shops", label: "Shop List" },
      { href: "/shops/products", label: "Products" },
      { href: "/shops/orders", label: "Orders" },
      { href: "/shops/reviews", label: "Reviews" },
    ],
  },
  {
    label: "Token",
    icon: Coins,
    children: [
      { href: "/token/config", label: "Config" },
      { href: "/token/holders", label: "Holders" },
      { href: "/token/dividend", label: "Dividend" },
      { href: "/token/lock", label: "Lock" },
    ],
  },
  { href: "/market", label: "Market", icon: TrendingUp },
  {
    label: "Members",
    icon: Users,
    children: [
      { href: "/members/list", label: "Member List" },
      { href: "/members/levels", label: "Levels" },
      { href: "/members/rules", label: "Upgrade Rules" },
      { href: "/members/pending", label: "Pending" },
    ],
  },
  { href: "/commission", label: "Commission", icon: Wallet },
  {
    label: "Governance",
    icon: Vote,
    children: [
      { href: "/governance/proposals", label: "Proposals" },
      { href: "/governance/config", label: "Config" },
    ],
  },
  {
    label: "Disclosure",
    icon: FileText,
    children: [
      { href: "/disclosure/reports", label: "Reports" },
      { href: "/disclosure/announcements", label: "Announcements" },
      { href: "/disclosure/insiders", label: "Insiders" },
    ],
  },
  {
    label: "KYC",
    icon: ShieldCheck,
    children: [
      { href: "/kyc/records", label: "Records" },
      { href: "/kyc/providers", label: "Providers" },
      { href: "/kyc/settings", label: "Settings" },
    ],
  },
  {
    label: "Tokensale",
    icon: Rocket,
    children: [
      { href: "/tokensale/rounds", label: "Rounds" },
      { href: "/tokensale/create", label: "Create" },
    ],
  },
];

interface NavItemProps {
  item: (typeof navItems)[number];
  collapsed: boolean;
  pathname: string;
}

function NavItem({ item, collapsed, pathname }: NavItemProps) {
  const Icon = item.icon;

  if ("href" in item && item.href) {
    const isActive = pathname === item.href;
    return (
      <Link
        href={item.href}
        className={cn(
          "flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors",
          isActive
            ? "bg-sidebar-accent text-sidebar-accent-foreground font-medium"
            : "text-sidebar-foreground/70 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground"
        )}
      >
        <Icon className="h-4 w-4 shrink-0" />
        {!collapsed && <span>{item.label}</span>}
      </Link>
    );
  }

  const children = "children" in item ? item.children : [];
  const isGroupActive = children?.some((c) => pathname.startsWith(c.href));

  return (
    <div className="space-y-1">
      <div
        className={cn(
          "flex items-center gap-3 rounded-lg px-3 py-2 text-xs font-semibold uppercase tracking-wider",
          isGroupActive ? "text-sidebar-primary" : "text-sidebar-foreground/50"
        )}
      >
        <Icon className="h-4 w-4 shrink-0" />
        {!collapsed && <span>{item.label}</span>}
      </div>
      {!collapsed &&
        children?.map((child) => {
          const isActive = pathname === child.href || pathname.startsWith(child.href + "/");
          return (
            <Link
              key={child.href}
              href={child.href}
              className={cn(
                "flex items-center gap-3 rounded-lg py-1.5 pl-10 pr-3 text-sm transition-colors",
                isActive
                  ? "bg-sidebar-accent text-sidebar-accent-foreground font-medium"
                  : "text-sidebar-foreground/60 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground"
              )}
            >
              {child.label}
            </Link>
          );
        })}
    </div>
  );
}

export function Sidebar() {
  const pathname = usePathname();
  const { sidebarCollapsed, toggleSidebar } = useUiStore();

  return (
    <aside
      className={cn(
        "flex h-screen flex-col border-r bg-sidebar transition-all duration-300",
        sidebarCollapsed ? "w-16" : "w-64"
      )}
    >
      <div className="flex h-14 items-center justify-between border-b px-4">
        {!sidebarCollapsed && (
          <span className="text-lg font-bold text-sidebar-primary">NEXUS</span>
        )}
        <button
          onClick={toggleSidebar}
          className="rounded-md p-1.5 text-sidebar-foreground/50 hover:bg-sidebar-accent hover:text-sidebar-foreground"
        >
          {sidebarCollapsed ? <ChevronRight className="h-4 w-4" /> : <ChevronLeft className="h-4 w-4" />}
        </button>
      </div>

      <nav className="flex-1 space-y-1 overflow-y-auto p-3">
        {navItems.map((item, i) => (
          <NavItem key={i} item={item} collapsed={sidebarCollapsed} pathname={pathname} />
        ))}
      </nav>
    </aside>
  );
}
