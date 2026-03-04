"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { useTranslations } from "next-intl";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { TxButton } from "@/components/shared/TxButton";
import { useRegisterEntity } from "@/hooks/useEntity";
import { useWalletStore } from "@/stores/wallet";
import { ENTITY_TYPES } from "@/lib/constants";
import { Building2, ArrowLeft } from "lucide-react";
import Link from "next/link";

export default function CreateEntityPage() {
  const t = useTranslations("entity.create");
  const tc = useTranslations("common");
  const router = useRouter();
  const { isConnected } = useWalletStore();
  const { registerEntity, txState, resetTx } = useRegisterEntity();

  const [name, setName] = useState("");
  const [entityType, setEntityType] = useState<string>("");
  const [referrer, setReferrer] = useState("");

  if (!isConnected) {
    return (
      <div className="flex items-center justify-center h-[60vh]">
        <p className="text-muted-foreground">{tc("selectEntity")}</p>
      </div>
    );
  }

  const handleSubmit = () => {
    if (!name.trim() || !entityType) return;
    registerEntity(name.trim(), entityType, referrer.trim() || null);
  };

  const isValid = name.trim().length > 0 && entityType.length > 0;

  return (
    <div className="space-y-6 p-6">
      <div className="flex items-center gap-4">
        <Link href="/">
          <Button variant="ghost" size="icon">
            <ArrowLeft className="h-4 w-4" />
          </Button>
        </Link>
        <div>
          <h1 className="text-2xl font-bold tracking-tight">{t("title")}</h1>
          <p className="text-muted-foreground">{t("subtitle")}</p>
        </div>
      </div>

      <div className="grid gap-6 max-w-2xl">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Building2 className="h-5 w-5" />
              {t("basicInfo")}
            </CardTitle>
            <CardDescription>{t("basicInfoDesc")}</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <label htmlFor="name" className="text-sm font-medium">
                {t("entityName")}
              </label>
              <Input
                id="name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={t("entityNamePlaceholder")}
                maxLength={64}
              />
              <p className="text-xs text-muted-foreground">
                {name.length}/64
              </p>
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium">{t("entityType")}</label>
              <Select
                value={entityType}
                onChange={(e) => setEntityType(e.target.value)}
              >
                <option value="" disabled>
                  {t("selectType")}
                </option>
                {ENTITY_TYPES.map((type) => (
                  <option key={type} value={type}>
                    {type}
                  </option>
                ))}
              </Select>
              <p className="text-xs text-muted-foreground">{t("typeDesc")}</p>
            </div>

            <div className="space-y-2">
              <label htmlFor="referrer" className="text-sm font-medium">
                {t("referrer")}
              </label>
              <Input
                id="referrer"
                value={referrer}
                onChange={(e) => setReferrer(e.target.value)}
                placeholder={t("referrerPlaceholder")}
              />
              <p className="text-xs text-muted-foreground">{t("referrerDesc")}</p>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>{t("fundInfo")}</CardTitle>
            <CardDescription>{t("fundInfoDesc")}</CardDescription>
          </CardHeader>
          <CardContent>
            <ul className="list-disc list-inside space-y-1 text-sm text-muted-foreground">
              <li>{t("fundNote1")}</li>
              <li>{t("fundNote2")}</li>
              <li>{t("fundNote3")}</li>
            </ul>
          </CardContent>
        </Card>

        <div className="flex gap-3">
          <TxButton
            onClick={handleSubmit}
            txStatus={txState.status}
            disabled={!isValid}
            className="w-full"
          >
            {t("submit")}
          </TxButton>
        </div>

        {txState.status === "finalized" && (
          <div className="rounded-lg border border-green-200 bg-green-50 p-4 text-center">
            <p className="text-green-800 font-medium">{t("success")}</p>
            <Button
              variant="link"
              className="mt-2"
              onClick={() => router.push("/")}
            >
              {t("goToDashboard")}
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}
