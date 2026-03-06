"use client";

import { useState, useMemo } from "react";
import { useEntityStore } from "@/stores/entity";
import { useShops } from "@/hooks/useShop";
import { useMembers, useLevels, useMemberActions, useMemberCount, usePendingMembers } from "@/hooks/useMember";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { TxButton } from "@/components/shared/TxButton";
import { STATUS_COLORS, MEMBER_STATUS } from "@/lib/constants";
import {
  Users, UserCheck, UserX, Crown, Star, Shield, Zap,
  ShieldBan, ShieldOff, UserMinus, ArrowUpCircle, RotateCcw,
  Search, Settings, Layers, ChevronRight, AlertTriangle,
} from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const STATUS_ICONS: Record<string, typeof Users> = {
  Active: UserCheck,
  Pending: Shield,
  Frozen: ShieldOff,
  Banned: ShieldBan,
  Expired: AlertTriangle,
};

export default function MembersPage() {
  const { currentEntityId } = useEntityStore();
  const { shops } = useShops(currentEntityId);
  const primaryShop = shops.find((s) => s.isPrimary) || shops[0];
  const shopId = primaryShop?.id ?? null;
  const { members, isLoading, refetch } = useMembers(currentEntityId);
  const { levels } = useLevels(currentEntityId);
  const { count: totalCount, bannedCount } = useMemberCount(currentEntityId);
  const { pending } = usePendingMembers(currentEntityId);
  const actions = useMemberActions();
  const t = useTranslations("members");
  const tc = useTranslations("common");

  const [statusFilter, setStatusFilter] = useState<string>("All");
  const [searchQuery, setSearchQuery] = useState("");
  const [banReason, setBanReason] = useState("");
  const [manualLevelTarget, setManualLevelTarget] = useState<{ account: string; levelId: string } | null>(null);

  const statusCounts = useMemo(() => {
    const counts: Record<string, number> = { All: members.length };
    MEMBER_STATUS.forEach((s) => { counts[s] = 0; });
    members.forEach((m) => { counts[m.status] = (counts[m.status] || 0) + 1; });
    counts.Pending = pending.length;
    return counts;
  }, [members, pending]);

  const filteredMembers = useMemo(() => {
    let result = members;
    if (statusFilter !== "All") {
      result = result.filter((m) => m.status === statusFilter);
    }
    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      result = result.filter((m) => m.account.toLowerCase().includes(q));
    }
    return result;
  }, [members, statusFilter, searchQuery]);

  if (!currentEntityId) return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;

  const handleAction = async (action: () => Promise<void>) => {
    await action();
    refetch();
  };

  const quickLinks = [
    { href: "/members/levels", icon: Layers, label: "Level Management", desc: "Configure member levels and benefits" },
    { href: "/members/rules", icon: Zap, label: "Upgrade Rules", desc: "Automatic level upgrade triggers" },
    { href: "/members/policy", icon: Shield, label: "Member Policy", desc: "Registration and membership policies" },
    { href: "/members/pending", icon: UserCheck, label: "Pending Members", desc: `${pending.length} applications waiting` },
  ];

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">{t("title")}</h1>
          <p className="text-muted-foreground">{t("subtitle")}</p>
        </div>
        <Button variant="outline" size="sm" onClick={refetch}>
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-5">
        {MEMBER_STATUS.map((status) => {
          const Icon = STATUS_ICONS[status] || Users;
          return (
            <Card
              key={status}
              className={`cursor-pointer transition-all hover:shadow-md ${statusFilter === status ? "ring-2 ring-primary" : ""}`}
              onClick={() => setStatusFilter(statusFilter === status ? "All" : status)}
            >
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">{status}</CardTitle>
                <Icon className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <p className="text-2xl font-bold">{statusCounts[status] || 0}</p>
              </CardContent>
            </Card>
          );
        })}
      </div>

      <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-4">
        {quickLinks.map((link) => {
          const Icon = link.icon;
          return (
            <Link key={link.href} href={link.href}>
              <Card className="h-full cursor-pointer transition-all hover:shadow-md hover:border-primary/50">
                <CardContent className="flex items-center gap-3 p-4">
                  <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-primary/10">
                    <Icon className="h-5 w-5 text-primary" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="text-sm font-medium">{link.label}</p>
                    <p className="text-xs text-muted-foreground truncate">{link.desc}</p>
                  </div>
                  <ChevronRight className="h-4 w-4 text-muted-foreground" />
                </CardContent>
              </Card>
            </Link>
          );
        })}
      </div>

      {levels.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Crown className="h-5 w-5" />Level System</CardTitle>
            <CardDescription>{levels.length} levels configured</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="flex flex-wrap gap-3">
              {levels.map((level, i: number) => (
                <div key={i} className="flex items-center gap-2 rounded-lg border p-3">
                  <Star className="h-4 w-4 text-yellow-500" />
                  <div>
                    <p className="text-sm font-medium">{level.name || `Level ${level.id}`}</p>
                    <p className="text-xs text-muted-foreground">Threshold: {level.threshold} | Discount: {(level.discountRate / 100).toFixed(1)}%</p>
                  </div>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <div>
              <CardTitle className="flex items-center gap-2"><Users className="h-5 w-5" />Member List</CardTitle>
              <CardDescription>
                {statusFilter === "All" ? `${members.length} total members` : `${filteredMembers.length} ${statusFilter.toLowerCase()} members`}
              </CardDescription>
            </div>
            <div className="flex items-center gap-2">
              <div className="relative">
                <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
                <Input
                  placeholder="Search address..."
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="pl-9 w-64"
                />
              </div>
            </div>
          </div>
          <div className="flex flex-wrap gap-2 pt-2">
            <Badge
              variant={statusFilter === "All" ? "default" : "outline"}
              className="cursor-pointer"
              onClick={() => setStatusFilter("All")}
            >
              All ({members.length})
            </Badge>
            {MEMBER_STATUS.map((s) => (
              <Badge
                key={s}
                variant={statusFilter === s ? "default" : "outline"}
                className="cursor-pointer"
                onClick={() => setStatusFilter(s)}
              >
                {s} ({statusCounts[s] || 0})
              </Badge>
            ))}
          </div>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="flex justify-center py-8"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
          ) : filteredMembers.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12">
              <Users className="h-12 w-12 text-muted-foreground/50" />
              <p className="mt-4 text-lg font-medium">No Members Found</p>
              <p className="text-sm text-muted-foreground">
                {statusFilter !== "All" ? `No members with status "${statusFilter}".` : "No members have joined this entity yet."}
              </p>
            </div>
          ) : (
            <div className="space-y-3">
              {filteredMembers.map((member) => (
                <div key={member.account} className="rounded-lg border p-4">
                  <div className="flex items-center gap-4">
                    <AddressDisplay address={member.account} />
                    <StatusBadge status={member.status} />
                    <Badge variant="secondary">
                      {levels.find((l) => l.id === member.customLevelId)?.name || `Lv.${member.customLevelId}`}
                    </Badge>
                    <div className="flex items-center gap-4 text-xs text-muted-foreground">
                      <span>Spent: ${member.totalSpentUsdt?.toLocaleString?.() || 0}</span>
                      <span>Orders: {member.orderCount || 0}</span>
                      <span>Referrals: {member.directReferrals || 0}</span>
                      <span>Team: {member.teamSize || 0}</span>
                    </div>

                    <div className="ml-auto flex items-center gap-1">
                      {member.status === "Active" && shopId && (
                        <>
                          <Button
                            size="sm"
                            variant="outline"
                            onClick={() => setManualLevelTarget(manualLevelTarget?.account === member.account ? null : { account: member.account, levelId: "" })}
                          >
                            <ArrowUpCircle className="mr-1 h-3 w-3" />Level
                          </Button>
                          <Button
                            size="sm"
                            variant="outline"
                            className="text-orange-600"
                            onClick={() => handleAction(() => actions.deactivateMember(shopId, member.account))}
                          >
                            <ShieldOff className="mr-1 h-3 w-3" />Freeze
                          </Button>
                          <Button
                            size="sm"
                            variant="outline"
                            className="text-red-600"
                            onClick={() => handleAction(() => actions.banMember(shopId, member.account, banReason || null))}
                          >
                            <ShieldBan className="mr-1 h-3 w-3" />Ban
                          </Button>
                        </>
                      )}
                      {member.status === "Frozen" && shopId && (
                        <Button
                          size="sm"
                          onClick={() => handleAction(() => actions.activateMember(shopId, member.account))}
                        >
                          <UserCheck className="mr-1 h-3 w-3" />Activate
                        </Button>
                      )}
                      {member.status === "Banned" && shopId && (
                        <Button
                          size="sm"
                          onClick={() => handleAction(() => actions.unbanMember(shopId, member.account))}
                        >
                          <ShieldOff className="mr-1 h-3 w-3" />Unban
                        </Button>
                      )}
                      {member.status === "Pending" && shopId && (
                        <>
                          <Button size="sm" onClick={() => handleAction(() => actions.approveMember(shopId, member.account))}>
                            <UserCheck className="mr-1 h-3 w-3" />Approve
                          </Button>
                          <Button size="sm" variant="outline" onClick={() => handleAction(() => actions.rejectMember(shopId, member.account))}>
                            <UserX className="mr-1 h-3 w-3" />Reject
                          </Button>
                        </>
                      )}
                      {shopId && member.status !== "Pending" && (
                        <Button
                          size="sm"
                          variant="ghost"
                          className="text-red-600"
                          onClick={() => handleAction(() => actions.removeMember(shopId, member.account))}
                        >
                          <UserMinus className="h-3 w-3" />
                        </Button>
                      )}
                    </div>
                  </div>

                  {manualLevelTarget?.account === member.account && shopId && (
                    <div className="mt-3 flex items-center gap-2 rounded-lg bg-muted/50 p-3">
                      <span className="text-sm font-medium">Set Level:</span>
                      <div className="flex flex-wrap gap-2">
                        {levels.map((l) => (
                          <Button
                            key={l.id}
                            size="sm"
                            variant={manualLevelTarget.levelId === String(l.id) ? "default" : "outline"}
                            onClick={async () => {
                              await actions.manualUpgrade(shopId, member.account, l.id);
                              setManualLevelTarget(null);
                              refetch();
                            }}
                          >
                            {l.name || `Level ${l.id}`}
                          </Button>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Action completed!</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
