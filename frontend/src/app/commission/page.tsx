"use client";

import { useEntityStore } from "@/stores/entity";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Percent, Layers, ArrowRight } from "lucide-react";
import { useTranslations } from "next-intl";

export default function CommissionPage() {
  const { currentEntityId } = useEntityStore();
  const t = useTranslations("commission");
  const tc = useTranslations("common");

  if (!currentEntityId) return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">{t("title")}</h1>
        <p className="text-muted-foreground">{t("subtitle")}</p>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t("activeTiers")}</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Layers className="h-5 w-5 text-muted-foreground" />
            <p className="text-2xl font-bold">—</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t("totalDistributed")}</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Percent className="h-5 w-5 text-muted-foreground" />
            <p className="text-2xl font-bold">— NEX</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t("activeReferrers")}</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <ArrowRight className="h-5 w-5 text-muted-foreground" />
            <p className="text-2xl font-bold">—</p>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>{t("howItWorks")}</CardTitle>
          <CardDescription>{t("howItWorksDesc")}</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            <div className="rounded-lg border p-4">
              <h4 className="text-sm font-semibold mb-2">{t("howItWorks")}</h4>
              <ul className="space-y-2 text-sm text-muted-foreground">
                <li className="flex items-start gap-2"><span className="mt-1 h-1.5 w-1.5 rounded-full bg-primary shrink-0" />{t("rule1desc")}</li>
                <li className="flex items-start gap-2"><span className="mt-1 h-1.5 w-1.5 rounded-full bg-primary shrink-0" />{t("rule2desc")}</li>
                <li className="flex items-start gap-2"><span className="mt-1 h-1.5 w-1.5 rounded-full bg-primary shrink-0" />{t("rule3desc")}</li>
              </ul>
            </div>
            <p className="text-sm text-muted-foreground">Commission tier configuration and distribution history will be populated from on-chain data.</p>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
