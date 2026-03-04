"use client";

import { useEntityStore } from "@/stores/entity";
import { useMembers, useLevels, useMemberActions } from "@/hooks/useMember";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { Users, UserCheck, Crown, Star } from "lucide-react";
import { useTranslations } from "next-intl";

export default function MembersPage() {
  const { currentEntityId } = useEntityStore();
  const { members, isLoading } = useMembers(currentEntityId);
  const { levels } = useLevels(currentEntityId);
  const actions = useMemberActions();
  const t = useTranslations("members");
  const tc = useTranslations("common");

  if (!currentEntityId) return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">{t("title")}</h1>
          <p className="text-muted-foreground">{t("subtitle")}</p>
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t("totalMembers")}</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">{members.length}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t("levelsConfigured")}</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">{levels.length}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t("pendingApproval")}</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">{members.filter((m: any) => m.status === "Pending").length}</p></CardContent>
        </Card>
      </div>

      {levels.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Crown className="h-5 w-5" />{t("levelSystem")}</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="flex flex-wrap gap-3">
              {levels.map((level: any, i: number) => (
                <div key={i} className="flex items-center gap-2 rounded-lg border p-3">
                  <Star className="h-4 w-4 text-yellow-500" />
                  <div>
                    <p className="text-sm font-medium">{level.name || `Level ${level.id}`}</p>
                    <p className="text-xs text-muted-foreground">Threshold: {level.threshold}</p>
                  </div>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Users className="h-5 w-5" />{t("memberList")}</CardTitle>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="flex justify-center py-8"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
          ) : members.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t("noMembers")}</p>
          ) : (
            <div className="space-y-3">
              {members.map((member: any) => (
                <div key={member.account} className="flex items-center gap-4 rounded-lg border p-4">
                  <AddressDisplay address={member.account} />
                  <Badge variant="secondary">{member.level || "Normal"}</Badge>
                  <span className="text-sm text-muted-foreground">Spent: {member.totalSpent || 0}</span>
                  <span className="text-sm text-muted-foreground">Referrer: {member.referrer ? "Yes" : "None"}</span>
                  {member.status === "Pending" && (
                    <div className="ml-auto flex gap-2">
                      <Button size="sm" onClick={() => actions.approveMember(member.shopId || 0, member.account)}>
                        <UserCheck className="mr-1 h-3 w-3" />Approve
                      </Button>
                      <Button size="sm" variant="outline" onClick={() => actions.rejectMember(member.shopId || 0, member.account)}>Reject</Button>
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
