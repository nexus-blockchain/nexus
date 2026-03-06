"use client";

import { useState, useMemo } from "react";
import { useEntityStore } from "@/stores/entity";
import { useWalletStore } from "@/stores/wallet";
import {
  useTokenLocks,
  useTokenActions,
  useVestingSchedule,
} from "@/hooks/useToken";
import { useApi } from "@/hooks/useApi";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { Progress } from "@/components/ui/progress";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { TxButton } from "@/components/shared/TxButton";
import { formatBalance } from "@/lib/utils";
import {
  Lock,
  Unlock,
  Clock,
  CalendarClock,
  Timer,
  Shield,
  Coins,
} from "lucide-react";
import { useTranslations } from "next-intl";

export default function TokenLockPage() {
  const { currentEntityId } = useEntityStore();
  const { address } = useWalletStore();
  const { locks, isLoading: locksLoading, refetch: refetchLocks } = useTokenLocks(currentEntityId);
  const { schedule, isLoading: vestingLoading } = useVestingSchedule(
    currentEntityId,
    address
  );
  const actions = useTokenActions();
  const { chainInfo } = useApi();
  const tc = useTranslations("common");
  const currentBlock = chainInfo.bestBlock;

  const [lockUser, setLockUser] = useState("");
  const [lockAmount, setLockAmount] = useState("");
  const [unlockAt, setUnlockAt] = useState("");

  const [vestBeneficiary, setVestBeneficiary] = useState("");
  const [vestTotal, setVestTotal] = useState("");
  const [vestStart, setVestStart] = useState("");
  const [vestCliff, setVestCliff] = useState("");
  const [vestDuration, setVestDuration] = useState("");

  const lockStats = useMemo(() => {
    const totalLocked = locks.reduce((sum, l) => sum + l.amount, BigInt(0));
    const activeLocks = locks.filter((l) => l.unlockAt > currentBlock).length;
    const nextUnlock = locks
      .filter((l) => l.unlockAt > currentBlock)
      .sort((a, b) => a.unlockAt - b.unlockAt)[0];
    return { totalLocked, activeLocks, nextUnlock };
  }, [locks, currentBlock]);

  if (!currentEntityId) {
    return (
      <div className="flex h-full items-center justify-center text-muted-foreground">
        {tc("selectEntity")}
      </div>
    );
  }

  const lockProgress = (unlockBlock: number) => {
    if (currentBlock >= unlockBlock) return 100;
    const lockedBlocks = unlockBlock - currentBlock;
    const total = unlockBlock;
    if (total <= 0) return 100;
    return Math.min(((currentBlock / total) * 100), 100);
  };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">
          Token Lock & Vesting
        </h1>
        <p className="text-muted-foreground">
          Manage token locks and vesting schedules
        </p>
      </div>

      <Tabs defaultValue="locks">
        <TabsList>
          <TabsTrigger value="locks" className="gap-2">
            <Lock className="h-4 w-4" />
            Locks
          </TabsTrigger>
          <TabsTrigger value="vesting" className="gap-2">
            <CalendarClock className="h-4 w-4" />
            Vesting
          </TabsTrigger>
        </TabsList>

        {/* ============ LOCKS TAB ============ */}
        <TabsContent value="locks" className="space-y-6">
          {/* Lock Stats */}
          <div className="grid gap-4 md:grid-cols-3">
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-sm font-medium flex items-center gap-2">
                  <Coins className="h-4 w-4 text-muted-foreground" />
                  Total Locked
                </CardTitle>
              </CardHeader>
              <CardContent>
                <p className="text-2xl font-bold">
                  {formatBalance(lockStats.totalLocked)}
                </p>
              </CardContent>
            </Card>
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-sm font-medium flex items-center gap-2">
                  <Lock className="h-4 w-4 text-muted-foreground" />
                  Active Locks
                </CardTitle>
              </CardHeader>
              <CardContent>
                <p className="text-2xl font-bold">{lockStats.activeLocks}</p>
              </CardContent>
            </Card>
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-sm font-medium flex items-center gap-2">
                  <Timer className="h-4 w-4 text-muted-foreground" />
                  Next Unlock
                </CardTitle>
              </CardHeader>
              <CardContent>
                {lockStats.nextUnlock ? (
                  <div>
                    <p className="text-2xl font-bold">
                      Block #{lockStats.nextUnlock.unlockAt.toLocaleString()}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      {lockStats.nextUnlock.unlockAt - currentBlock} blocks remaining
                    </p>
                  </div>
                ) : (
                  <p className="text-sm text-muted-foreground">No pending unlocks</p>
                )}
              </CardContent>
            </Card>
          </div>

          {/* Lock Creation */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Lock className="h-5 w-5" />
                Lock Tokens
              </CardTitle>
              <CardDescription>
                Lock tokens for a user until a specified block number
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">User Address</label>
                <Input
                  value={lockUser}
                  onChange={(e) => setLockUser(e.target.value)}
                  placeholder="5xxx..."
                />
              </div>
              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Amount</label>
                  <Input
                    type="number"
                    value={lockAmount}
                    onChange={(e) => setLockAmount(e.target.value)}
                    placeholder="0"
                    min="0"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">
                    Unlock At (block number)
                  </label>
                  <Input
                    type="number"
                    value={unlockAt}
                    onChange={(e) => setUnlockAt(e.target.value)}
                    placeholder={`Current: ${currentBlock}`}
                    min="0"
                  />
                </div>
              </div>
              <TxButton
                onClick={() => {
                  if (!lockUser || !lockAmount || !unlockAt || !currentEntityId)
                    return;
                  actions.lockTokens(
                    currentEntityId,
                    lockUser,
                    BigInt(lockAmount),
                    Number(unlockAt)
                  );
                }}
                txStatus={actions.txState.status}
                disabled={!lockUser || !lockAmount || !unlockAt}
              >
                <Lock className="mr-2 h-4 w-4" />
                Lock Tokens
              </TxButton>
            </CardContent>
          </Card>

          {/* Active Locks List */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Shield className="h-5 w-5" />
                Active Locks
              </CardTitle>
              <CardDescription>
                {locks.length} lock{locks.length !== 1 ? "s" : ""} found
              </CardDescription>
            </CardHeader>
            <CardContent>
              {locksLoading ? (
                <div className="flex justify-center py-8">
                  <div className="h-6 w-6 animate-spin rounded-full border-4 border-primary border-t-transparent" />
                </div>
              ) : locks.length === 0 ? (
                <p className="text-center text-sm text-muted-foreground py-8">
                  No token locks found
                </p>
              ) : (
                <div className="space-y-4">
                  {locks.map((lock, i) => {
                    const isExpired = currentBlock >= lock.unlockAt;
                    const progress = lockProgress(lock.unlockAt);
                    return (
                      <div
                        key={`${lock.account}-${i}`}
                        className={`rounded-lg border p-4 space-y-3 ${
                          isExpired
                            ? "border-green-200 bg-green-50/50 dark:border-green-900 dark:bg-green-950/20"
                            : ""
                        }`}
                      >
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-3">
                            <AddressDisplay address={lock.account} chars={4} />
                            {isExpired && (
                              <Badge className="bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400">
                                Unlockable
                              </Badge>
                            )}
                          </div>
                          <span className="text-sm font-bold">
                            {formatBalance(lock.amount)}
                          </span>
                        </div>
                        <div className="space-y-1">
                          <div className="flex justify-between text-xs text-muted-foreground">
                            <span>
                              Block #{currentBlock.toLocaleString()}
                            </span>
                            <span>
                              Unlock: #{lock.unlockAt.toLocaleString()}
                            </span>
                          </div>
                          <Progress value={progress} className="h-2" />
                        </div>
                        {!isExpired && (
                          <p className="text-xs text-muted-foreground">
                            {lock.unlockAt - currentBlock} blocks remaining
                          </p>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}

              <Separator className="my-4" />

              <TxButton
                onClick={() => {
                  if (currentEntityId) actions.unlockTokens(currentEntityId);
                }}
                txStatus={actions.txState.status}
              >
                <Unlock className="mr-2 h-4 w-4" />
                Unlock Expired Tokens
              </TxButton>
            </CardContent>
          </Card>
        </TabsContent>

        {/* ============ VESTING TAB ============ */}
        <TabsContent value="vesting" className="space-y-6">
          {/* Current User's Vesting Schedule */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <CalendarClock className="h-5 w-5" />
                Your Vesting Schedule
              </CardTitle>
              <CardDescription>
                {address
                  ? `Vesting details for your wallet`
                  : "Connect wallet to view your vesting schedule"}
              </CardDescription>
            </CardHeader>
            <CardContent>
              {vestingLoading ? (
                <div className="flex justify-center py-8">
                  <div className="h-6 w-6 animate-spin rounded-full border-4 border-primary border-t-transparent" />
                </div>
              ) : !schedule ? (
                <p className="text-center text-sm text-muted-foreground py-8">
                  No vesting schedule found for your address
                </p>
              ) : (
                <div className="space-y-6">
                  {/* Vesting amounts */}
                  <div className="grid gap-4 md:grid-cols-3">
                    <div className="rounded-lg border p-4 space-y-1">
                      <p className="text-xs text-muted-foreground uppercase tracking-wider">
                        Total Vesting
                      </p>
                      <p className="text-xl font-bold">
                        {formatBalance(schedule.total)}
                      </p>
                    </div>
                    <div className="rounded-lg border p-4 space-y-1">
                      <p className="text-xs text-muted-foreground uppercase tracking-wider">
                        Released
                      </p>
                      <p className="text-xl font-bold text-green-600">
                        {formatBalance(schedule.released)}
                      </p>
                    </div>
                    <div className="rounded-lg border p-4 space-y-1">
                      <p className="text-xs text-muted-foreground uppercase tracking-wider">
                        Remaining
                      </p>
                      <p className="text-xl font-bold text-orange-600">
                        {formatBalance(schedule.total - schedule.released)}
                      </p>
                    </div>
                  </div>

                  {/* Release progress */}
                  <div className="space-y-2">
                    <div className="flex justify-between text-sm">
                      <span className="text-muted-foreground">
                        Released Amount
                      </span>
                      <span className="font-medium">
                        {schedule.total > BigInt(0)
                          ? (
                              (Number(schedule.released) /
                                Number(schedule.total)) *
                              100
                            ).toFixed(1)
                          : "0"}
                        %
                      </span>
                    </div>
                    <Progress
                      value={
                        schedule.total > BigInt(0)
                          ? (Number(schedule.released) /
                              Number(schedule.total)) *
                            100
                          : 0
                      }
                      className="h-3"
                    />
                  </div>

                  {/* Block milestones */}
                  <div className="space-y-3">
                    <div className="flex justify-between items-center">
                      <span className="text-sm text-muted-foreground">
                        Start Block
                      </span>
                      <Badge variant="outline">
                        #{schedule.startBlock.toLocaleString()}
                      </Badge>
                    </div>
                    <div className="flex justify-between items-center">
                      <span className="text-sm text-muted-foreground">
                        Cliff End
                      </span>
                      <Badge variant="outline">
                        #
                        {(
                          schedule.startBlock + schedule.cliffBlocks
                        ).toLocaleString()}
                      </Badge>
                    </div>
                    <div className="flex justify-between items-center">
                      <span className="text-sm text-muted-foreground">
                        Vesting End
                      </span>
                      <Badge variant="outline">
                        #
                        {(
                          schedule.startBlock + schedule.vestingBlocks
                        ).toLocaleString()}
                      </Badge>
                    </div>
                    <div className="flex justify-between items-center">
                      <span className="text-sm text-muted-foreground">
                        Current Block
                      </span>
                      <Badge>{`#${currentBlock.toLocaleString()}`}</Badge>
                    </div>
                  </div>

                  {/* Cliff progress */}
                  {(() => {
                    const cliffEnd =
                      schedule.startBlock + schedule.cliffBlocks;
                    const vestEnd =
                      schedule.startBlock + schedule.vestingBlocks;
                    const cliffProgress =
                      currentBlock >= cliffEnd
                        ? 100
                        : schedule.cliffBlocks > 0
                          ? Math.max(
                              0,
                              ((currentBlock - schedule.startBlock) /
                                schedule.cliffBlocks) *
                                100
                            )
                          : 100;
                    const vestProgress =
                      currentBlock >= vestEnd
                        ? 100
                        : schedule.vestingBlocks > 0
                          ? Math.max(
                              0,
                              ((currentBlock - schedule.startBlock) /
                                schedule.vestingBlocks) *
                                100
                            )
                          : 100;
                    return (
                      <div className="space-y-4">
                        <div className="space-y-1">
                          <div className="flex justify-between text-xs text-muted-foreground">
                            <span>Cliff Progress</span>
                            <span>{cliffProgress.toFixed(0)}%</span>
                          </div>
                          <Progress value={cliffProgress} className="h-2" />
                        </div>
                        <div className="space-y-1">
                          <div className="flex justify-between text-xs text-muted-foreground">
                            <span>Vesting Progress</span>
                            <span>{vestProgress.toFixed(0)}%</span>
                          </div>
                          <Progress value={vestProgress} className="h-2" />
                        </div>
                      </div>
                    );
                  })()}

                  <TxButton
                    onClick={() => {
                      if (currentEntityId)
                        actions.releaseVested(currentEntityId);
                    }}
                    txStatus={actions.txState.status}
                    className="w-full"
                  >
                    <Unlock className="mr-2 h-4 w-4" />
                    Release Vested Tokens
                  </TxButton>
                </div>
              )}
            </CardContent>
          </Card>

          {/* Create Vesting Schedule (admin) */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Clock className="h-5 w-5" />
                Create Vesting Schedule
              </CardTitle>
              <CardDescription>
                Set up a new vesting schedule for a beneficiary
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">
                  Beneficiary Address
                </label>
                <Input
                  value={vestBeneficiary}
                  onChange={(e) => setVestBeneficiary(e.target.value)}
                  placeholder="5xxx..."
                />
              </div>
              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-2">
                  <label className="text-sm font-medium">
                    Total Amount (raw units)
                  </label>
                  <Input
                    type="number"
                    value={vestTotal}
                    onChange={(e) => setVestTotal(e.target.value)}
                    placeholder="0"
                    min="0"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">
                    Start Block
                  </label>
                  <Input
                    type="number"
                    value={vestStart}
                    onChange={(e) => setVestStart(e.target.value)}
                    placeholder={`Current: ${currentBlock}`}
                    min="0"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">
                    Cliff Duration (blocks)
                  </label>
                  <Input
                    type="number"
                    value={vestCliff}
                    onChange={(e) => setVestCliff(e.target.value)}
                    placeholder="0"
                    min="0"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">
                    Vesting Duration (blocks)
                  </label>
                  <Input
                    type="number"
                    value={vestDuration}
                    onChange={(e) => setVestDuration(e.target.value)}
                    placeholder="0"
                    min="0"
                  />
                </div>
              </div>
              <TxButton
                onClick={() => {
                  if (
                    !vestBeneficiary ||
                    !vestTotal ||
                    !vestStart ||
                    !vestCliff ||
                    !vestDuration ||
                    !currentEntityId
                  )
                    return;
                  actions.createVesting(
                    currentEntityId,
                    vestBeneficiary,
                    BigInt(vestTotal),
                    Number(vestStart),
                    Number(vestCliff),
                    Number(vestDuration)
                  );
                }}
                txStatus={actions.txState.status}
                disabled={
                  !vestBeneficiary ||
                  !vestTotal ||
                  !vestStart ||
                  !vestCliff ||
                  !vestDuration
                }
              >
                <CalendarClock className="mr-2 h-4 w-4" />
                Create Vesting Schedule
              </TxButton>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>

      {actions.txState.status === "finalized" && (
        <div className="rounded-lg border border-green-200 bg-green-50 p-3 text-sm text-green-700 dark:border-green-800 dark:bg-green-950/50 dark:text-green-400">
          Transaction finalized successfully.
        </div>
      )}
      {actions.txState.status === "error" && (
        <div className="rounded-lg border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive">
          {actions.txState.error}
        </div>
      )}
    </div>
  );
}
