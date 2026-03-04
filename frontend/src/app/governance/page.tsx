"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useProposals, useGovernanceActions } from "@/hooks/useGovernance";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Vote, Plus, CheckCircle, XCircle, Clock, Gavel } from "lucide-react";
import { useTranslations } from "next-intl";

export default function GovernancePage() {
  const { currentEntityId } = useEntityStore();
  const { proposals, isLoading } = useProposals(currentEntityId);
  const actions = useGovernanceActions();
  const t = useTranslations("governance");
  const tc = useTranslations("common");
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");

  if (!currentEntityId) return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;

  const handleCreate = () => {
    if (title && description) {
      actions.createProposal(currentEntityId, "General", title, description);
      setTitle("");
      setDescription("");
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Governance</h1>
        <p className="text-muted-foreground">DAO proposals, voting, and execution</p>
      </div>

      <Tabs defaultValue="proposals">
        <TabsList>
          <TabsTrigger value="proposals">Proposals</TabsTrigger>
          <TabsTrigger value="create">Create Proposal</TabsTrigger>
        </TabsList>

        <TabsContent value="proposals" className="space-y-4">
          {isLoading ? (
            <div className="flex justify-center py-8"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
          ) : proposals.length === 0 ? (
            <Card>
              <CardContent className="flex flex-col items-center justify-center py-12">
                <Vote className="h-12 w-12 text-muted-foreground/50" />
                <p className="mt-4 text-lg font-medium">No Proposals</p>
                <p className="text-sm text-muted-foreground">Create the first governance proposal.</p>
              </CardContent>
            </Card>
          ) : (
            proposals.map((proposal: any) => (
              <Card key={proposal.id}>
                <CardHeader>
                  <div className="flex items-center justify-between">
                    <CardTitle className="text-lg">#{proposal.id} — {proposal.title || "Untitled"}</CardTitle>
                    <StatusBadge status={proposal.status} />
                  </div>
                  <CardDescription>Proposed by {proposal.proposer?.slice(0, 8)}...</CardDescription>
                </CardHeader>
                <CardContent>
                  <div className="flex items-center gap-6 text-sm">
                    <div className="flex items-center gap-1 text-green-600"><CheckCircle className="h-4 w-4" />{proposal.ayeVotes || 0} Aye</div>
                    <div className="flex items-center gap-1 text-red-600"><XCircle className="h-4 w-4" />{proposal.nayVotes || 0} Nay</div>
                    <div className="flex items-center gap-1 text-muted-foreground"><Clock className="h-4 w-4" />Block {proposal.endBlock || "—"}</div>
                  </div>
                  {proposal.status === "Active" && (
                    <div className="mt-4 flex gap-2">
                      <Button size="sm" className="bg-green-600 hover:bg-green-700" onClick={() => actions.vote(proposal.id, "Aye")}>Vote Aye</Button>
                      <Button size="sm" variant="destructive" onClick={() => actions.vote(proposal.id, "Nay")}>Vote Nay</Button>
                      <Button size="sm" variant="outline" onClick={() => actions.finalizeVoting(proposal.id)}>Finalize</Button>
                    </div>
                  )}
                  {proposal.status === "Passed" && (
                    <div className="mt-4">
                      <Button size="sm" onClick={() => actions.executeProposal(proposal.id)}>Execute Proposal</Button>
                    </div>
                  )}
                </CardContent>
              </Card>
            ))
          )}
        </TabsContent>

        <TabsContent value="create">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2"><Plus className="h-5 w-5" />New Proposal</CardTitle>
              <CardDescription>Submit a governance proposal for voting</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">Title</label>
                <Input value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Proposal title" />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">Description / Call Data CID</label>
                <Input value={description} onChange={(e) => setDescription(e.target.value)} placeholder="IPFS CID or description" />
              </div>
              <Button onClick={handleCreate}>Submit Proposal</Button>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
