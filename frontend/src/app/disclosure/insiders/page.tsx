"use client";

import { useState, useMemo } from "react";
import { useEntityStore } from "@/stores/entity";
import {
  useInsiders,
  useBlackout,
  useDisclosureActions,
} from "@/hooks/useDisclosure";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { TxButton } from "@/components/shared/TxButton";
import { Separator } from "@/components/ui/separator";
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from "@/components/ui/table";
import {
  ArrowLeft,
  UserPlus,
  Trash2,
  ShieldAlert,
  Users,
  RotateCcw,
  Clock,
  Ban,
  CheckSquare,
  Square,
  UserCog,
  Shield,
  Eye,
  Briefcase,
  Crown,
  Landmark,
} from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";
import { INSIDER_ROLES } from "@/lib/constants";
import type { InsiderRecord } from "@/lib/types";

const ROLE_CONFIG: Record<string, { icon: typeof Shield; color: string }> = {
  Owner: {
    icon: Crown,
    color: "bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400",
  },
  Admin: {
    icon: Shield,
    color: "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400",
  },
  Auditor: {
    icon: Eye,
    color: "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400",
  },
  Advisor: {
    icon: Briefcase,
    color: "bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400",
  },
  MajorHolder: {
    icon: Landmark,
    color: "bg-cyan-100 text-cyan-700 dark:bg-cyan-900/30 dark:text-cyan-400",
  },
};

