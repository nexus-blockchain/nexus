"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import {
  useKycProviders, useAuthorizedProviders, useKycActions,
} from "@/hooks/useKyc";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Select } from "@/components/ui/select";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { TxButton } from "@/components/shared/TxButton";
import { Separator } from "@/components/ui/separator";
import {
  Table, TableHeader, TableBody, TableRow, TableHead, TableCell,
} from "@/components/ui/table";
import { KYC_LEVELS, PROVIDER_TYPES } from "@/lib/constants";
import {
  ArrowLeft, Plus, Trash2, ShieldCheck, Users,
  PauseCircle, PlayCircle, Link2, Unlink,
} from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const TYPE_COLORS: Record<string, string> = {
  Internal: "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400",
  ThirdParty: "bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400",
  Government: "bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400",
  Financial: "bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400",
};

export default function KycProvidersPage() {
  const { currentEntityId } = useEntityStore();
  const { providers, isLoading: providersLoading, refetch: refetchProviders } = useKycProviders();
  const { authorized, isLoading: authLoading, refetch: refetchAuth } = useAuthorizedProviders(currentEntityId);
  const actions = useKycActions();
  const tc = useTranslations("common");

  const [regAccount, setRegAccount] = useState("");
  const [regName, setRegName] = useState("");
  const [regType, setRegType] = useState<string>(PROVIDER_TYPES[0]);
  const [regMaxLevel, setRegMaxLevel] = useState<string>(KYC_LEVELS[1]);

  const [authProvider, setAuthProvider] = useState("");

  if (!currentEntityId) {
    return (
      <div className="flex h-full items-center justify-center text-muted-foreground">
        {tc("selectEntity")}
      </div>
    );
  }

  const handleRegister = () => {
    if (!regAccount.trim() || !regName.trim()) return;
    actions.registerProvider(regAccount, regName, regType, regMaxLevel);
    setRegAccount("");
    setRegName("");
    setRegType(PROVIDER_TYPES[0]);
    setRegMaxLevel(KYC_LEVELS[1]);
  };

  const handleAuthorize = () => {
    if (!authProvider) return;
    actions.authorizeProvider(currentEntityId, authProvider);
    setAuthProvider("");
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/kyc"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">KYC Providers</h1>
          <p className="text-muted-foreground">
            Manage global verification providers and entity authorizations
          </p>
        </div>
      </div>

      {/* ── Register Provider ─────────────────────────────────── */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Plus className="h-5 w-5" />Register Provider
          </CardTitle>
          <CardDescription>Register a new global KYC verification provider</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Provider Account</label>
              <Input
                value={regAccount}
                onChange={(e) => setRegAccount(e.target.value)}
                placeholder="5xxx…"
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Name</label>
              <Input
                value={regName}
                onChange={(e) => setRegName(e.target.value)}
                placeholder="e.g. Jumio"
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Provider Type</label>
              <Select value={regType} onChange={(e) => setRegType(e.target.value)}>
                {PROVIDER_TYPES.map((t) => (
                  <option key={t} value={t}>{t}</option>
                ))}
              </Select>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Max Level</label>
              <Select value={regMaxLevel} onChange={(e) => setRegMaxLevel(e.target.value)}>
                {KYC_LEVELS.filter((l) => l !== "None").map((l) => (
                  <option key={l} value={l}>{l}</option>
                ))}
              </Select>
            </div>
          </div>
          <TxButton
            onClick={handleRegister}
            txStatus={actions.txState.status}
            disabled={!regAccount.trim() || !regName.trim()}
          >
            <Plus className="mr-2 h-4 w-4" />Register Provider
          </TxButton>
        </CardContent>
      </Card>

      {/* ── Global Providers Table ────────────────────────────── */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <ShieldCheck className="h-5 w-5" />Global Providers
          </CardTitle>
          <CardDescription>
            All registered KYC verification providers across the network
          </CardDescription>
        </CardHeader>
        {providersLoading ? (
          <CardContent>
            <div className="flex justify-center py-8">
              <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
            </div>
          </CardContent>
        ) : providers.length === 0 ? (
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Users className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No Providers Registered</p>
            <p className="text-sm text-muted-foreground">
              Register a provider to enable KYC verification.
            </p>
          </CardContent>
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Provider</TableHead>
                <TableHead>Type</TableHead>
                <TableHead>Max Level</TableHead>
                <TableHead>Verifications</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {providers.map((p) => (
                <TableRow key={p.account}>
                  <TableCell>
                    <div>
                      <p className="font-medium">{p.name}</p>
                      <AddressDisplay address={p.account} chars={4} />
                    </div>
                  </TableCell>
                  <TableCell>
                    <Badge
                      className={TYPE_COLORS[p.providerType] || ""}
                      variant="outline"
                    >
                      {p.providerType}
                    </Badge>
                  </TableCell>
                  <TableCell>{p.maxLevel}</TableCell>
                  <TableCell className="font-mono">{p.verificationsCount}</TableCell>
                  <TableCell>
                    <StatusBadge status={p.suspended ? "Suspended" : "Active"} />
                  </TableCell>
                  <TableCell className="text-right">
                    <div className="flex justify-end gap-1">
                      {p.suspended ? (
                        <Button
                          variant="ghost"
                          size="icon"
                          title="Resume"
                          onClick={() => actions.resumeProvider(p.account)}
                        >
                          <PlayCircle className="h-4 w-4 text-green-600" />
                        </Button>
                      ) : (
                        <Button
                          variant="ghost"
                          size="icon"
                          title="Suspend"
                          onClick={() => actions.suspendProvider(p.account)}
                        >
                          <PauseCircle className="h-4 w-4 text-yellow-600" />
                        </Button>
                      )}
                      <Button
                        variant="ghost"
                        size="icon"
                        title="Remove"
                        onClick={() => actions.removeProvider(p.account)}
                      >
                        <Trash2 className="h-4 w-4 text-destructive" />
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </Card>

      <Separator />

      {/* ── Entity Authorized Providers ───────────────────────── */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Link2 className="h-5 w-5" />Entity Authorized Providers
          </CardTitle>
          <CardDescription>
            Providers authorized to perform KYC verification for this entity
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex gap-3">
            <div className="flex-1">
              <Select value={authProvider} onChange={(e) => setAuthProvider(e.target.value)}>
                <option value="">Select a provider to authorize…</option>
                {providers
                  .filter((p) => !authorized.includes(p.account) && !p.suspended)
                  .map((p) => (
                    <option key={p.account} value={p.account}>
                      {p.name} ({p.account.slice(0, 8)}…)
                    </option>
                  ))}
              </Select>
            </div>
            <TxButton
              onClick={handleAuthorize}
              txStatus={actions.txState.status}
              disabled={!authProvider}
            >
              <Link2 className="mr-2 h-4 w-4" />Authorize
            </TxButton>
          </div>

          <Separator />

          {authLoading ? (
            <div className="flex justify-center py-4">
              <div className="h-6 w-6 animate-spin rounded-full border-4 border-primary border-t-transparent" />
            </div>
          ) : authorized.length === 0 ? (
            <p className="text-sm text-muted-foreground py-4 text-center">
              No providers authorized for this entity.
            </p>
          ) : (
            <div className="space-y-2">
              {authorized.map((addr) => {
                const info = providers.find((p) => p.account === addr);
                return (
                  <div
                    key={addr}
                    className="flex items-center justify-between rounded-lg border p-3"
                  >
                    <div className="flex items-center gap-3">
                      <AddressDisplay address={addr} />
                      {info && (
                        <>
                          <span className="text-sm font-medium">{info.name}</span>
                          <Badge
                            className={TYPE_COLORS[info.providerType] || ""}
                            variant="outline"
                          >
                            {info.providerType}
                          </Badge>
                        </>
                      )}
                    </div>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => actions.deauthorizeProvider(currentEntityId, addr)}
                    >
                      <Unlink className="mr-1.5 h-3.5 w-3.5" />Deauthorize
                    </Button>
                  </div>
                );
              })}
            </div>
          )}
        </CardContent>
      </Card>

      {actions.txState.status === "finalized" && (
        <p className="text-sm text-green-600">Action completed successfully.</p>
      )}
      {actions.txState.status === "error" && (
        <p className="text-sm text-destructive">{actions.txState.error}</p>
      )}
    </div>
  );
}
