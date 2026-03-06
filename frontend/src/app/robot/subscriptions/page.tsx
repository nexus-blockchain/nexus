"use client";

import { useMemo, useState } from "react";
import Link from "next/link";
import { useTranslations } from "next-intl";
import {
  CreditCard,
  ArrowLeft,
  Plus,
  Pause,
  Play,
  XCircle,
  Megaphone,
  Settings2,
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogDescription,
} from "@/components/ui/dialog";
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from "@/components/ui/table";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { TxButton } from "@/components/shared/TxButton";
import { formatBalance, shortenAddress } from "@/lib/utils";
import { useWalletStore } from "@/stores/wallet";
import {
  useSubscriptions,
  useAdCommitments,
  useSubscriptionActions,
} from "@/hooks/useSubscription";

const SUBSCRIPTION_TIERS = ["Free", "Basic", "Pro", "Enterprise"];
const SUBSCRIPTION_STATUS = ["Active", "PastDue", "Suspended", "Cancelled", "Paused"];
const AD_COMMITMENT_STATUS = ["Active", "Underdelivery", "Cancelled"];

export default function RobotSubscriptionsPage() {
  const t = useTranslations("robot");
  const tCommon = useTranslations("common");
  const address = useWalletStore((s) => s.address);
  const { subscriptions, isLoading: subsLoading } = useSubscriptions();
  const { commitments, isLoading: commitmentsLoading } = useAdCommitments();
  const {
    subscribe,
    depositSubscription,
    cancelSubscription,
    changeTier,
    pauseSubscription,
    resumeSubscription,
    commitAds,
    cancelAdCommitment,
    updateAdCommitment,
    txState,
    resetTx,
  } = useSubscriptionActions();

  const mySubscriptions = useMemo(
    () => (address ? subscriptions.filter((s) => s.owner === address) : []),
    [subscriptions, address]
  );
  const myCommitments = useMemo(
    () => (address ? commitments.filter((c) => c.owner === address) : []),
    [commitments, address]
  );

  const toBigInt = (v: unknown) => BigInt(String(v ?? 0));

  const [subscribeOpen, setSubscribeOpen] = useState(false);
  const [subBotIdHash, setSubBotIdHash] = useState("");
  const [subTier, setSubTier] = useState("Basic");
  const [subDeposit, setSubDeposit] = useState("");

  const [depositOpen, setDepositOpen] = useState(false);
  const [depositBotIdHash, setDepositBotIdHash] = useState("");
  const [depositAmount, setDepositAmount] = useState("");

  const [changeTierOpen, setChangeTierOpen] = useState(false);
  const [changeTierBotIdHash, setChangeTierBotIdHash] = useState("");
  const [changeTierNew, setChangeTierNew] = useState("Pro");

  const [commitOpen, setCommitOpen] = useState(false);
  const [commitBotIdHash, setCommitBotIdHash] = useState("");
  const [commitCommunityIdHash, setCommitCommunityIdHash] = useState("");
  const [commitAdsPerEra, setCommitAdsPerEra] = useState("");

  const [updateCommitOpen, setUpdateCommitOpen] = useState(false);
  const [updateCommitBotIdHash, setUpdateCommitBotIdHash] = useState("");
  const [updateCommitAdsPerEra, setUpdateCommitAdsPerEra] = useState("");
  const [updateCommitCommunityIdHash, setUpdateCommitCommunityIdHash] = useState("");

  const handleSubscribe = () => {
    if (!subBotIdHash || !subDeposit) return;
    const amount = BigInt(subDeposit);
    subscribe(subBotIdHash, subTier, amount);
    setSubscribeOpen(false);
    setSubBotIdHash("");
    setSubTier("Basic");
    setSubDeposit("");
  };

  const handleDeposit = () => {
    if (!depositBotIdHash || !depositAmount) return;
    depositSubscription(depositBotIdHash, BigInt(depositAmount));
    setDepositOpen(false);
    setDepositBotIdHash("");
    setDepositAmount("");
  };

  const handleChangeTier = () => {
    if (!changeTierBotIdHash) return;
    changeTier(changeTierBotIdHash, changeTierNew);
    setChangeTierOpen(false);
    setChangeTierBotIdHash("");
    setChangeTierNew("Pro");
  };

  const handleCommitAds = () => {
    if (!commitBotIdHash || !commitCommunityIdHash || !commitAdsPerEra) return;
    commitAds(commitBotIdHash, commitCommunityIdHash, parseInt(commitAdsPerEra, 10));
    setCommitOpen(false);
    setCommitBotIdHash("");
    setCommitCommunityIdHash("");
    setCommitAdsPerEra("");
  };

  const handleUpdateCommitment = () => {
    if (!updateCommitBotIdHash || !updateCommitAdsPerEra) return;
    updateAdCommitment(
      updateCommitBotIdHash,
      parseInt(updateCommitAdsPerEra, 10),
      updateCommitCommunityIdHash || null
    );
    setUpdateCommitOpen(false);
    setUpdateCommitBotIdHash("");
    setUpdateCommitAdsPerEra("");
    setUpdateCommitCommunityIdHash("");
  };

  return (
    <div className="space-y-8">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/robot">
            <ArrowLeft className="h-4 w-4" />
          </Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <CreditCard className="h-7 w-7" />
            {t("subscriptions.title")}
          </h1>
          <p className="text-muted-foreground">{t("subscriptions.subtitle")}</p>
        </div>
        <Button onClick={() => setSubscribeOpen(true)}>
          <Plus className="mr-2 h-4 w-4" />
          {t("subscriptions.subscribe")}
        </Button>
      </div>

      {/* Subscriptions Section */}
      <Card>
        <CardHeader>
          <CardTitle>{t("subscriptions.sectionTitle")}</CardTitle>
          <CardDescription>{t("subscriptions.sectionDesc")}</CardDescription>
        </CardHeader>
        <CardContent>
          {!address ? (
            <div className="flex flex-col items-center justify-center py-12 text-muted-foreground">
              <CreditCard className="h-12 w-12 mb-4 opacity-50" />
              <p>{t("connectWallet")}</p>
            </div>
          ) : subsLoading ? (
            <div className="flex justify-center py-12">
              <div className="h-8 w-8 animate-spin rounded-full border-2 border-primary border-t-transparent" />
            </div>
          ) : mySubscriptions.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12">
              <CreditCard className="h-12 w-12 text-muted-foreground/50" />
              <p className="mt-4 text-lg font-medium">{t("subscriptions.noSubscriptions")}</p>
              <p className="text-sm text-muted-foreground">{t("subscriptions.noSubscriptionsDesc")}</p>
              <Button className="mt-4" onClick={() => setSubscribeOpen(true)}>
                <Plus className="mr-2 h-4 w-4" />
                {t("subscriptions.subscribe")}
              </Button>
            </div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("subscriptions.botHash")}</TableHead>
                  <TableHead>{t("subscriptions.tier")}</TableHead>
                  <TableHead className="text-right">{t("subscriptions.feePerEra")}</TableHead>
                  <TableHead>{t("status")}</TableHead>
                  <TableHead>{t("subscriptions.started")}</TableHead>
                  <TableHead className="text-right">{t("actions")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {mySubscriptions.map((s) => (
                  <TableRow key={s.botIdHash}>
                    <TableCell className="font-mono text-sm">
                      {shortenAddress(s.botIdHash, 10)}
                    </TableCell>
                    <TableCell>{s.tier}</TableCell>
                    <TableCell className="text-right font-mono">
                      {formatBalance(toBigInt(s.feePerEra))}
                    </TableCell>
                    <TableCell>
                      <StatusBadge status={s.status} />
                    </TableCell>
                    <TableCell className="text-muted-foreground">
                      {new Date(s.startedAt * 1000).toLocaleDateString()}
                    </TableCell>
                    <TableCell className="text-right">
                      <div className="flex items-center justify-end gap-2 flex-wrap">
                        {s.status === "Active" && (
                          <>
                            <Button
                              size="sm"
                              variant="outline"
                              onClick={() => {
                                setDepositBotIdHash(s.botIdHash);
                                setDepositAmount("");
                                setDepositOpen(true);
                              }}
                            >
                              {t("subscriptions.deposit")}
                            </Button>
                            <Button
                              size="sm"
                              variant="outline"
                              onClick={() => {
                                setChangeTierBotIdHash(s.botIdHash);
                                setChangeTierNew(s.tier);
                                setChangeTierOpen(true);
                              }}
                            >
                              {t("subscriptions.changeTier")}
                            </Button>
                            <TxButton
                              size="sm"
                              variant="outline"
                              txStatus={txState.status}
                              onClick={() => pauseSubscription(s.botIdHash)}
                            >
                              <Pause className="h-4 w-4" />
                            </TxButton>
                          </>
                        )}
                        {s.status === "Paused" && (
                          <TxButton
                            size="sm"
                            variant="outline"
                            txStatus={txState.status}
                            onClick={() => resumeSubscription(s.botIdHash)}
                          >
                            <Play className="h-4 w-4" />
                          </TxButton>
                        )}
                        {(s.status === "Active" || s.status === "Paused" || s.status === "PastDue") && (
                          <TxButton
                            size="sm"
                            variant="destructive"
                            txStatus={txState.status}
                            onClick={() => cancelSubscription(s.botIdHash)}
                          >
                            <XCircle className="h-4 w-4" />
                          </TxButton>
                        )}
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      {/* Ad Commitments Section */}
      <Card>
        <CardHeader>
          <CardTitle>{t("subscriptions.adCommitments")}</CardTitle>
          <CardDescription>{t("subscriptions.adCommitmentsDesc")}</CardDescription>
          <div className="pt-2">
            <Button size="sm" variant="outline" onClick={() => setCommitOpen(true)}>
              <Megaphone className="mr-2 h-4 w-4" />
              {t("subscriptions.commitAds")}
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          {!address ? (
            <div className="flex flex-col items-center justify-center py-12 text-muted-foreground">
              <Megaphone className="h-12 w-12 mb-4 opacity-50" />
              <p>{t("connectWallet")}</p>
            </div>
          ) : commitmentsLoading ? (
            <div className="flex justify-center py-12">
              <div className="h-8 w-8 animate-spin rounded-full border-2 border-primary border-t-transparent" />
            </div>
          ) : myCommitments.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12">
              <Megaphone className="h-12 w-12 text-muted-foreground/50" />
              <p className="mt-4 text-lg font-medium">{t("subscriptions.noCommitments")}</p>
              <p className="text-sm text-muted-foreground">{t("subscriptions.noCommitmentsDesc")}</p>
              <Button className="mt-4" variant="outline" onClick={() => setCommitOpen(true)}>
                <Megaphone className="mr-2 h-4 w-4" />
                {t("subscriptions.commitAds")}
              </Button>
            </div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("subscriptions.botHash")}</TableHead>
                  <TableHead>{t("subscriptions.communityHash")}</TableHead>
                  <TableHead className="text-right">{t("subscriptions.adsPerEra")}</TableHead>
                  <TableHead>{t("subscriptions.tier")}</TableHead>
                  <TableHead>{t("subscriptions.underdeliveryEras")}</TableHead>
                  <TableHead>{t("status")}</TableHead>
                  <TableHead className="text-right">{t("actions")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {myCommitments.map((c) => (
                  <TableRow key={`${c.botIdHash}-${c.communityIdHash}`}>
                    <TableCell className="font-mono text-sm">
                      {shortenAddress(c.botIdHash, 10)}
                    </TableCell>
                    <TableCell className="font-mono text-sm">
                      {shortenAddress(c.communityIdHash, 10)}
                    </TableCell>
                    <TableCell className="text-right font-mono">
                      {c.committedAdsPerEra}
                    </TableCell>
                    <TableCell>{c.effectiveTier}</TableCell>
                    <TableCell>{c.underdeliveryEras}</TableCell>
                    <TableCell>
                      <StatusBadge status={c.status} />
                    </TableCell>
                    <TableCell className="text-right">
                      <div className="flex items-center justify-end gap-2">
                        {c.status !== "Cancelled" && (
                          <>
                            <Button
                              size="sm"
                              variant="outline"
                              onClick={() => {
                                setUpdateCommitBotIdHash(c.botIdHash);
                                setUpdateCommitAdsPerEra(String(c.committedAdsPerEra));
                                setUpdateCommitCommunityIdHash(c.communityIdHash);
                                setUpdateCommitOpen(true);
                              }}
                            >
                              {t("subscriptions.update")}
                            </Button>
                            <TxButton
                              size="sm"
                              variant="destructive"
                              txStatus={txState.status}
                              onClick={() => cancelAdCommitment(c.botIdHash)}
                            >
                              {t("subscriptions.cancel")}
                            </TxButton>
                          </>
                        )}
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      {/* Subscribe Dialog */}
      <Dialog open={subscribeOpen} onOpenChange={setSubscribeOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("subscriptions.subscribe")}</DialogTitle>
            <DialogDescription>{t("subscriptions.subscribeDesc")}</DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div>
              <label className="text-sm font-medium">{t("botIdHash")}</label>
              <Input
                className="mt-1"
                value={subBotIdHash}
                onChange={(e) => setSubBotIdHash(e.target.value)}
                placeholder={t("botIdHashPlaceholder")}
              />
            </div>
            <div>
              <label className="text-sm font-medium">{t("subscriptions.tier")}</label>
              <select
                className="mt-1 w-full rounded-md border px-3 py-2 text-sm"
                value={subTier}
                onChange={(e) => setSubTier(e.target.value)}
              >
                {SUBSCRIPTION_TIERS.map((tier) => (
                  <option key={tier} value={tier}>
                    {tier}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="text-sm font-medium">{t("subscriptions.deposit")}</label>
              <Input
                type="text"
                className="mt-1"
                value={subDeposit}
                onChange={(e) => setSubDeposit(e.target.value)}
                placeholder="0"
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setSubscribeOpen(false)}>
              {tCommon("cancel")}
            </Button>
            <TxButton
              txStatus={txState.status}
              onClick={handleSubscribe}
              disabled={!subBotIdHash || !subDeposit}
            >
              {t("subscriptions.submit")}
            </TxButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Deposit Dialog */}
      <Dialog open={depositOpen} onOpenChange={setDepositOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("subscriptions.deposit")}</DialogTitle>
            <DialogDescription>{t("subscriptions.depositDesc")}</DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div>
              <label className="text-sm font-medium">{t("subscriptions.botHash")}</label>
              <Input
                className="mt-1"
                value={depositBotIdHash}
                onChange={(e) => setDepositBotIdHash(e.target.value)}
                placeholder={t("botIdHashPlaceholder")}
              />
            </div>
            <div>
              <label className="text-sm font-medium">{tCommon("amount")}</label>
              <Input
                type="text"
                className="mt-1"
                value={depositAmount}
                onChange={(e) => setDepositAmount(e.target.value)}
                placeholder="0"
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDepositOpen(false)}>
              {tCommon("cancel")}
            </Button>
            <TxButton
              txStatus={txState.status}
              onClick={handleDeposit}
              disabled={!depositBotIdHash || !depositAmount}
            >
              {t("subscriptions.submit")}
            </TxButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Change Tier Dialog */}
      <Dialog open={changeTierOpen} onOpenChange={setChangeTierOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("subscriptions.changeTier")}</DialogTitle>
            <DialogDescription>{t("subscriptions.changeTierDesc")}</DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div>
              <label className="text-sm font-medium">{t("subscriptions.botHash")}</label>
              <Input
                className="mt-1"
                value={changeTierBotIdHash}
                onChange={(e) => setChangeTierBotIdHash(e.target.value)}
                placeholder={t("botIdHashPlaceholder")}
              />
            </div>
            <div>
              <label className="text-sm font-medium">{t("subscriptions.newTier")}</label>
              <select
                className="mt-1 w-full rounded-md border px-3 py-2 text-sm"
                value={changeTierNew}
                onChange={(e) => setChangeTierNew(e.target.value)}
              >
                {SUBSCRIPTION_TIERS.map((tier) => (
                  <option key={tier} value={tier}>
                    {tier}
                  </option>
                ))}
              </select>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setChangeTierOpen(false)}>
              {tCommon("cancel")}
            </Button>
            <TxButton
              txStatus={txState.status}
              onClick={handleChangeTier}
              disabled={!changeTierBotIdHash}
            >
              {t("subscriptions.submit")}
            </TxButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Commit Ads Dialog */}
      <Dialog open={commitOpen} onOpenChange={setCommitOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("subscriptions.commitAds")}</DialogTitle>
            <DialogDescription>{t("subscriptions.commitAdsDesc")}</DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div>
              <label className="text-sm font-medium">{t("botIdHash")}</label>
              <Input
                className="mt-1"
                value={commitBotIdHash}
                onChange={(e) => setCommitBotIdHash(e.target.value)}
                placeholder={t("botIdHashPlaceholder")}
              />
            </div>
            <div>
              <label className="text-sm font-medium">{t("communityIdHash")}</label>
              <Input
                className="mt-1"
                value={commitCommunityIdHash}
                onChange={(e) => setCommitCommunityIdHash(e.target.value)}
                placeholder={t("communityIdHashPlaceholder")}
              />
            </div>
            <div>
              <label className="text-sm font-medium">{t("subscriptions.committedAdsPerEra")}</label>
              <Input
                type="number"
                className="mt-1"
                value={commitAdsPerEra}
                onChange={(e) => setCommitAdsPerEra(e.target.value)}
                placeholder="0"
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setCommitOpen(false)}>
              {tCommon("cancel")}
            </Button>
            <TxButton
              txStatus={txState.status}
              onClick={handleCommitAds}
              disabled={!commitBotIdHash || !commitCommunityIdHash || !commitAdsPerEra}
            >
              {t("subscriptions.submit")}
            </TxButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Update Commitment Dialog */}
      <Dialog open={updateCommitOpen} onOpenChange={setUpdateCommitOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("subscriptions.update")}</DialogTitle>
            <DialogDescription>{t("subscriptions.updateCommitDesc")}</DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div>
              <label className="text-sm font-medium">{t("subscriptions.botHash")}</label>
              <Input
                className="mt-1"
                value={updateCommitBotIdHash}
                onChange={(e) => setUpdateCommitBotIdHash(e.target.value)}
                placeholder={t("botIdHashPlaceholder")}
              />
            </div>
            <div>
              <label className="text-sm font-medium">{t("subscriptions.committedAdsPerEra")}</label>
              <Input
                type="number"
                className="mt-1"
                value={updateCommitAdsPerEra}
                onChange={(e) => setUpdateCommitAdsPerEra(e.target.value)}
                placeholder="0"
              />
            </div>
            <div>
              <label className="text-sm font-medium">{t("communityIdHash")} (optional)</label>
              <Input
                className="mt-1"
                value={updateCommitCommunityIdHash}
                onChange={(e) => setUpdateCommitCommunityIdHash(e.target.value)}
                placeholder={t("communityIdHashPlaceholder")}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setUpdateCommitOpen(false)}>
              {tCommon("cancel")}
            </Button>
            <TxButton
              txStatus={txState.status}
              onClick={handleUpdateCommitment}
              disabled={!updateCommitBotIdHash || !updateCommitAdsPerEra}
            >
              {t("subscriptions.submit")}
            </TxButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