export default function InsidersPage() {
  const { currentEntityId } = useEntityStore();
  const { insiders, isLoading, refetch } = useInsiders(currentEntityId);
  const { blackout, isLoading: blackoutLoading } = useBlackout(currentEntityId);
  const actions = useDisclosureActions();
  const tc = useTranslations("common");

  const [newAddr, setNewAddr] = useState("");
  const [newRole, setNewRole] = useState<string>("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [editingRole, setEditingRole] = useState<string | null>(null);
  const [editRoleValue, setEditRoleValue] = useState<string>("");

  const roleCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const role of INSIDER_ROLES) counts[role] = 0;
    for (const ins of insiders) {
      counts[ins.role] = (counts[ins.role] || 0) + 1;
    }
    return counts;
  }, [insiders]);

  if (!currentEntityId) {
    return (
      <div className="flex h-full items-center justify-center text-muted-foreground">
        {tc("selectEntity")}
      </div>
    );
  }

  const handleAdd = () => {
    if (!newAddr.trim() || !newRole) return;
    actions.addInsider(currentEntityId, newAddr.trim(), newRole);
    setNewAddr("");
    setNewRole("");
  };

  const handleRemove = (account: string) => {
    actions.removeInsider(currentEntityId, account);
  };

  const handleUpdateRole = (account: string) => {
    if (!editRoleValue) return;
    actions.updateInsiderRole(currentEntityId, account, editRoleValue);
    setEditingRole(null);
    setEditRoleValue("");
  };

  const toggleSelect = (account: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(account)) next.delete(account);
      else next.add(account);
      return next;
    });
  };

  const toggleSelectAll = () => {
    if (selected.size === insiders.length) {
      setSelected(new Set());
    } else {
      setSelected(new Set(insiders.map((i: InsiderRecord) => i.account)));
    }
  };

  const handleBatchRemove = () => {
    if (selected.size === 0 || !currentEntityId) return;
    actions.batchRemoveInsiders(currentEntityId, Array.from(selected));
    setSelected(new Set());
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/disclosure">
            <ArrowLeft className="h-4 w-4" />
          </Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight">
            Insider Management
          </h1>
          <p className="text-muted-foreground">
            Manage insider list, roles, and blackout periods for compliance
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={refetch}>
          <RotateCcw className="mr-2 h-3 w-3" />
          Refresh
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-3 lg:grid-cols-6">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Total Insiders</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">{insiders.length}</p>
          </CardContent>
        </Card>
        {INSIDER_ROLES.map((role) => {
          const cfg = ROLE_CONFIG[role];
          return (
            <Card key={role}>
              <CardHeader className="pb-2">
                <CardTitle className="text-sm font-medium flex items-center gap-1.5">
                  {cfg && <cfg.icon className="h-3.5 w-3.5" />}
                  {role === "MajorHolder" ? "Major Holder" : role}
                </CardTitle>
              </CardHeader>
              <CardContent>
                <p className="text-2xl font-bold">{roleCounts[role] || 0}</p>
              </CardContent>
            </Card>
          );
        })}
      </div>

      {blackout && (
        <Card className="border-amber-300 dark:border-amber-700">
          <CardContent className="py-4">
            <div className="flex items-center gap-3">
              <div className="rounded-full bg-amber-100 p-2 dark:bg-amber-900/30">
                <Ban className="h-5 w-5 text-amber-600 dark:text-amber-400" />
              </div>
              <div className="flex-1">
                <p className="font-semibold text-amber-700 dark:text-amber-400">
                  Blackout Period Active
                </p>
                <p className="text-sm text-muted-foreground">
                  Block #{blackout.start} → Block #{blackout.end}
                </p>
              </div>
              <Badge
                variant="outline"
                className="border-amber-300 text-amber-700 dark:border-amber-700 dark:text-amber-400"
              >
                Trading Restricted
              </Badge>
            </div>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <UserPlus className="h-5 w-5" />
            Add Insider
          </CardTitle>
          <CardDescription>
            Register an insider with their role for disclosure compliance
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-4 md:grid-cols-2">
            <div className="space-y-2">
              <label className="text-sm font-medium">Address</label>
              <Input
                value={newAddr}
                onChange={(e) => setNewAddr(e.target.value)}
                placeholder="5xxx..."
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Role</label>
              <div className="grid grid-cols-5 gap-2">
                {INSIDER_ROLES.map((role) => {
                  const cfg = ROLE_CONFIG[role];
                  return (
                    <button
                      key={role}
                      onClick={() => setNewRole(role)}
                      className={`flex flex-col items-center gap-1 rounded-lg border p-2.5 text-xs font-medium transition-colors hover:bg-accent ${
                        newRole === role
                          ? "border-primary bg-primary/5 ring-1 ring-primary"
                          : "border-border"
                      }`}
                    >
                      {cfg && <cfg.icon className="h-4 w-4" />}
                      {role === "MajorHolder" ? "Major" : role}
                    </button>
                  );
                })}
              </div>
            </div>
          </div>
          <TxButton
            onClick={handleAdd}
            txStatus={actions.txState.status}
            disabled={!newAddr.trim() || !newRole}
          >
            <UserPlus className="mr-2 h-4 w-4" />
            Add Insider
          </TxButton>
        </CardContent>
      </Card>

      {isLoading ? (
        <div className="flex justify-center py-12">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
        </div>
      ) : insiders.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Users className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No Insiders Registered</p>
            <p className="text-sm text-muted-foreground">
              Add insiders above for compliance tracking.
            </p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardHeader>
            <div className="flex items-center justify-between">
              <CardTitle className="flex items-center gap-2">
                <ShieldAlert className="h-5 w-5" />
                Registered Insiders
              </CardTitle>
              {selected.size > 0 && (
                <div className="flex items-center gap-2">
                  <span className="text-sm text-muted-foreground">
                    {selected.size} selected
                  </span>
                  <TxButton
                    size="sm"
                    variant="destructive"
                    onClick={handleBatchRemove}
                    txStatus={actions.txState.status}
                  >
                    <Trash2 className="mr-2 h-3 w-3" />
                    Remove Selected
                  </TxButton>
                </div>
              )}
            </div>
          </CardHeader>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-12">
                  <button
                    onClick={toggleSelectAll}
                    className="text-muted-foreground hover:text-foreground transition-colors"
                  >
                    {selected.size === insiders.length && insiders.length > 0 ? (
                      <CheckSquare className="h-4 w-4" />
                    ) : (
                      <Square className="h-4 w-4" />
                    )}
                  </button>
                </TableHead>
                <TableHead>Address</TableHead>
                <TableHead>Role</TableHead>
                <TableHead>Added</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {insiders.map((insider: InsiderRecord) => {
                const cfg = ROLE_CONFIG[insider.role];
                const isEditing = editingRole === insider.account;

                return (
                  <TableRow key={insider.account}>
                    <TableCell>
                      <button
                        onClick={() => toggleSelect(insider.account)}
                        className="text-muted-foreground hover:text-foreground transition-colors"
                      >
                        {selected.has(insider.account) ? (
                          <CheckSquare className="h-4 w-4 text-primary" />
                        ) : (
                          <Square className="h-4 w-4" />
                        )}
                      </button>
                    </TableCell>
                    <TableCell>
                      <AddressDisplay address={insider.account} />
                    </TableCell>
                    <TableCell>
                      {isEditing ? (
                        <div className="flex items-center gap-2">
                          <select
                            value={editRoleValue}
                            onChange={(e) => setEditRoleValue(e.target.value)}
                            className="rounded-md border bg-background px-2 py-1 text-sm"
                          >
                            <option value="">Select role</option>
                            {INSIDER_ROLES.map((r) => (
                              <option key={r} value={r}>
                                {r}
                              </option>
                            ))}
                          </select>
                          <Button
                            size="sm"
                            variant="ghost"
                            onClick={() =>
                              handleUpdateRole(insider.account)
                            }
                            disabled={!editRoleValue}
                          >
                            Save
                          </Button>
                          <Button
                            size="sm"
                            variant="ghost"
                            onClick={() => {
                              setEditingRole(null);
                              setEditRoleValue("");
                            }}
                          >
                            Cancel
                          </Button>
                        </div>
                      ) : (
                        <span
                          className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-semibold ${
                            cfg?.color || "bg-gray-100 text-gray-700"
                          }`}
                        >
                          {cfg && <cfg.icon className="h-3 w-3" />}
                          {insider.role === "MajorHolder"
                            ? "Major Holder"
                            : insider.role}
                        </span>
                      )}
                    </TableCell>
                    <TableCell className="text-sm text-muted-foreground">
                      <span className="flex items-center gap-1">
                        <Clock className="h-3 w-3" />
                        Block #{insider.addedAt}
                      </span>
                    </TableCell>
                    <TableCell className="text-right">
                      <div className="flex items-center justify-end gap-1">
                        {!isEditing && (
                          <Button
                            variant="ghost"
                            size="icon"
                            onClick={() => {
                              setEditingRole(insider.account);
                              setEditRoleValue(insider.role);
                            }}
                            title="Update Role"
                          >
                            <UserCog className="h-4 w-4" />
                          </Button>
                        )}
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => handleRemove(insider.account)}
                          title="Remove"
                        >
                          <Trash2 className="h-4 w-4 text-destructive" />
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </Card>
      )}

      {actions.txState.status === "finalized" && (
        <p className="text-sm text-green-600">
          Transaction finalized successfully.
        </p>
      )}
      {actions.txState.status === "error" && (
        <p className="text-sm text-destructive">{actions.txState.error}</p>
      )}
    </div>
  );
}
