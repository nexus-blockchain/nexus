"use client";

import { useState, useEffect } from "react";
import { useEntityStore } from "@/stores/entity";
import { useEntityKycRequirement, useKycActions } from "@/hooks/useKyc";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Select } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Separator } from "@/components/ui/separator";
import { TxButton } from "@/components/shared/TxButton";
import { KYC_LEVELS } from "@/lib/constants";
import { ArrowLeft, Shield, Settings, Globe, X, Plus, Trash2 } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function KycSettingsPage() {
  const { currentEntityId } = useEntityStore();
  const { requirement, isLoading, refetch } = useEntityKycRequirement(currentEntityId);
  const actions = useKycActions();
  const tc = useTranslations("common");

  const [minLevel, setMinLevel] = useState<string>(KYC_LEVELS[1]);
  const [mandatory, setMandatory] = useState(false);
  const [gracePeriod, setGracePeriod] = useState("0");
  const [allowHighRisk, setAllowHighRisk] = useState(false);
  const [maxRiskScore, setMaxRiskScore] = useState("100");

  const [countries, setCountries] = useState<string[]>([]);
  const [newCountry, setNewCountry] = useState("");

  useEffect(() => {
    if (requirement) {
      setMinLevel(requirement.minLevel);
      setMandatory(requirement.mandatory);
      setGracePeriod(String(requirement.gracePeriod));
      setAllowHighRisk(requirement.allowHighRiskCountries);
      setMaxRiskScore(String(requirement.maxRiskScore));
    }
  }, [requirement]);

  if (!currentEntityId) {
    return (
      <div className="flex h-full items-center justify-center text-muted-foreground">
        {tc("selectEntity")}
      </div>
    );
  }

  const handleSetRequirement = () => {
    actions.setEntityRequirement(
      currentEntityId,
      minLevel,
      mandatory,
      Number(gracePeriod),
      allowHighRisk,
      Number(maxRiskScore),
    );
  };

  const handleRemoveRequirement = () => {
    actions.removeEntityRequirement(currentEntityId);
  };

  const handleAddCountry = () => {
    const code = newCountry.trim().toUpperCase();
    if (code.length === 2 && !countries.includes(code)) {
      setCountries((prev) => [...prev, code]);
      setNewCountry("");
    }
  };

  const handleRemoveCountry = (code: string) => {
    setCountries((prev) => prev.filter((c) => c !== code));
  };

  const handleSaveCountries = () => {
    actions.updateHighRiskCountries(countries);
  };

  const scoreNum = Number(maxRiskScore) || 0;
  const riskColor =
    scoreNum < 30 ? "text-green-600" : scoreNum <= 60 ? "text-yellow-600" : "text-red-600";

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/kyc"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">KYC Settings</h1>
          <p className="text-muted-foreground">
            Configure entity-level KYC requirements and compliance rules
          </p>
        </div>
      </div>

      {/* ── Current Requirement ───────────────────────────────── */}
      {isLoading ? (
        <Card>
          <CardContent className="flex justify-center py-8">
            <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
          </CardContent>
        </Card>
      ) : requirement ? (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Shield className="h-5 w-5 text-primary" />Current Requirement
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="grid gap-4 sm:grid-cols-2 md:grid-cols-5">
              <div className="space-y-1">
                <p className="text-xs text-muted-foreground">Min Level</p>
                <p className="text-sm font-semibold">{requirement.minLevel}</p>
              </div>
              <div className="space-y-1">
                <p className="text-xs text-muted-foreground">Mandatory</p>
                <Badge variant={requirement.mandatory ? "default" : "secondary"}>
                  {requirement.mandatory ? "Yes" : "No"}
                </Badge>
              </div>
              <div className="space-y-1">
                <p className="text-xs text-muted-foreground">Grace Period</p>
                <p className="text-sm font-semibold">{requirement.gracePeriod} blocks</p>
              </div>
              <div className="space-y-1">
                <p className="text-xs text-muted-foreground">High-Risk Countries</p>
                <Badge variant={requirement.allowHighRiskCountries ? "default" : "destructive"}>
                  {requirement.allowHighRiskCountries ? "Allowed" : "Blocked"}
                </Badge>
              </div>
              <div className="space-y-1">
                <p className="text-xs text-muted-foreground">Max Risk Score</p>
                <p className="text-sm font-semibold">{requirement.maxRiskScore}</p>
              </div>
            </div>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-8">
            <Shield className="h-10 w-10 text-muted-foreground/50" />
            <p className="mt-3 text-sm text-muted-foreground">
              No KYC requirement configured for this entity.
            </p>
          </CardContent>
        </Card>
      )}

      {/* ── Set / Update Requirement ──────────────────────────── */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Settings className="h-5 w-5" />
            {requirement ? "Update" : "Set"} Entity Requirement
          </CardTitle>
          <CardDescription>
            Define the KYC requirements members must meet
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-5">
          <div className="grid gap-4 md:grid-cols-2">
            <div className="space-y-2">
              <label className="text-sm font-medium">Minimum KYC Level</label>
              <Select value={minLevel} onChange={(e) => setMinLevel(e.target.value)}>
                {KYC_LEVELS.map((level) => (
                  <option key={level} value={level}>{level}</option>
                ))}
              </Select>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Grace Period (blocks)</label>
              <Input
                type="number"
                value={gracePeriod}
                onChange={(e) => setGracePeriod(e.target.value)}
                min="0"
                placeholder="Number of blocks"
              />
            </div>
          </div>

          <div className="flex items-center justify-between rounded-lg border p-4">
            <div>
              <p className="text-sm font-medium">Mandatory</p>
              <p className="text-xs text-muted-foreground">
                Require all members to complete KYC verification
              </p>
            </div>
            <Switch checked={mandatory} onCheckedChange={setMandatory} />
          </div>

          <div className="flex items-center justify-between rounded-lg border p-4">
            <div>
              <p className="text-sm font-medium">Allow High-Risk Countries</p>
              <p className="text-xs text-muted-foreground">
                Accept KYC from users in high-risk jurisdictions
              </p>
            </div>
            <Switch checked={allowHighRisk} onCheckedChange={setAllowHighRisk} />
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium">Max Risk Score (0–100)</label>
            <div className="flex items-center gap-4">
              <input
                type="range"
                min="0"
                max="100"
                value={maxRiskScore}
                onChange={(e) => setMaxRiskScore(e.target.value)}
                className="flex-1 h-2 rounded-lg appearance-none cursor-pointer bg-muted accent-primary"
              />
              <span className={`text-sm font-mono font-bold w-8 text-right ${riskColor}`}>
                {maxRiskScore}
              </span>
            </div>
            <div className="flex justify-between text-xs text-muted-foreground">
              <span>Low Risk</span>
              <span>High Risk</span>
            </div>
          </div>

          <div className="flex gap-3">
            <TxButton onClick={handleSetRequirement} txStatus={actions.txState.status}>
              <Shield className="mr-2 h-4 w-4" />
              {requirement ? "Update" : "Set"} Requirement
            </TxButton>
            {requirement && (
              <Button variant="destructive" onClick={handleRemoveRequirement}>
                <Trash2 className="mr-2 h-4 w-4" />Remove Requirement
              </Button>
            )}
          </div>
        </CardContent>
      </Card>

      {/* ── High-Risk Countries ───────────────────────────────── */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Globe className="h-5 w-5" />High-Risk Countries
          </CardTitle>
          <CardDescription>
            Manage the list of high-risk country codes for KYC screening
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex gap-3">
            <Input
              value={newCountry}
              onChange={(e) => setNewCountry(e.target.value.toUpperCase())}
              placeholder="e.g. KP"
              maxLength={2}
              className="w-32"
              onKeyDown={(e) => { if (e.key === "Enter") handleAddCountry(); }}
            />
            <Button
              variant="outline"
              onClick={handleAddCountry}
              disabled={newCountry.trim().length !== 2}
            >
              <Plus className="mr-2 h-4 w-4" />Add Country
            </Button>
          </div>

          {countries.length === 0 ? (
            <p className="text-sm text-muted-foreground py-4">
              No high-risk countries configured. Add ISO 3166-1 alpha-2 codes above.
            </p>
          ) : (
            <div className="flex flex-wrap gap-2">
              {countries.map((code) => (
                <Badge key={code} variant="secondary" className="gap-1.5 pl-2.5 pr-1.5 py-1">
                  {code}
                  <button
                    onClick={() => handleRemoveCountry(code)}
                    className="ml-0.5 rounded-full hover:bg-muted-foreground/20 p-0.5"
                  >
                    <X className="h-3 w-3" />
                  </button>
                </Badge>
              ))}
            </div>
          )}

          <Separator />

          <TxButton
            onClick={handleSaveCountries}
            txStatus={actions.txState.status}
            disabled={countries.length === 0}
          >
            <Globe className="mr-2 h-4 w-4" />Save Countries List
          </TxButton>
        </CardContent>
      </Card>

      {actions.txState.status === "finalized" && (
        <p className="text-sm text-green-600">Settings saved successfully.</p>
      )}
      {actions.txState.status === "error" && (
        <p className="text-sm text-destructive">{actions.txState.error}</p>
      )}
    </div>
  );
}
