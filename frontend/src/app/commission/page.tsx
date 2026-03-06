"use client";

import { useEntityStore } from "@/stores/entity";
import { useCommissionConfig, useCommissionRecords, useWithdrawable, COMMISSION_MODE_BITS, COMMISSION_MODE_LABELS } from "@/hooks/useCommission";
import { useWalletStore } from "@/stores/wallet";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { formatBalance, basisPointsToPercent } from "@/lib/utils";
import {
  Percent, Layers, GitBranch, Users, Wallet, TrendingUp,
  CheckCircle, XCircle, Clock, Power, Settings, ChevronRight,
} from "lucide-react";
import { useTranslations } from "next-intl";
import Link from "next/link";

export default function CommissionPage() {
  const { currentEntityId } = useEntityStore();
  const { address } = useWalletStore();
  const { config, isLoading: configLoading } = useCommissionConfig(currentEntityId);
  const { records, isLoading: recordsLoading } = useCommissionRecords(currentEntityId);
  const { balance } = useWithdrawable(currentEntityId, address);
  const t = useTranslations("commission");
  const tc = useTranslations("common");

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const isLoading = configLoading || recordsLoading;

  const allModeBits = Object.entries(COMMISSION_MODE_BITS).filter(([k]) => k !== "NONE").map(([, v]) => v);
  const enabledModes = config ? allModeBits.filter((b) => !!(config.enabledModes & b)) : [];
  const enabledModeNames = enabledModes.map((b) => COMMISSION_MODE_LABELS[b]?.name).filter(Boolean);

  const totalDistributed = records.reduce((sum, r) => sum + BigInt(r.amount || 0), BigInt(0));
  const uniqueBeneficiaries = new Set(records.map((r) => r.beneficiary)).size;
  const pendingRecords = records.filter((r) => r.status === "Pending");

  const quickLinks = [
    { href: "/commission/config", label: "Configure Modes", desc: `${enabledModes.length} modes active`, icon: Settings },
    { href: "/commission/withdraw", label: "Withdraw", desc: `${formatBalance(balance.nex)} NEX available`, icon: Wallet },
    { href: "/commission/pool", label: "Reward Pool", desc: "Pool distribution rounds", icon: Layers },
  ];

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">{t("title")}</h1>
        <p className="text-muted-foreground">{t("subtitle")}</p>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
      ) : (
        <>
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
            <Card>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">Commission Status</CardTitle>
                <Power className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <div className="flex items-center gap-2">
                  <Badge variant={config?.enabled ? "default" : "secondary"}>
                    {config?.enabled ? "Enabled" : "Disabled"}
                  </Badge>
                  <span className="text-sm text-muted-foreground">{enabledModes.length} modes</span>
                </div>
                <div className="mt-2 flex flex-wrap gap-1">
                  {enabledModeNames.slice(0, 3).map((name) => (
                    <Badge key={name} variant="outline" className="text-[10px]">{name}</Badge>
                  ))}
                  {enabledModeNames.length > 3 && (
                    <Badge variant="outline" className="text-[10px]">+{enabledModeNames.length - 3}</Badge>
                  )}
                </div>
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">{t("totalDistributed")}</CardTitle>
                <TrendingUp className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <p className="text-2xl font-bold">{formatBalance(totalDistributed)} NEX</p>
                <p className="text-xs text-muted-foreground">{records.length} total records</p>
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">{t("activeReferrers")}</CardTitle>
                <Users className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <p className="text-2xl font-bold">{uniqueBeneficiaries}</p>
                <p className="text-xs text-muted-foreground">{pendingRecords.length} pending claims</p>
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">Your Balance</CardTitle>
                <Wallet className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <p className="text-2xl font-bold">{formatBalance(balance.nex)} NEX</p>
                {balance.token > 0 && <p className="text-xs text-muted-foreground">{formatBalance(balance.token)} Token</p>}
              </CardContent>
            </Card>
          </div>

          <div className="grid gap-3 md:grid-cols-3">
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

          {config && (
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2"><GitBranch className="h-5 w-5" />Commission Configuration</CardTitle>
                <CardDescription>Current commission modes and settings</CardDescription>
              </CardHeader>
              <CardContent className="space-y-3">
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Enabled</span>
                  <Badge variant={config.enabled ? "default" : "secondary"}>{config.enabled ? "Yes" : "No"}</Badge>
                </div>
                <Separator />
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Max Commission Rate</span>
                  <span className="text-sm font-medium">{basisPointsToPercent(config.maxCommissionRate)}</span>
                </div>
                <Separator />
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Creator Reward Rate</span>
                  <span className="text-sm font-medium">{basisPointsToPercent(config.creatorRewardRate)}</span>
                </div>
                <Separator />
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">NEX Withdrawal Cooldown</span>
                  <span className="text-sm font-medium">{config.withdrawalCooldown} blocks</span>
                </div>
                <Separator />
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Token Withdrawal Cooldown</span>
                  <span className="text-sm font-medium">{config.tokenWithdrawalCooldown} blocks</span>
                </div>
                <Separator />
                <div>
                  <span className="text-sm text-muted-foreground">Enabled Modes ({enabledModes.length})</span>
                  <div className="mt-2 flex flex-wrap gap-1">
                    {enabledModeNames.map((name) => (
                      <Badge key={name} variant="secondary" className="text-xs">{name}</Badge>
                    ))}
                    {enabledModeNames.length === 0 && <span className="text-xs text-muted-foreground">No modes enabled</span>}
                  </div>
                </div>
              </CardContent>
            </Card>
          )}

          <Card>
            <CardHeader>
              <CardTitle>Recent Commission Records</CardTitle>
              <CardDescription>{records.length} total records</CardDescription>
            </CardHeader>
            <CardContent>
              {records.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-8">
                  <Percent className="h-12 w-12 text-muted-foreground/50" />
                  <p className="mt-4 text-sm text-muted-foreground">No commission records yet.</p>
                </div>
              ) : (
                <div className="space-y-2">
                  {records.slice(0, 20).map((record) => (
                    <div key={record.id} className="flex items-center gap-4 rounded-lg border p-3">
                      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary/10">
                        {record.status === "Completed" || record.status === "Withdrawn"
                          ? <CheckCircle className="h-4 w-4 text-green-600" />
                          : record.status === "Cancelled"
                            ? <XCircle className="h-4 w-4 text-red-600" />
                            : <Clock className="h-4 w-4 text-yellow-600" />
                        }
                      </div>
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          <span className="text-sm font-medium">#{record.id}</span>
                          <Badge variant="outline" className="text-[10px]">{record.source}</Badge>
                          <StatusBadge status={record.status} />
                        </div>
                        <div className="flex items-center gap-2 text-xs text-muted-foreground">
                          <AddressDisplay address={record.beneficiary} chars={4} />
                          <span>&middot; Order #{record.orderId}</span>
                        </div>
                      </div>
                      <span className="text-sm font-semibold">{formatBalance(BigInt(record.amount || 0))} NEX</span>
                    </div>
                  ))}
                  {records.length > 20 && (
                    <p className="text-center text-xs text-muted-foreground pt-2">Showing 20 of {records.length} records</p>
                  )}
                </div>
              )}
            </CardContent>
          </Card>
        </>
      )}
    </div>
  );
}
