"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useDisclosures, useAnnouncements, useDisclosureActions } from "@/hooks/useDisclosure";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { FileText, Megaphone, Plus, Pin, AlertTriangle } from "lucide-react";

export default function DisclosurePage() {
  const { currentEntityId } = useEntityStore();
  const { disclosures, isLoading } = useDisclosures(currentEntityId);
  const { announcements } = useAnnouncements(currentEntityId);
  const actions = useDisclosureActions();
  const [discType, setDiscType] = useState("");
  const [contentCid, setContentCid] = useState("");
  const [annTitle, setAnnTitle] = useState("");
  const [annCid, setAnnCid] = useState("");

  if (!currentEntityId) return <div className="flex h-full items-center justify-center text-muted-foreground">Select an entity first</div>;

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Disclosure & Announcements</h1>
        <p className="text-muted-foreground">Financial disclosures, compliance reports, and announcements</p>
      </div>

      <Tabs defaultValue="disclosures">
        <TabsList>
          <TabsTrigger value="disclosures">Disclosures</TabsTrigger>
          <TabsTrigger value="announcements">Announcements</TabsTrigger>
          <TabsTrigger value="insiders">Insider Management</TabsTrigger>
        </TabsList>

        <TabsContent value="disclosures" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2"><Plus className="h-5 w-5" />Publish Disclosure</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Disclosure Type</label>
                  <Input value={discType} onChange={(e) => setDiscType(e.target.value)} placeholder="Financial, Operational, etc." />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Content CID</label>
                  <Input value={contentCid} onChange={(e) => setContentCid(e.target.value)} placeholder="IPFS CID" />
                </div>
              </div>
              <Button onClick={() => { if (discType && contentCid && currentEntityId) actions.publishDisclosure(currentEntityId, discType, contentCid, "Normal"); }}>
                <FileText className="mr-2 h-4 w-4" />Publish
              </Button>
            </CardContent>
          </Card>

          {isLoading ? (
            <div className="flex justify-center py-8"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
          ) : disclosures.length === 0 ? (
            <Card><CardContent className="py-8 text-center text-sm text-muted-foreground">No disclosures published yet.</CardContent></Card>
          ) : (
            disclosures.map((disc: any, i: number) => (
              <Card key={i}>
                <CardHeader>
                  <div className="flex items-center justify-between">
                    <CardTitle className="text-base">{disc.disclosureType || "Disclosure"} #{disc.id || i}</CardTitle>
                    <StatusBadge status={disc.status || "Published"} />
                  </div>
                </CardHeader>
                <CardContent className="flex items-center justify-between">
                  <span className="text-sm text-muted-foreground font-mono">{disc.contentCid?.slice(0, 20)}...</span>
                  <div className="flex gap-2">
                    <Button size="sm" variant="outline" onClick={() => actions.withdrawDisclosure(currentEntityId, disc.id)}>Withdraw</Button>
                  </div>
                </CardContent>
              </Card>
            ))
          )}
        </TabsContent>

        <TabsContent value="announcements" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2"><Megaphone className="h-5 w-5" />Publish Announcement</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Title</label>
                  <Input value={annTitle} onChange={(e) => setAnnTitle(e.target.value)} placeholder="Announcement title" />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Content CID</label>
                  <Input value={annCid} onChange={(e) => setAnnCid(e.target.value)} placeholder="IPFS CID" />
                </div>
              </div>
              <Button onClick={() => { if (annTitle && annCid && currentEntityId) actions.publishAnnouncement(currentEntityId, annTitle, annCid, "General", null); }}>
                <Megaphone className="mr-2 h-4 w-4" />Publish
              </Button>
            </CardContent>
          </Card>

          {announcements.length === 0 ? (
            <Card><CardContent className="py-8 text-center text-sm text-muted-foreground">No announcements yet.</CardContent></Card>
          ) : (
            announcements.map((ann: any, i: number) => (
              <Card key={i}>
                <CardHeader>
                  <div className="flex items-center justify-between">
                    <CardTitle className="text-base">{ann.title || `Announcement #${ann.id || i}`}</CardTitle>
                    <div className="flex items-center gap-2">
                      {ann.pinned && <Pin className="h-4 w-4 text-primary" />}
                      <StatusBadge status={ann.status || "Active"} />
                    </div>
                  </div>
                </CardHeader>
                <CardContent className="flex items-center justify-between">
                  <span className="text-sm text-muted-foreground font-mono">{ann.contentCid?.slice(0, 20)}...</span>
                  <div className="flex gap-2">
                    <Button size="sm" variant="outline" onClick={() => actions.pinAnnouncement(currentEntityId, ann.id)}>
                      <Pin className="mr-1 h-3 w-3" />Pin
                    </Button>
                    <Button size="sm" variant="outline" onClick={() => actions.withdrawAnnouncement(currentEntityId, ann.id)}>Withdraw</Button>
                  </div>
                </CardContent>
              </Card>
            ))
          )}
        </TabsContent>

        <TabsContent value="insiders">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2"><AlertTriangle className="h-5 w-5" />Insider Management</CardTitle>
              <CardDescription>Manage insider list and blackout windows for compliance</CardDescription>
            </CardHeader>
            <CardContent>
              <p className="text-sm text-muted-foreground">Insider management and blackout window controls will be populated from chain state.</p>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
